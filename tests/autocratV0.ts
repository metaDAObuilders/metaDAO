import * as anchor from "@coral-xyz/anchor";
import { BN, Program } from "@coral-xyz/anchor";
import * as token from "@solana/spl-token";
import { BankrunProvider } from "anchor-bankrun";
import {
  mintConditionalTokens,
  redeemConditionalTokens,
} from "./conditionalVault";
import {
  OpenBookV2Client,
  IDL,
  PlaceOrderArgs,
  Side,
  OrderType,
  SelfTradeBehavior,
} from "@openbook-dex/openbook-v2";

const { PublicKey, Keypair } = anchor.web3;

import { assert } from "chai";

import {
  startAnchor,
  Clock,
  BanksClient,
  ProgramTestContext,
} from "solana-bankrun";

import { expectError } from "./utils/utils";

import { AutocratV0 } from "../target/types/autocrat_v0";
import { ConditionalVault } from "../target/types/conditional_vault";
import { AutocratMigrator } from "../target/types/autocrat_migrator";
import { OpenbookTwap } from "./fixtures/openbook_twap";

const OpenbookTwapIDL: OpenbookTwap = require("./fixtures/openbook_twap.json");

const AutocratIDL: AutocratV0 = require("../target/idl/autocrat_v0.json");
const ConditionalVaultIDL: ConditionalVault = require("../target/idl/conditional_vault.json");
const AutocratMigratorIDL: AutocratMigrator = require("../target/idl/autocrat_migrator.json");

export type PublicKey = anchor.web3.PublicKey;
export type Signer = anchor.web3.Signer;
export type Keypair = anchor.web3.Keypair;

interface MarketMaker {
  publicKey: PublicKey;
  keypair: Keypair;
  pOpenOrdersAccount: PublicKey;
  fOpenOrdersAccount: PublicKey;
  pMetaAcc: PublicKey;
  fMetaAcc: PublicKey;
  pUsdcAcc: PublicKey;
  fUsdcAcc: PublicKey;
}

type ProposalInstruction = anchor.IdlTypes<AutocratV0>["ProposalInstruction"];

import {
  createMint,
  createAccount,
  createAssociatedTokenAccount,
  mintToOverride,
  getMint,
  getAccount,
} from "spl-token-bankrun";
import { open } from "fs";

// this test file isn't 'clean' or DRY or whatever; sorry!
const AUTOCRAT_PROGRAM_ID = new PublicKey(
  "metaX99LHn3A7Gr7VAcCfXhpfocvpMpqQ3eyp3PGUUq"
);

const CONDITIONAL_VAULT_PROGRAM_ID = new PublicKey(
  "vaU1tVLj8RFk7mNj1BxqgAsMKKaL8UvEUHvU3tdbZPe"
);

const OPENBOOK_TWAP_PROGRAM_ID = new PublicKey(
  "TWAPrdhADy2aTKN5iFZtNnkQYXERD9NvKjPFVPMSCNN"
);

const OPENBOOK_PROGRAM_ID = new PublicKey(
  "opnb2LAfJYbRMAHHvqjCwQxanZn7ReEHp1k81EohpZb"
);

const AUTOCRAT_MIGRATOR_PROGRAM_ID = new PublicKey(
  "migkwAXrXFN34voCYQUhFQBXZJjHrWnpEXbSGTqZdB3"
);

describe("autocrat_v0", async function () {
  let provider,
    connection,
    autocrat,
    payer,
    context,
    banksClient,
    dao,
    daoTreasury,
    META,
    USDC,
    vaultProgram,
    openbook,
    openbookTwap,
    migrator,
    treasuryMetaAccount,
    treasuryUsdcAccount;

  before(async function () {
    context = await startAnchor(
      "./",
      [
        {
          name: "openbook_v2",
          programId: OPENBOOK_PROGRAM_ID,
        },
        {
          name: "openbook_twap",
          programId: OPENBOOK_TWAP_PROGRAM_ID,
        },
      ],
      []
    );
    banksClient = context.banksClient;
    provider = new BankrunProvider(context);
    anchor.setProvider(provider);

    autocrat = new anchor.Program<AutocratV0>(
      AutocratIDL,
      AUTOCRAT_PROGRAM_ID,
      provider
    );
    openbook = new OpenBookV2Client(provider);
    openbookTwap = new Program<OpenbookTwap>(
      OpenbookTwapIDL,
      OPENBOOK_TWAP_PROGRAM_ID,
      provider
    );

    vaultProgram = new Program<ConditionalVault>(
      ConditionalVaultIDL,
      CONDITIONAL_VAULT_PROGRAM_ID,
      provider
    );

    migrator = new anchor.Program<AutocratMigrator>(
      AutocratMigratorIDL,
      AUTOCRAT_MIGRATOR_PROGRAM_ID,
      provider
    );

    payer = autocrat.provider.wallet.payer;

    USDC = await createMint(
      banksClient,
      payer,
      payer.publicKey,
      payer.publicKey,
      6
    );

    META = await createMint(banksClient, payer, dao, dao, 9);
  });

  describe("#initialize_dao", async function () {
    it("initializes the DAO", async function () {
      [dao] = PublicKey.findProgramAddressSync(
        [anchor.utils.bytes.utf8.encode("WWCACOTMICMIBMHAFTTWYGHMB")],
        autocrat.programId
      );
      [daoTreasury] = PublicKey.findProgramAddressSync(
        [dao.toBuffer()],
        autocrat.programId
      );

      await autocrat.methods
        .initializeDao()
        .accounts({
          dao,
          payer: payer.publicKey,
          systemProgram: anchor.web3.SystemProgram.programId,
          metaMint: META,
          usdcMint: USDC,
        })
        .rpc()
        .then(
          () => {},
          (err) => console.error(err)
        );

      const daoAcc = await autocrat.account.dao.fetch(dao);
      assert(daoAcc.metaMint.equals(META));
      assert(daoAcc.usdcMint.equals(USDC));
      assert.equal(daoAcc.proposalCount, 0);
      assert.equal(daoAcc.passThresholdBps, 500);
      assert.ok(daoAcc.baseBurnLamports.eq(new BN(1_000_000_000).muln(10)));
      assert.ok(daoAcc.burnDecayPerSlotLamports.eq(new BN(23_150)));

      treasuryMetaAccount = await createAssociatedTokenAccount(
        banksClient,
        payer,
        META,
        daoTreasury
      );
      treasuryUsdcAccount = await createAssociatedTokenAccount(
        banksClient,
        payer,
        USDC,
        daoTreasury
      );
    });
  });

  describe("#initialize_proposal", async function () {
    it("initializes proposals", async function () {
      const accounts = [
        {
          pubkey: dao,
          isSigner: true,
          isWritable: true,
        },
      ];
      const data = autocrat.coder.instruction.encode("update_dao", {
        daoParams: {
          passThresholdBps: 500,
          baseBurnLamports: null,
          burnDecayPerSlotLamports: null,
          slotsPerProposal: null,
          marketTakerFee: null,
        }
      });
      const instruction = {
        programId: autocrat.programId,
        accounts,
        data,
      };

      let currentClock = await context.banksClient.getClock();
      let newSlot = currentClock.slot + 216_000n; // 1 day
      context.setClock(
        new Clock(
          newSlot,
          currentClock.epochStartTimestamp,
          currentClock.epoch,
          currentClock.leaderScheduleEpoch,
          currentClock.unixTimestamp
        )
      );

      let balanceBefore = await banksClient.getBalance(payer.publicKey);

      await initializeProposal(
        autocrat,
        instruction,
        vaultProgram,
        dao,
        context,
        payer,
        openbook,
        openbookTwap
      );

      let balanceAfter = await banksClient.getBalance(payer.publicKey);

      // two days, so proposer should burn 5 SOL
      assert(balanceAfter < balanceBefore - 1_000_000_000n * 5n);

      assert(balanceAfter > balanceBefore - 1_000_000_000n * 10n);
    });
  });

  describe("#finalize_proposal", async function () {
    let proposal,
      openbookPassMarket,
      openbookFailMarket,
      openbookTwapPassMarket,
      openbookTwapFailMarket,
      baseVault,
      quoteVault,
      basePassVaultUnderlyingTokenAccount,
      basePassConditionalTokenMint,
      baseFailConditionalTokenMint,
      mm0,
      mm1,
      mm0OpenOrdersAccount,
      mm1OpenOrdersAccount,
      alice,
      aliceUnderlyingQuoteTokenAccount,
      aliceUnderlyingBaseTokenAccount,
      aliceBasePassConditionalTokenAccount,
      aliceBaseFailConditionalTokenAccount,
      aliceQuotePassConditionalTokenAccount,
      aliceQuoteFailConditionalTokenAccount,
      newPassThresholdBps,
      instruction;

    beforeEach(async function () {
      // just uncomment this and replace with another instruction that you wish to test
      // const accounts = [
      //   {
      //     pubkey: dao,
      //     isSigner: false,
      //     isWritable: true,
      //   },
      //   {
      //     pubkey: daoTreasury,
      //     isSigner: true,
      //     isWritable: false,
      //   },
      // ];
      // newPassThresholdBps = Math.floor(Math.random() * 1000);
      // const data = autocrat.coder.instruction.encode("update_dao", {
      //   daoParams: {
      //     passThresholdBps: newPassThresholdBps,
      //     baseBurnLamports: null,
      //     burnDecayPerSlotLamports: null,
      //     slotsPerProposal: null,
      //     marketTakerFee: null,
      //     twapExpectedValue: null,
      //   }
      // });
      // instruction = {
      //   programId: autocrat.programId,
      //   accounts,
      //   data,
      // };
      // let lamportReceiver = Keypair.generate();

      // let ix = anchor.web3.SystemProgram.transfer({
      //   fromPubkey: daoTreasury,
      //   toPubkey: lamportReceiver.publicKey,
      //   lamports: 1_000_000,
      // });
      
      // instruction = {
      //   programId: ix.programId,
      //   accounts: ix.keys,
      //   data: ix.data,
      // };

      await mintToOverride(context, treasuryMetaAccount, 1_000_000_000n);
      await mintToOverride(context, treasuryUsdcAccount, 1_000_000n);

      let receiver = Keypair.generate();
      let to0 = await createAccount(
        banksClient,
        payer,
        META,
        receiver.publicKey
      );
      let to1 = await createAccount(
        banksClient,
        payer,
        USDC,
        receiver.publicKey
      );

      const ix = await migrator.methods
        .multiTransfer2()
        .accounts({
          authority: daoTreasury,
          from0: treasuryMetaAccount,
          to0,
          from1: treasuryUsdcAccount,
          to1,
          lamportReceiver: receiver.publicKey,
        })
        .instruction();

      instruction = {
        programId: ix.programId,
        accounts: ix.keys,
        data: ix.data,
      };

      proposal = await initializeProposal(
        autocrat,
        instruction,
        vaultProgram,
        dao,
        context,
        payer,
        openbook,
        openbookTwap
      );

      ({
        openbookPassMarket,
        openbookFailMarket,
        openbookTwapPassMarket,
        openbookTwapFailMarket,
        baseVault,
        quoteVault,
      } = await autocrat.account.proposal.fetch(proposal));

      mm0 = await generateMarketMaker(
        openbook,
        openbookTwap,
        banksClient,
        payer,
        openbookPassMarket,
        openbookFailMarket,
        vaultProgram,
        context
      );

      mm1 = await generateMarketMaker(
        openbook,
        openbookTwap,
        banksClient,
        payer,
        openbookPassMarket,
        openbookFailMarket,
        vaultProgram,
        context
      );

      // alice wants to buy META if the proposal passes, so she locks up USDC
      // and swaps her pUSDC for pMETA
      alice = Keypair.generate();

      // needed because of penalty fee on take
      let ixs = [
        anchor.web3.SystemProgram.transfer({
          fromPubkey: payer.publicKey,
          lamports: 1_000_000_000n,
          toPubkey: alice.publicKey,
        }),
        anchor.web3.SystemProgram.transfer({
          fromPubkey: payer.publicKey,
          lamports: 1_000_000_000n,
          toPubkey: daoTreasury,
        }),
      ];
      let tx = new anchor.web3.Transaction().add(...ixs);
      [tx.recentBlockhash] = await banksClient.getLatestBlockhash();
      tx.feePayer = payer.publicKey;
      tx.sign(payer);
      await banksClient.processTransaction(tx);

      const storedQuoteVault =
        await vaultProgram.account.conditionalVault.fetch(quoteVault);
      const quoteVaultUnderlyingTokenAccount =
        storedQuoteVault.underlyingTokenAccount;
      const quotePassConditionalTokenMint =
        storedQuoteVault.conditionalOnFinalizeTokenMint;
      const quoteFailConditionalTokenMint =
        storedQuoteVault.conditionalOnRevertTokenMint;

      const storedBaseVault = await vaultProgram.account.conditionalVault.fetch(
        baseVault
      );
      basePassConditionalTokenMint =
        storedBaseVault.conditionalOnFinalizeTokenMint;
      baseFailConditionalTokenMint =
        storedBaseVault.conditionalOnRevertTokenMint;

      aliceUnderlyingQuoteTokenAccount = await createAssociatedTokenAccount(
        banksClient,
        payer,
        USDC,
        alice.publicKey
      );
      aliceUnderlyingBaseTokenAccount = await createAssociatedTokenAccount(
        banksClient,
        payer,
        META,
        alice.publicKey
      );

      await mintToOverride(
        context,
        aliceUnderlyingQuoteTokenAccount,
        10_000n * 1_000_000n
      );

      aliceQuotePassConditionalTokenAccount =
        await createAssociatedTokenAccount(
          banksClient,
          payer,
          quotePassConditionalTokenMint,
          alice.publicKey
        );
      aliceQuoteFailConditionalTokenAccount =
        await createAssociatedTokenAccount(
          banksClient,
          payer,
          quoteFailConditionalTokenMint,
          alice.publicKey
        );

      await mintConditionalTokens(
        vaultProgram,
        10_000n * 1_000_000n,
        alice,
        quoteVault,
        banksClient
      );

      aliceBasePassConditionalTokenAccount = await createAssociatedTokenAccount(
        banksClient,
        payer,
        basePassConditionalTokenMint,
        alice.publicKey
      );

      aliceBaseFailConditionalTokenAccount = await createAssociatedTokenAccount(
        banksClient,
        payer,
        baseFailConditionalTokenMint,
        alice.publicKey
      );
    });

    it("doesn't finalize proposals that are too young", async function () {
      const callbacks = expectError(
        autocrat,
        "ProposalTooYoung",
        "finalize succeeded despite proposal being too young"
      );

      await autocrat.methods
        .finalizeProposal()
        .accounts({
          proposal,
          openbookTwapPassMarket,
          openbookTwapFailMarket,
          dao,
          baseVault,
          quoteVault,
          vaultProgram: vaultProgram.programId,
          daoTreasury,
        })
        .rpc()
        .then(callbacks[0], callbacks[1]);
    });

    it("finalizes proposals when pass price TWAP > (fail price TWAP + threshold)", async function () {
      let storedProposal = await autocrat.account.proposal.fetch(proposal);

      let passBuyArgs: PlaceOrderArgs = {
        side: Side.Bid,
        priceLots: new BN(10000), // 1 USDC for 1 META
        maxBaseLots: new BN(10),
        maxQuoteLotsIncludingFees: new BN(10000),
        clientOrderId: new BN(1),
        orderType: OrderType.Limit,
        expiryTimestamp: new BN(0),
        selfTradeBehavior: SelfTradeBehavior.DecrementTake,
        limit: 255,
      };
      let failBuyArgs: PlaceOrderArgs = {
        side: Side.Bid,
        priceLots: new BN(7000), // 0.7 USDC for 1 META
        maxBaseLots: new BN(10),
        maxQuoteLotsIncludingFees: new BN(10000),
        clientOrderId: new BN(1),
        orderType: OrderType.Limit,
        expiryTimestamp: new BN(0),
        selfTradeBehavior: SelfTradeBehavior.DecrementTake,
        limit: 255,
      };

      let passSellArgs: PlaceOrderArgs = {
        side: Side.Ask,
        priceLots: new BN(11_000), // 1.1 USDC for 1 META
        maxBaseLots: new BN(10),
        maxQuoteLotsIncludingFees: new BN(12000),
        clientOrderId: new BN(2),
        orderType: OrderType.Limit,
        expiryTimestamp: new BN(0),
        selfTradeBehavior: SelfTradeBehavior.DecrementTake,
        limit: 255,
      };
      let failSellArgs: PlaceOrderArgs = {
        side: Side.Ask,
        priceLots: new BN(8_000), // 8 USDC for 1 META
        maxBaseLots: new BN(10),
        maxQuoteLotsIncludingFees: new BN(12000),
        clientOrderId: new BN(2),
        orderType: OrderType.Limit,
        expiryTimestamp: new BN(0),
        selfTradeBehavior: SelfTradeBehavior.DecrementTake,
        limit: 255,
      };
      const storedPassMarket = await openbook.getMarketAccount(
        openbookPassMarket
      );
      const storedFailMarket = await openbook.getMarketAccount(
        openbookFailMarket
      );

      let currentClock;
      for (let i = 0; i < 10; i++) {
        await openbookTwap.methods
          .placeOrder(passBuyArgs)
          .accounts({
            signer: mm0.publicKey,
            market: openbookPassMarket,
            asks: storedPassMarket.asks,
            bids: storedPassMarket.bids,
            marketVault: storedPassMarket.marketQuoteVault,
            eventHeap: storedPassMarket.eventHeap,
            openOrdersAccount: mm0.pOpenOrdersAccount,
            userTokenAccount: mm0.pUsdcAcc,
            twapMarket: openbookTwapPassMarket,
            openbookProgram: OPENBOOK_PROGRAM_ID,
          })
          .signers([mm0.keypair])
          .rpc();

        await openbookTwap.methods
          .placeOrder(passSellArgs)
          .accounts({
            signer: mm0.publicKey,
            market: openbookPassMarket,
            asks: storedPassMarket.asks,
            bids: storedPassMarket.bids,
            marketVault: storedPassMarket.marketBaseVault,
            eventHeap: storedPassMarket.eventHeap,
            openOrdersAccount: mm0.pOpenOrdersAccount,
            userTokenAccount: mm0.pMetaAcc,
            twapMarket: openbookTwapPassMarket,
            openbookProgram: OPENBOOK_PROGRAM_ID,
          })
          .signers([mm0.keypair])
          .rpc();

        await openbookTwap.methods
          .placeOrder(failBuyArgs)
          .accounts({
            signer: mm0.publicKey,
            market: openbookFailMarket,
            asks: storedFailMarket.asks,
            bids: storedFailMarket.bids,
            marketVault: storedFailMarket.marketQuoteVault,
            eventHeap: storedFailMarket.eventHeap,
            openOrdersAccount: mm0.fOpenOrdersAccount,
            userTokenAccount: mm0.fUsdcAcc,
            twapMarket: openbookTwapFailMarket,
            openbookProgram: OPENBOOK_PROGRAM_ID,
          })
          .signers([mm0.keypair])
          .rpc();

        await openbookTwap.methods
          .placeOrder(failSellArgs)
          .accounts({
            signer: mm0.publicKey,
            market: openbookFailMarket,
            asks: storedFailMarket.asks,
            bids: storedFailMarket.bids,
            marketVault: storedFailMarket.marketBaseVault,
            eventHeap: storedFailMarket.eventHeap,
            openOrdersAccount: mm0.fOpenOrdersAccount,
            userTokenAccount: mm0.fMetaAcc,
            twapMarket: openbookTwapFailMarket,
            openbookProgram: OPENBOOK_PROGRAM_ID,
          })
          .signers([mm0.keypair])
          .rpc();
        currentClock = await context.banksClient.getClock();
        context.setClock(
          new Clock(
            currentClock.slot + 10_000n,
            currentClock.epochStartTimestamp,
            currentClock.epoch,
            currentClock.leaderScheduleEpoch,
            currentClock.unixTimestamp
          )
        );
      }

      // set the current clock slot to +10_000
      currentClock = await context.banksClient.getClock();
      context.setClock(
        new Clock(
          currentClock.slot + 10_000n,
          currentClock.epochStartTimestamp,
          currentClock.epoch,
          currentClock.leaderScheduleEpoch,
          currentClock.unixTimestamp
        )
      );

      let takeBuyArgs: PlaceOrderArgs = {
        side: Side.Bid,
        priceLots: new BN(13000), // 13 USDC for 1 META
        maxBaseLots: new BN(1),
        maxQuoteLotsIncludingFees: new BN(20000),
        clientOrderId: new BN(1),
        orderType: OrderType.Market,
        expiryTimestamp: new BN(0),
        selfTradeBehavior: SelfTradeBehavior.DecrementTake,
        limit: 255,
      };

      await openbookTwap.methods
        .placeTakeOrder(takeBuyArgs)
        .accountsStrict({
          market: openbookPassMarket,
          asks: storedPassMarket.asks,
          bids: storedPassMarket.bids,
          eventHeap: storedPassMarket.eventHeap,
          marketAuthority: storedPassMarket.marketAuthority,
          marketBaseVault: storedPassMarket.marketBaseVault,
          marketQuoteVault: storedPassMarket.marketQuoteVault,
          userQuoteAccount: aliceQuotePassConditionalTokenAccount,
          userBaseAccount: aliceBasePassConditionalTokenAccount,
          referrerAccount: null,
          twapMarket: openbookTwapPassMarket,
          openbookProgram: OPENBOOK_PROGRAM_ID,
          tokenProgram: token.TOKEN_PROGRAM_ID,
          systemProgram: anchor.web3.SystemProgram.programId,
          signer: alice.publicKey,
        })
        .signers([alice])
        .rpc();

      currentClock = await context.banksClient.getClock();
      const newSlot = currentClock.slot + 10_000_000n;
      context.setClock(
        new Clock(
          newSlot,
          currentClock.epochStartTimestamp,
          currentClock.epoch,
          currentClock.leaderScheduleEpoch,
          currentClock.unixTimestamp
        )
      );

      await openbookTwap.methods
        .placeOrder(passBuyArgs)
        .accounts({
          signer: mm0.publicKey,
          market: openbookPassMarket,
          asks: storedPassMarket.asks,
          bids: storedPassMarket.bids,
          marketVault: storedPassMarket.marketQuoteVault,
          eventHeap: storedPassMarket.eventHeap,
          openOrdersAccount: mm0.pOpenOrdersAccount,
          userTokenAccount: mm0.pUsdcAcc,
          twapMarket: openbookTwapPassMarket,
          openbookProgram: OPENBOOK_PROGRAM_ID,
        })
        .signers([mm0.keypair])
        .rpc();

      await openbookTwap.methods
        .placeOrder(passSellArgs)
        .accounts({
          signer: mm0.publicKey,
          market: openbookPassMarket,
          asks: storedPassMarket.asks,
          bids: storedPassMarket.bids,
          marketVault: storedPassMarket.marketBaseVault,
          eventHeap: storedPassMarket.eventHeap,
          openOrdersAccount: mm0.pOpenOrdersAccount,
          userTokenAccount: mm0.pMetaAcc,
          twapMarket: openbookTwapPassMarket,
          openbookProgram: OPENBOOK_PROGRAM_ID,
        })
        .signers([mm0.keypair])
        .rpc();

      await openbookTwap.methods
        .placeOrder(failBuyArgs)
        .accounts({
          signer: mm0.publicKey,
          market: openbookFailMarket,
          asks: storedFailMarket.asks,
          bids: storedFailMarket.bids,
          marketVault: storedFailMarket.marketQuoteVault,
          eventHeap: storedFailMarket.eventHeap,
          openOrdersAccount: mm0.fOpenOrdersAccount,
          userTokenAccount: mm0.fUsdcAcc,
          twapMarket: openbookTwapFailMarket,
          openbookProgram: OPENBOOK_PROGRAM_ID,
        })
        .signers([mm0.keypair])
        .rpc();

      await openbookTwap.methods
        .placeOrder(failSellArgs)
        .accounts({
          signer: mm0.publicKey,
          market: openbookFailMarket,
          asks: storedFailMarket.asks,
          bids: storedFailMarket.bids,
          marketVault: storedFailMarket.marketBaseVault,
          eventHeap: storedFailMarket.eventHeap,
          openOrdersAccount: mm0.fOpenOrdersAccount,
          userTokenAccount: mm0.fMetaAcc,
          twapMarket: openbookTwapFailMarket,
          openbookProgram: OPENBOOK_PROGRAM_ID,
        })
        .signers([mm0.keypair])
        .rpc();

      let tx = await autocrat.methods
        .finalizeProposal()
        .accounts({
          proposal,
          openbookTwapPassMarket,
          openbookTwapFailMarket,
          dao,
          baseVault,
          quoteVault,
          vaultProgram: vaultProgram.programId,
          daoTreasury,
        })
        .remainingAccounts(
          instruction.accounts
            .concat({
              pubkey: instruction.programId,
              isWritable: false,
              isSigner: false,
            })
            .map((meta) =>
              meta.pubkey.equals(daoTreasury)
                ? { ...meta, isSigner: false }
                : meta
            )
        )
        .rpc();

      let storedBaseVault = await vaultProgram.account.conditionalVault.fetch(
        baseVault
      );
      let storedQuoteVault = await vaultProgram.account.conditionalVault.fetch(
        quoteVault
      );

      assert.exists(storedBaseVault.status.finalized);
      assert.exists(storedQuoteVault.status.finalized);

      storedProposal = await autocrat.account.proposal.fetch(proposal);
      assert.exists(storedProposal.state.passed);

      assert((await getAccount(banksClient, treasuryMetaAccount)).amount == 0n);
      assert((await getAccount(banksClient, treasuryUsdcAccount)).amount == 0n);

      // console.log(await banksClient.getAccount(daoTreasury));

      // const storedDao = await autocrat.account.dao.fetch(dao);
      // assert.equal(storedDao.passThresholdBps, newPassThresholdBps);

      await redeemConditionalTokens(
        vaultProgram,
        alice,
        aliceBasePassConditionalTokenAccount,
        aliceBaseFailConditionalTokenAccount,
        storedBaseVault.conditionalOnFinalizeTokenMint,
        storedBaseVault.conditionalOnRevertTokenMint,
        aliceUnderlyingBaseTokenAccount,
        storedBaseVault.underlyingTokenAccount,
        baseVault,
        banksClient
      );
      await redeemConditionalTokens(
        vaultProgram,
        alice,
        aliceQuotePassConditionalTokenAccount,
        aliceQuoteFailConditionalTokenAccount,
        storedQuoteVault.conditionalOnFinalizeTokenMint,
        storedQuoteVault.conditionalOnRevertTokenMint,
        aliceUnderlyingQuoteTokenAccount,
        storedQuoteVault.underlyingTokenAccount,
        quoteVault,
        banksClient
      );

      // alice should have gained 1 META & lost 0.11 USDC
      assert.equal(
        (await getAccount(banksClient, aliceUnderlyingBaseTokenAccount)).amount,
        1_000_000_000n
      );
      assert.equal(
        (await getAccount(banksClient, aliceUnderlyingQuoteTokenAccount))
          .amount,
        10_000n * 1_000_000n - 11_000n
      );
    });

    it("rejects proposals when pass price TWAP < fail price TWAP", async function () {
      let storedProposal = await autocrat.account.proposal.fetch(proposal);

      let passBuyArgs: PlaceOrderArgs = {
        side: Side.Bid,
        priceLots: new BN(1000), // 0.1 USDC for 1 META
        maxBaseLots: new BN(10),
        maxQuoteLotsIncludingFees: new BN(10000),
        clientOrderId: new BN(1),
        orderType: OrderType.Limit,
        expiryTimestamp: new BN(0),
        selfTradeBehavior: SelfTradeBehavior.DecrementTake,
        limit: 255,
      };
      let failBuyArgs: PlaceOrderArgs = {
        side: Side.Bid,
        priceLots: new BN(3000), // 0.3 USDC for 1 META
        maxBaseLots: new BN(10),
        maxQuoteLotsIncludingFees: new BN(10000),
        clientOrderId: new BN(1),
        orderType: OrderType.Limit,
        expiryTimestamp: new BN(0),
        selfTradeBehavior: SelfTradeBehavior.DecrementTake,
        limit: 255,
      };

      let passSellArgs: PlaceOrderArgs = {
        side: Side.Ask,
        priceLots: new BN(1100), // 0.11 USDC for 1 META
        maxBaseLots: new BN(10),
        maxQuoteLotsIncludingFees: new BN(12000),
        clientOrderId: new BN(2),
        orderType: OrderType.Limit,
        expiryTimestamp: new BN(0),
        selfTradeBehavior: SelfTradeBehavior.DecrementTake,
        limit: 255,
      };

      let failSellArgs: PlaceOrderArgs = {
        side: Side.Ask,
        priceLots: new BN(3200), // 0.32 USDC for 1 META
        maxBaseLots: new BN(10),
        maxQuoteLotsIncludingFees: new BN(12000),
        clientOrderId: new BN(2),
        orderType: OrderType.Limit,
        expiryTimestamp: new BN(0),
        selfTradeBehavior: SelfTradeBehavior.DecrementTake,
        limit: 255,
      };
      const storedPassMarket = await openbook.getMarketAccount(
        openbookPassMarket
      );
      const storedFailMarket = await openbook.getMarketAccount(
        openbookFailMarket
      );

      let currentClock;
      for (let i = 0; i < 10; i++) {
        await openbookTwap.methods
          .placeOrder(passBuyArgs)
          .accounts({
            signer: mm0.publicKey,
            market: openbookPassMarket,
            asks: storedPassMarket.asks,
            bids: storedPassMarket.bids,
            marketVault: storedPassMarket.marketQuoteVault,
            eventHeap: storedPassMarket.eventHeap,
            openOrdersAccount: mm0.pOpenOrdersAccount,
            userTokenAccount: mm0.pUsdcAcc,
            twapMarket: openbookTwapPassMarket,
            openbookProgram: OPENBOOK_PROGRAM_ID,
          })
          .signers([mm0.keypair])
          .rpc();

        await openbookTwap.methods
          .placeOrder(passSellArgs)
          .accounts({
            signer: mm0.publicKey,
            market: openbookPassMarket,
            asks: storedPassMarket.asks,
            bids: storedPassMarket.bids,
            marketVault: storedPassMarket.marketBaseVault,
            eventHeap: storedPassMarket.eventHeap,
            openOrdersAccount: mm0.pOpenOrdersAccount,
            userTokenAccount: mm0.pMetaAcc,
            twapMarket: openbookTwapPassMarket,
            openbookProgram: OPENBOOK_PROGRAM_ID,
          })
          .signers([mm0.keypair])
          .rpc();

        await openbookTwap.methods
          .placeOrder(failBuyArgs)
          .accounts({
            signer: mm0.publicKey,
            market: openbookFailMarket,
            asks: storedFailMarket.asks,
            bids: storedFailMarket.bids,
            marketVault: storedFailMarket.marketQuoteVault,
            eventHeap: storedFailMarket.eventHeap,
            openOrdersAccount: mm0.fOpenOrdersAccount,
            userTokenAccount: mm0.fUsdcAcc,
            twapMarket: openbookTwapFailMarket,
            openbookProgram: OPENBOOK_PROGRAM_ID,
          })
          .signers([mm0.keypair])
          .rpc();

        await openbookTwap.methods
          .placeOrder(failSellArgs)
          .accounts({
            signer: mm0.publicKey,
            market: openbookFailMarket,
            asks: storedFailMarket.asks,
            bids: storedFailMarket.bids,
            marketVault: storedFailMarket.marketBaseVault,
            eventHeap: storedFailMarket.eventHeap,
            openOrdersAccount: mm0.fOpenOrdersAccount,
            userTokenAccount: mm0.fMetaAcc,
            twapMarket: openbookTwapFailMarket,
            openbookProgram: OPENBOOK_PROGRAM_ID,
          })
          .signers([mm0.keypair])
          .rpc();
        currentClock = await context.banksClient.getClock();
        context.setClock(
          new Clock(
            currentClock.slot + 10_000n,
            currentClock.epochStartTimestamp,
            currentClock.epoch,
            currentClock.leaderScheduleEpoch,
            currentClock.unixTimestamp
          )
        );
      }

      // /crank_that_twap
      for (let i = 0; i < 1000; i++) {
        let crankArgs: PlaceOrderArgs = {
            side: Side.Bid,
            priceLots: new BN(1), 
            maxBaseLots: new BN(i),
            maxQuoteLotsIncludingFees: new BN(0),
            clientOrderId: new BN(2),
            orderType: OrderType.Limit,
            expiryTimestamp: new BN(0),
            selfTradeBehavior: SelfTradeBehavior.DecrementTake,
            limit: 255,
          };

        await openbookTwap.methods
          .placeOrder(crankArgs)
          .accounts({
            signer: mm0.publicKey,
            market: openbookPassMarket,
            asks: storedPassMarket.asks,
            bids: storedPassMarket.bids,
            marketVault: storedPassMarket.marketQuoteVault,
            eventHeap: storedPassMarket.eventHeap,
            openOrdersAccount: mm0.pOpenOrdersAccount,
            userTokenAccount: mm0.pUsdcAcc,
            twapMarket: openbookTwapPassMarket,
            openbookProgram: OPENBOOK_PROGRAM_ID,
          })
          .signers([mm0.keypair])
          .rpc();

        // set the current clock slot to +10_000
        currentClock = await context.banksClient.getClock();
        context.setClock(
          new Clock(
            currentClock.slot + 10_000n,
            currentClock.epochStartTimestamp,
            currentClock.epoch,
            currentClock.leaderScheduleEpoch,
            currentClock.unixTimestamp
          )
        );
      }

      let takeBuyArgs: PlaceOrderArgs = {
        side: Side.Bid,
        priceLots: new BN(1_300), // 1.3 USDC for 1 META
        maxBaseLots: new BN(1),
        maxQuoteLotsIncludingFees: new BN(2000),
        clientOrderId: new BN(1),
        orderType: OrderType.Market,
        expiryTimestamp: new BN(0),
        selfTradeBehavior: SelfTradeBehavior.DecrementTake,
        limit: 255,
      };

      await openbookTwap.methods
        .placeTakeOrder(takeBuyArgs)
        .accountsStrict({
          market: openbookPassMarket,
          asks: storedPassMarket.asks,
          bids: storedPassMarket.bids,
          eventHeap: storedPassMarket.eventHeap,
          marketAuthority: storedPassMarket.marketAuthority,
          marketBaseVault: storedPassMarket.marketBaseVault,
          marketQuoteVault: storedPassMarket.marketQuoteVault,
          userQuoteAccount: aliceQuotePassConditionalTokenAccount,
          userBaseAccount: aliceBasePassConditionalTokenAccount,
          referrerAccount: null,
          twapMarket: openbookTwapPassMarket,
          openbookProgram: OPENBOOK_PROGRAM_ID,
          tokenProgram: token.TOKEN_PROGRAM_ID,
          systemProgram: anchor.web3.SystemProgram.programId,
          signer: alice.publicKey,
        })
        .signers([alice])
        .rpc();

      currentClock = await context.banksClient.getClock();
      const newSlot = currentClock.slot + 10_000_000n;
      context.setClock(
        new Clock(
          newSlot,
          currentClock.epochStartTimestamp,
          currentClock.epoch,
          currentClock.leaderScheduleEpoch,
          currentClock.unixTimestamp
        )
      );

      await openbookTwap.methods
        .placeOrder(passBuyArgs)
        .accounts({
          signer: mm0.publicKey,
          market: openbookPassMarket,
          asks: storedPassMarket.asks,
          bids: storedPassMarket.bids,
          marketVault: storedPassMarket.marketQuoteVault,
          eventHeap: storedPassMarket.eventHeap,
          openOrdersAccount: mm0.pOpenOrdersAccount,
          userTokenAccount: mm0.pUsdcAcc,
          twapMarket: openbookTwapPassMarket,
          openbookProgram: OPENBOOK_PROGRAM_ID,
        })
        .signers([mm0.keypair])
        .rpc();

      await openbookTwap.methods
        .placeOrder(passSellArgs)
        .accounts({
          signer: mm0.publicKey,
          market: openbookPassMarket,
          asks: storedPassMarket.asks,
          bids: storedPassMarket.bids,
          marketVault: storedPassMarket.marketBaseVault,
          eventHeap: storedPassMarket.eventHeap,
          openOrdersAccount: mm0.pOpenOrdersAccount,
          userTokenAccount: mm0.pMetaAcc,
          twapMarket: openbookTwapPassMarket,
          openbookProgram: OPENBOOK_PROGRAM_ID,
        })
        .signers([mm0.keypair])
        .rpc();

      await openbookTwap.methods
        .placeOrder(failBuyArgs)
        .accounts({
          signer: mm0.publicKey,
          market: openbookFailMarket,
          asks: storedFailMarket.asks,
          bids: storedFailMarket.bids,
          marketVault: storedFailMarket.marketQuoteVault,
          eventHeap: storedFailMarket.eventHeap,
          openOrdersAccount: mm0.fOpenOrdersAccount,
          userTokenAccount: mm0.fUsdcAcc,
          twapMarket: openbookTwapFailMarket,
          openbookProgram: OPENBOOK_PROGRAM_ID,
        })
        .signers([mm0.keypair])
        .rpc();

      await openbookTwap.methods
        .placeOrder(failSellArgs)
        .accounts({
          signer: mm0.publicKey,
          market: openbookFailMarket,
          asks: storedFailMarket.asks,
          bids: storedFailMarket.bids,
          marketVault: storedFailMarket.marketBaseVault,
          eventHeap: storedFailMarket.eventHeap,
          openOrdersAccount: mm0.fOpenOrdersAccount,
          userTokenAccount: mm0.fMetaAcc,
          twapMarket: openbookTwapFailMarket,
          openbookProgram: OPENBOOK_PROGRAM_ID,
        })
        .signers([mm0.keypair])
        .rpc();

      let storedDao = await autocrat.account.dao.fetch(dao);
      const passThresholdBpsBefore = storedDao.passThresholdBps;

      await autocrat.methods
        .finalizeProposal()
        .accounts({
          proposal,
          openbookTwapPassMarket,
          openbookTwapFailMarket,
          dao,
          baseVault,
          quoteVault,
          vaultProgram: vaultProgram.programId,
          daoTreasury,
        })
        .remainingAccounts(
          autocrat.instruction.updateDao
            .accounts({
              dao,
              daoTreasury,
            })
            .concat({
              pubkey: autocrat.programId,
              isWritable: false,
              isSigner: false,
            })
            .map((meta) =>
              meta.pubkey.equals(daoTreasury)
                ? { ...meta, isSigner: false }
                : meta
            )
        )
        .rpc();

      storedProposal = await autocrat.account.proposal.fetch(proposal);
      assert.exists(storedProposal.state.failed);

      let storedBaseVault = await vaultProgram.account.conditionalVault.fetch(
        baseVault
      );
      let storedQuoteVault = await vaultProgram.account.conditionalVault.fetch(
        quoteVault
      );

      assert.exists(storedBaseVault.status.reverted);
      assert.exists(storedQuoteVault.status.reverted);

      storedDao = await autocrat.account.dao.fetch(dao);
      assert.equal(storedDao.passThresholdBps, passThresholdBpsBefore);

      await redeemConditionalTokens(
        vaultProgram,
        alice,
        aliceBasePassConditionalTokenAccount,
        aliceBaseFailConditionalTokenAccount,
        storedBaseVault.conditionalOnFinalizeTokenMint,
        storedBaseVault.conditionalOnRevertTokenMint,
        aliceUnderlyingBaseTokenAccount,
        storedBaseVault.underlyingTokenAccount,
        baseVault,
        banksClient
      );
      await redeemConditionalTokens(
        vaultProgram,
        alice,
        aliceQuotePassConditionalTokenAccount,
        aliceQuoteFailConditionalTokenAccount,
        storedQuoteVault.conditionalOnFinalizeTokenMint,
        storedQuoteVault.conditionalOnRevertTokenMint,
        aliceUnderlyingQuoteTokenAccount,
        storedQuoteVault.underlyingTokenAccount,
        quoteVault,
        banksClient
      );

      // alice should have the same balance as she started with
      assert.equal(
        (await getAccount(banksClient, aliceUnderlyingBaseTokenAccount)).amount,
        0n
      );
      assert.equal(
        (await getAccount(banksClient, aliceUnderlyingQuoteTokenAccount))
          .amount,
        10_000n * 1_000_000n
      );
    });
  });
});

async function generateMarketMaker(
  openbook: OpenBookV2Client,
  openbookTwap: Program<OpenbookTwap>,
  banksClient: BanksClient,
  payer: anchor.web3.Keypair,
  passMarket: anchor.web3.PublicKey,
  failMarket: anchor.web3.PublicKey,
  vaultProgram: Program<ConditionalVault>,
  context: ProgramTestContext
): Promise<MarketMaker> {
  const mm = anchor.web3.Keypair.generate();

  const storedPassMarket = await openbook.getMarketAccount(passMarket);
  const storedFailMarket = await openbook.getMarketAccount(failMarket);

  const metaPassAcc = await createAccount(
    banksClient,
    payer,
    storedPassMarket.baseMint,
    mm.publicKey
  );

  const usdcPassAcc = await createAccount(
    banksClient,
    payer,
    storedPassMarket.quoteMint,
    mm.publicKey
  );

  const metaFailAcc = await createAccount(
    banksClient,
    payer,
    storedFailMarket.baseMint,
    mm.publicKey
  );

  const usdcFailAcc = await createAccount(
    banksClient,
    payer,
    storedFailMarket.quoteMint,
    mm.publicKey
  );

  // we can use either to get the base/quote vault
  const baseMint = await getMint(banksClient, storedPassMarket.baseMint);
  const quoteMint = await getMint(banksClient, storedPassMarket.quoteMint);

  const baseVault = baseMint.mintAuthority;
  const quoteVault = quoteMint.mintAuthority;

  assert(
    baseVault.equals(
      (await getMint(banksClient, storedFailMarket.baseMint)).mintAuthority
    )
  );
  assert(
    quoteVault.equals(
      (await getMint(banksClient, storedFailMarket.quoteMint)).mintAuthority
    )
  );

  const storedBaseVault = await vaultProgram.account.conditionalVault.fetch(
    baseVault
  );
  const storedQuoteVault = await vaultProgram.account.conditionalVault.fetch(
    quoteVault
  );

  const mmBaseUnderlying = await createAccount(
    banksClient,
    payer,
    storedBaseVault.underlyingTokenMint,
    mm.publicKey
  );
  const mmQuoteUnderlying = await createAccount(
    banksClient,
    payer,
    storedQuoteVault.underlyingTokenMint,
    mm.publicKey
  );

  const MM_BASE_AMOUNT = 10_000n * 1_000_000_000n;
  const MM_QUOTE_AMOUNT = 100_000n * 1_000_000n;
  await mintToOverride(context, mmBaseUnderlying, MM_BASE_AMOUNT);
  await mintToOverride(context, mmQuoteUnderlying, MM_QUOTE_AMOUNT);

  await mintConditionalTokens(
    vaultProgram,
    MM_BASE_AMOUNT,
    mm,
    baseVault,
    banksClient
  );

  await mintConditionalTokens(
    vaultProgram,
    MM_QUOTE_AMOUNT,
    mm,
    quoteVault,
    banksClient
  );

  let pOpenOrdersAccount = await openbook.createOpenOrders(
    payer,
    passMarket,
    new BN(1),
    "oo",
    mm
  );

  let fOpenOrdersAccount = await openbook.createOpenOrders(
    payer,
    failMarket,
    new BN(2),
    "oo",
    mm
  );

  return {
    publicKey: mm.publicKey,
    keypair: mm,
    pOpenOrdersAccount,
    fOpenOrdersAccount,
    pMetaAcc: token.getAssociatedTokenAddressSync(
      storedBaseVault.conditionalOnFinalizeTokenMint,
      mm.publicKey
    ),
    fMetaAcc: token.getAssociatedTokenAddressSync(
      storedBaseVault.conditionalOnRevertTokenMint,
      mm.publicKey
    ),
    pUsdcAcc: token.getAssociatedTokenAddressSync(
      storedQuoteVault.conditionalOnFinalizeTokenMint,
      mm.publicKey
    ),
    fUsdcAcc: token.getAssociatedTokenAddressSync(
      storedQuoteVault.conditionalOnRevertTokenMint,
      mm.publicKey
    ),
  };
}

async function initializeProposal(
  autocrat: Program<AutocratV0>,
  ix: ProposalInstruction,
  vaultProgram: Program<ConditionalVault>,
  dao: PublicKey,
  context: ProgramTestContext,
  payer: Keypair,
  openbook: OpenBookV2Client,
  openbookTwap: Program<OpenbookTwap>
): Promise<PublicKey> {
  const proposalKeypair = Keypair.generate();

  const currentClock = await context.banksClient.getClock();
  const slot = currentClock.slot + 1n;
  context.setClock(
    new Clock(
      slot,
      currentClock.epochStartTimestamp,
      currentClock.epoch,
      currentClock.leaderScheduleEpoch,
      currentClock.unixTimestamp
    )
  );

  const storedDAO = await autocrat.account.dao.fetch(dao);

  // least signficant 32 bits of nonce are proposal number
  // most significant bit of nonce is 0 for pass and 1 for fail
  // second most significant bit of nonce is 0 for base and 1 for quote

  let baseNonce = new BN(storedDAO.proposalCount);

  const baseVault = await initializeVault(
    vaultProgram,
    storedDAO.treasury,
    storedDAO.metaMint,
    baseNonce,
    payer
  );

  const quoteVault = await initializeVault(
    vaultProgram,
    storedDAO.treasury,
    storedDAO.usdcMint,
    baseNonce.or(new BN(1).shln(63)),
    payer
  );

  const passBaseMint = (
    await vaultProgram.account.conditionalVault.fetch(baseVault)
  ).conditionalOnFinalizeTokenMint;
  const passQuoteMint = (
    await vaultProgram.account.conditionalVault.fetch(quoteVault)
  ).conditionalOnFinalizeTokenMint;
  const failBaseMint = (
    await vaultProgram.account.conditionalVault.fetch(baseVault)
  ).conditionalOnRevertTokenMint;
  const failQuoteMint = (
    await vaultProgram.account.conditionalVault.fetch(quoteVault)
  ).conditionalOnRevertTokenMint;

  const [daoTreasury] = PublicKey.findProgramAddressSync(
    [dao.toBuffer()],
    autocrat.programId
  );

  const daoBefore = await autocrat.account.dao.fetch(dao);

  const dummyURL = "https://www.eff.org/cyberspace-independence";

  let openbookPassMarketKP = Keypair.generate();

  let [openbookTwapPassMarket] = PublicKey.findProgramAddressSync(
    [
      anchor.utils.bytes.utf8.encode("twap_market"),
      openbookPassMarketKP.publicKey.toBuffer(),
    ],
    openbookTwap.programId
  );

  let openbookPassMarket = await openbook.createMarket(
    payer,
    "pMETA/pUSDC",
    passQuoteMint,
    passBaseMint,
    new BN(1),
    new BN(1e9),
    new BN(0),
    new BN(0),
    new BN(0),
    null,
    null,
    openbookTwapPassMarket,
    null,
    openbookTwapPassMarket,
    { confFilter: 0.1, maxStalenessSlots: 100 },
    openbookPassMarketKP,
    daoTreasury
  );

  await openbookTwap.methods
    .createTwapMarket(new BN(daoBefore.twapExpectedValue))
    .accounts({
      market: openbookPassMarket,
      twapMarket: openbookTwapPassMarket,
    })
    .rpc();

  let openbookFailMarketKP = Keypair.generate();

  let [openbookTwapFailMarket] = PublicKey.findProgramAddressSync(
    [
      anchor.utils.bytes.utf8.encode("twap_market"),
      openbookFailMarketKP.publicKey.toBuffer(),
    ],
    openbookTwap.programId
  );

  let openbookFailMarket = await openbook.createMarket(
    payer,
    "fMETA/fUSDC",
    failQuoteMint,
    failBaseMint,
    new BN(1),
    new BN(1e9),
    new BN(0),
    new BN(0),
    new BN(0),
    null,
    null,
    openbookTwapFailMarket,
    null,
    openbookTwapFailMarket,
    { confFilter: 0.1, maxStalenessSlots: 100 },
    openbookFailMarketKP,
    daoTreasury
  );
  await openbookTwap.methods
    .createTwapMarket(new BN(daoBefore.twapExpectedValue))
    .accounts({
      market: openbookFailMarket,
      twapMarket: openbookTwapFailMarket,
    })
    .rpc();

  await autocrat.methods
    .initializeProposal(dummyURL, ix)
    .preInstructions([
      await autocrat.account.proposal.createInstruction(proposalKeypair, 1500),
    ])
    .accounts({
      proposal: proposalKeypair.publicKey,
      dao,
      daoTreasury,
      baseVault,
      quoteVault,
      openbookTwapPassMarket,
      openbookTwapFailMarket,
      openbookPassMarket,
      openbookFailMarket,
      proposer: payer.publicKey,
      systemProgram: anchor.web3.SystemProgram.programId,
    })
    .signers([proposalKeypair])
    .rpc()
    .then(
      () => {},
      (err) => console.error(err)
    );

  const storedProposal = await autocrat.account.proposal.fetch(
    proposalKeypair.publicKey
  );

  const daoAfter = await autocrat.account.dao.fetch(dao);

  assert.equal(daoAfter.proposalCount, daoBefore.proposalCount + 1);

  assert.equal(storedProposal.number, daoBefore.proposalCount);
  assert.ok(storedProposal.proposer.equals(payer.publicKey));
  assert.equal(storedProposal.descriptionUrl, dummyURL);
  assert.ok(
    storedProposal.openbookTwapFailMarket.equals(openbookTwapFailMarket)
  );
  assert.ok(
    storedProposal.openbookTwapPassMarket.equals(openbookTwapPassMarket)
  );
  assert.equal(
    storedProposal.slotEnqueued.toString(),
    new BN(slot.toString()).toString()
  );
  assert.deepEqual(storedProposal.state, { pending: {} });

  const storedIx = storedProposal.instruction;
  assert.ok(storedIx.programId.equals(ix.programId));
  assert.deepEqual(storedIx.accounts, ix.accounts);
  assert.deepEqual(storedIx.data, ix.data);

  return proposalKeypair.publicKey;
}

async function initializeVault(
  vaultProgram: Program<ConditionalVault>,
  settlementAuthority: PublicKey,
  underlyingTokenMint: PublicKey,
  nonce: BN,
  payer: Keypair
): Promise<PublicKey> {
  const [vault] = anchor.web3.PublicKey.findProgramAddressSync(
    [
      anchor.utils.bytes.utf8.encode("conditional_vault"),
      settlementAuthority.toBuffer(),
      underlyingTokenMint.toBuffer(),
      nonce.toBuffer("le", 8),
    ],
    vaultProgram.programId
  );
  const conditionalOnFinalizeTokenMintKeypair = Keypair.generate();
  const conditionalOnRevertTokenMintKeypair = Keypair.generate();

  const vaultUnderlyingTokenAccount = await token.getAssociatedTokenAddress(
    underlyingTokenMint,
    vault,
    true
  );
  await vaultProgram.methods
    .initializeConditionalVault(settlementAuthority, nonce)
    .accounts({
      vault,
      underlyingTokenMint,
      vaultUnderlyingTokenAccount,
      conditionalOnFinalizeTokenMint:
        conditionalOnFinalizeTokenMintKeypair.publicKey,
      conditionalOnRevertTokenMint:
        conditionalOnRevertTokenMintKeypair.publicKey,
      payer: payer.publicKey,
      tokenProgram: token.TOKEN_PROGRAM_ID,
      associatedTokenProgram: token.ASSOCIATED_TOKEN_PROGRAM_ID,
      systemProgram: anchor.web3.SystemProgram.programId,
    })
    .signers([
      conditionalOnFinalizeTokenMintKeypair,
      conditionalOnRevertTokenMintKeypair,
    ])
    .rpc();

  return vault;
}
