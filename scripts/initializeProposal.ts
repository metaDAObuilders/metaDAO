import * as anchor from "@coral-xyz/anchor";
import { MEMO_PROGRAM_ID } from "@solana/spl-memo";
import * as token from "@solana/spl-token";
import { LAMPORTS_PER_SOL } from "@solana/web3.js";
import { AutocratClient } from "@metadaoproject/futarchy/v0.3";

const { PublicKey, Keypair, SystemProgram } = anchor.web3;
const { BN, Program } = anchor;

const provider = anchor.AnchorProvider.env();
anchor.setProvider(provider);

let autocratClient: AutocratClient = AutocratClient.createClient({
  provider: anchor.AnchorProvider.env(),
});

const payer = provider.wallet["payer"];

const PANTERA_PUBKEY = new PublicKey(
  "BtNPTBX1XkFCwazDJ6ZkK3hcUsomm1RPcfmtUrP6wd2K"
);

const COST_DEPLOY = 0.1 * LAMPORTS_PER_SOL;

// Transfer
const buildTreasuryTransferInstruction = async (
  daoTreasury: anchor.web3.PublicKey,
  destinationAccount: anchor.web3.PublicKey,
  tokenMint: anchor.web3.PublicKey,
  amount: number
) => {
  console.log(`Transfer token ${tokenMint.toString()}`);
  // This gets the origin account with the token intended for transfer
  const originAcc = await token.getOrCreateAssociatedTokenAccount(
    provider.connection,
    payer,
    tokenMint,
    daoTreasury,
    true
  );
  console.log("Origin account");
  console.log(originAcc.address.toString());
  console.log("Origin balance of token");
  const accountBalance = (
    await provider.connection.getTokenAccountBalance(originAcc.address)
  ).value;
  console.log(accountBalance.uiAmountString);

  const transferAmount = amount * 10 ** accountBalance.decimals;

  if (transferAmount > Number(accountBalance.amount)) {
    console.error(
      `Account does not have enough balance to transfer ${transferAmount}`
    );
    console.error(`Account's balance is ${accountBalance.amount}`);
    return;
  }

  console.log(
    `Transfer amount ${transferAmount / 10 ** accountBalance.decimals}`
  );

  // Sets up the destination account with the token and address provided
  const destinationAcc = await token.getOrCreateAssociatedTokenAccount(
    provider.connection,
    payer,
    tokenMint,
    destinationAccount,
    true
  );

  console.log("Destination account");
  console.log(destinationAcc.address.toString());

  const transferIx = token.createTransferInstruction(
    originAcc.address,
    destinationAcc.address,
    daoTreasury,
    transferAmount
  );

  console.log("Transfer instructions");
  console.log(transferIx);

  return transferIx;
};

// Memo
const buildMemoInstruction = async (memoText: string) => {
  console.log("Memo text");
  console.log(memoText);
  console.log("Memo text length");
  console.log(memoText.length);
  const byteLengthOfMemo = Buffer.byteLength(memoText);
  if (byteLengthOfMemo >= 566) {
    throw Error("Memo text is too big");
  }
  console.log("Memo program PublicKey");
  console.log(MEMO_PROGRAM_ID);

  const createMemoIx = {
    programId: new PublicKey(MEMO_PROGRAM_ID),
    keys: [],
    data: Buffer.from(memoText),
  };

  console.log("Memo instructions");
  console.log(createMemoIx);

  return createMemoIx;
};

// Burn
const buildTreasuryBurnInstruction = async (
  daoTreasury: anchor.web3.PublicKey,
  tokenMint: anchor.web3.PublicKey,
  amount: number
) => {
  console.log(`Burn token ${tokenMint.toString()}`);
  // This gets the origin account with the token intended for burn
  const originAcc = await token.getOrCreateAssociatedTokenAccount(
    provider.connection,
    payer,
    tokenMint,
    daoTreasury,
    true
  );
  console.log("Origin account");
  console.log(originAcc.address.toString());
  console.log("Origin balance of token");
  const accountBalance = (
    await provider.connection.getTokenAccountBalance(originAcc.address)
  ).value;
  console.log(accountBalance.uiAmountString);

  const burnAmount = amount * 10 ** accountBalance.decimals;

  if (burnAmount > Number(accountBalance.amount)) {
    console.error(`Account does not have enough balance to burn ${burnAmount}`);
    console.error(`Account's balance is ${accountBalance.amount}`);
    return;
  }

  console.log(`Burn amount ${burnAmount / 10 ** accountBalance.decimals}`);

  const burnIx = token.createBurnInstruction(
    originAcc.address,
    tokenMint,
    daoTreasury,
    burnAmount
  );

  console.log("Burn instructions");
  console.log(burnIx);

  return burnIx;
};

async function main() {
  console.log("Initializing with PublicKey");
  console.log(payer.publicKey.toString());
  const payerBalance = await provider.connection.getBalance(payer.publicKey);
  console.log("PublicKey SOL balance");
  console.log(payerBalance / LAMPORTS_PER_SOL);

  // Check to ensure the payer has enough SOL to actually execute the proposal creation
  if (payerBalance < COST_DEPLOY) {
    const diff = COST_DEPLOY - payerBalance;
    console.error(
      `PublicKey doesn't have enough balance ${
        payerBalance / LAMPORTS_PER_SOL
      } to initialize proposal ${COST_DEPLOY / LAMPORTS_PER_SOL}`
    );
    console.error(`Add ${diff / LAMPORTS_PER_SOL} more SOL`);
    return;
  }

  console.log("Account has enough SOL (4) to continue");

  const proposalIx = await buildMemoInstruction("TESTING DEVNET");

  const ix = {
    programId: proposalIx.programId,
    accounts: proposalIx.keys,
    data: proposalIx.data,
  };

  console.log("Proposal instruction");
  console.log(ix);

  // Sleep for review
  console.log("Sleeping for 60s, press ctrl + c to cancel");
  await new Promise((f) => setTimeout(f, 5000));

  const dao = new PublicKey("33Pi6Dxur8Q87K7DmG8JAdZoiTwSRi2HCP6ZjLAPn2sE");
  const storedDao = await autocratClient.getDao(dao);

  const proposal = await autocratClient.initializeProposal(
    dao,
    "http://lfg.com",
    ix,
    storedDao.minBaseFutarchicLiquidity,
    storedDao.minQuoteFutarchicLiquidity
  );

  console.log(proposal);
}

main();
