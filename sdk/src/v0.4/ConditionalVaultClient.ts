import { AnchorProvider, Program, utils } from "@coral-xyz/anchor";
import {
  AddressLookupTableAccount,
  Keypair,
  PublicKey,
  SystemProgram,
} from "@solana/web3.js";

import {
  ConditionalVault,
  IDL as ConditionalVaultIDL,
} from "./types/conditional_vault.js";

import BN from "bn.js";
import {
  CONDITIONAL_VAULT_PROGRAM_ID,
  MPL_TOKEN_METADATA_PROGRAM_ID,
} from "./constants.js";
import {
  getQuestionAddr,
  getMetadataAddr,
  getVaultAddr,
  getVaultFinalizeMintAddr,
  getVaultRevertMintAddr,
  getConditionalTokenMintAddr,
} from "./utils/index.js";
import {
  createAssociatedTokenAccountIdempotentInstruction,
  createAssociatedTokenAccountInstruction,
  getAssociatedTokenAddressSync,
  TOKEN_PROGRAM_ID,
} from "@solana/spl-token";

export type CreateVaultClientParams = {
  provider: AnchorProvider;
  conditionalVaultProgramId?: PublicKey;
};

export class ConditionalVaultClient {
  public readonly provider: AnchorProvider;
  public readonly vaultProgram: Program<ConditionalVault>;

  constructor(provider: AnchorProvider, conditionalVaultProgramId: PublicKey) {
    this.provider = provider;
    this.vaultProgram = new Program<ConditionalVault>(
      ConditionalVaultIDL,
      conditionalVaultProgramId,
      provider
    );
  }

  public static createClient(
    createVaultClientParams: CreateVaultClientParams
  ): ConditionalVaultClient {
    let { provider, conditionalVaultProgramId } = createVaultClientParams;

    return new ConditionalVaultClient(
      provider,
      conditionalVaultProgramId || CONDITIONAL_VAULT_PROGRAM_ID
    );
  }

  async fetchQuestion(question: PublicKey) {
    return this.vaultProgram.account.question.fetch(question);
  }

  async fetchVault(vault: PublicKey) {
    return this.vaultProgram.account.conditionalVault.fetch(vault);
  }

  async getVault(vault: PublicKey) {
    return this.vaultProgram.account.conditionalVault.fetch(vault);
  }

  initializeQuestionIx(
    questionId: Uint8Array,
    oracle: PublicKey,
    numConditions: number
  ) {
    const [question] = getQuestionAddr(
      this.vaultProgram.programId,
      questionId,
      oracle,
      numConditions
    );

    return this.vaultProgram.methods
      .initializeQuestion({
        questionId: Array.from(questionId),
        oracle,
        numConditions,
      })
      .accounts({
        question,
      });
  }

  async initializeQuestion(
    questionId: Uint8Array,
    oracle: PublicKey,
    numConditions: number
  ): Promise<PublicKey> {
    const [question] = getQuestionAddr(
      this.vaultProgram.programId,
      questionId,
      oracle,
      numConditions
    );

    await this.initializeQuestionIx(questionId, oracle, numConditions).rpc();

    return question;
  }

  initializeVaultIx(
    question: PublicKey,
    underlyingTokenMint: PublicKey,
    numOutcomes: number
  ) {
    const [vault] = getVaultAddr(
      this.vaultProgram.programId,
      question,
      underlyingTokenMint
    );

    let conditionalTokenMintAddrs = [];
    for (let i = 0; i < numOutcomes; i++) {
      const [conditionalTokenMint] = getConditionalTokenMintAddr(
        this.vaultProgram.programId,
        vault,
        i
      );
      conditionalTokenMintAddrs.push(conditionalTokenMint);
    }

    const vaultUnderlyingTokenAccount = getAssociatedTokenAddressSync(
      underlyingTokenMint,
      vault,
      true
    );

    return this.vaultProgram.methods
      .initializeConditionalVault()
      .accounts({
        vault,
        question,
        underlyingTokenMint,
        vaultUnderlyingTokenAccount,
      })
      .preInstructions([
        createAssociatedTokenAccountIdempotentInstruction(
          this.provider.publicKey,
          vaultUnderlyingTokenAccount,
          vault,
          underlyingTokenMint
        ),
      ])
      .remainingAccounts(
        conditionalTokenMintAddrs.map((conditionalTokenMint) => {
          return {
            pubkey: conditionalTokenMint,
            isWritable: true,
            isSigner: false,
          };
        })
      );
  }

  async initializeVault(
    question: PublicKey,
    underlyingTokenMint: PublicKey,
    numOutcomes: number
  ): Promise<PublicKey> {
    const [vault] = getVaultAddr(
      this.vaultProgram.programId,
      question,
      underlyingTokenMint
    );

    await this.initializeVaultIx(
      question,
      underlyingTokenMint,
      numOutcomes
    ).rpc();

    return vault;
  }

  resolveQuestionIx(
    question: PublicKey,
    oracle: Keypair,
    payoutNumerators: number[]
  ) {
    return this.vaultProgram.methods
      .resolveQuestion({
        payoutNumerators,
      })
      .accounts({
        question,
        oracle: oracle.publicKey,
      })
      .signers([oracle]);
  }

  getConditionalTokenMints(vault: PublicKey, numOutcomes: number): PublicKey[] {
    let conditionalTokenMintAddrs = [];
    for (let i = 0; i < numOutcomes; i++) {
      const [conditionalTokenMint] = getConditionalTokenMintAddr(
        this.vaultProgram.programId,
        vault,
        i
      );
      conditionalTokenMintAddrs.push(conditionalTokenMint);
    }
    return conditionalTokenMintAddrs;
  }

  getRemainingAccounts(
    conditionalTokenMints: PublicKey[],
    userConditionalAccounts: PublicKey[]
  ) {
    return conditionalTokenMints
      .concat(userConditionalAccounts)
      .map((account) => ({
        pubkey: account,
        isWritable: true,
        isSigner: false,
      }));
  }

  // Helper method to get conditional token accounts and instructions
  getConditionalTokenAccountsAndInstructions(
    vault: PublicKey,
    numOutcomes: number,
    user: PublicKey
  ) {
    const conditionalTokenMintAddrs = this.getConditionalTokenMints(
      vault,
      numOutcomes
    );
    const userConditionalAccounts = conditionalTokenMintAddrs.map((mint) =>
      getAssociatedTokenAddressSync(mint, user, true)
    );

    const preInstructions = conditionalTokenMintAddrs.map((mint) =>
      createAssociatedTokenAccountIdempotentInstruction(
        this.provider.publicKey,
        getAssociatedTokenAddressSync(mint, user),
        user,
        mint
      )
    );

    const remainingAccounts = this.getRemainingAccounts(
      conditionalTokenMintAddrs,
      userConditionalAccounts
    );

    return { userConditionalAccounts, preInstructions, remainingAccounts };
  }

  splitTokensIx(
    question: PublicKey,
    vault: PublicKey,
    underlyingTokenMint: PublicKey,
    amount: BN,
    numOutcomes: number,
    user: PublicKey = this.provider.publicKey
  ) {
    const { preInstructions, remainingAccounts } =
      this.getConditionalTokenAccountsAndInstructions(vault, numOutcomes, user);

    return this.vaultProgram.methods
      .splitTokens(amount)
      .accounts({
        question,
        authority: user,
        vault,
        vaultUnderlyingTokenAccount: getAssociatedTokenAddressSync(
          underlyingTokenMint,
          vault,
          true
        ),
        userUnderlyingTokenAccount: getAssociatedTokenAddressSync(
          underlyingTokenMint,
          user,
          true
        ),
      })
      .preInstructions(preInstructions)
      .remainingAccounts(remainingAccounts);
  }

  mergeTokensIx(
    question: PublicKey,
    vault: PublicKey,
    underlyingTokenMint: PublicKey,
    amount: BN,
    numOutcomes: number,
    user: PublicKey = this.provider.publicKey
  ) {
    let conditionalTokenMintAddrs = this.getConditionalTokenMints(
      vault,
      numOutcomes
    );

    let userConditionalAccounts = [];
    for (let conditionalTokenMint of conditionalTokenMintAddrs) {
      userConditionalAccounts.push(
        getAssociatedTokenAddressSync(conditionalTokenMint, user, true)
      );
    }

    let ix = this.vaultProgram.methods
      .mergeTokens(amount)
      .accounts({
        question,
        authority: user,
        vault,
        vaultUnderlyingTokenAccount: getAssociatedTokenAddressSync(
          underlyingTokenMint,
          vault,
          true
        ),
        userUnderlyingTokenAccount: getAssociatedTokenAddressSync(
          underlyingTokenMint,
          user,
          true
        ),
      })
      .preInstructions(
        conditionalTokenMintAddrs.map((conditionalTokenMint) => {
          return createAssociatedTokenAccountIdempotentInstruction(
            this.provider.publicKey,
            getAssociatedTokenAddressSync(conditionalTokenMint, user),
            user,
            conditionalTokenMint
          );
        })
      )
      .remainingAccounts(
        conditionalTokenMintAddrs
          .concat(userConditionalAccounts)
          .map((conditionalTokenMint) => {
            return {
              pubkey: conditionalTokenMint,
              isWritable: true,
              isSigner: false,
            };
          })
      );

    return ix;
  }

  redeemTokensIx(
    question: PublicKey,
    vault: PublicKey,
    underlyingTokenMint: PublicKey,
    numOutcomes: number,
    user: PublicKey = this.provider.publicKey
  ) {
    let conditionalTokenMintAddrs = [];
    for (let i = 0; i < numOutcomes; i++) {
      const [conditionalTokenMint] = getConditionalTokenMintAddr(
        this.vaultProgram.programId,
        vault,
        i
      );
      conditionalTokenMintAddrs.push(conditionalTokenMint);
    }

    let userConditionalAccounts = [];
    for (let conditionalTokenMint of conditionalTokenMintAddrs) {
      userConditionalAccounts.push(
        getAssociatedTokenAddressSync(conditionalTokenMint, user, true)
      );
    }

    let ix = this.vaultProgram.methods
      .redeemTokens()
      .accounts({
        question,
        authority: user,
        vault,
        vaultUnderlyingTokenAccount: getAssociatedTokenAddressSync(
          underlyingTokenMint,
          vault,
          true
        ),
        userUnderlyingTokenAccount: getAssociatedTokenAddressSync(
          underlyingTokenMint,
          user,
          true
        ),
      })
      .preInstructions(
        conditionalTokenMintAddrs.map((conditionalTokenMint) => {
          return createAssociatedTokenAccountIdempotentInstruction(
            this.provider.publicKey,
            getAssociatedTokenAddressSync(conditionalTokenMint, user),
            user,
            conditionalTokenMint
          );
        })
      )
      .remainingAccounts(
        conditionalTokenMintAddrs
          .concat(userConditionalAccounts)
          .map((conditionalTokenMint) => {
            return {
              pubkey: conditionalTokenMint,
              isWritable: true,
              isSigner: false,
            };
          })
      );

    return ix;
  }

  addMetadataToConditionalTokensIx(
    vault: PublicKey,
    index: number,
    name: string,
    symbol: string,
    image: string
    // underlyingTokenMint: PublicKey,
    // proposalNumber: number,
    // onFinalizeUri: string,
    // onRevertUri: string
  ) {
    // const [underlyingTokenMetadata] = getMetadataAddr(underlyingTokenMint);

    const [conditionalTokenMint] = getConditionalTokenMintAddr(
      this.vaultProgram.programId,
      vault,
      index
    );

    // const [conditionalOnFinalizeTokenMint] = getVaultFinalizeMintAddr(
    //   this.vaultProgram.programId,
    //   vault
    // );
    // const [conditionalOnRevertTokenMint] = getVaultRevertMintAddr(
    //   this.vaultProgram.programId,
    //   vault
    // );

    const [conditionalTokenMetadata] = getMetadataAddr(conditionalTokenMint);

    // const [conditionalOnRevertTokenMetadata] = getMetadataAddr(
    //   conditionalOnRevertTokenMint
    // );

    return this.vaultProgram.methods
      .addMetadataToConditionalTokens({
        name,
        symbol,
        image,
      })
      .accounts({
        payer: this.provider.publicKey,
        vault,
        conditionalTokenMint,
        conditionalTokenMetadata,
        tokenMetadataProgram: MPL_TOKEN_METADATA_PROGRAM_ID,
      });
  }
}
