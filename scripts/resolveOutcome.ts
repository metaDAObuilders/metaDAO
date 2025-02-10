import { PublicKey } from "@solana/web3.js";
import * as anchor from "@coral-xyz/anchor";
import {
  ConditionalVaultClient,
} from "@metadaoproject/futarchy/v0.4";

const provider = anchor.AnchorProvider.env();
const payer = provider.wallet["payer"];
console.log(payer.publicKey.toBase58())
const vaultProgram: ConditionalVaultClient =
  ConditionalVaultClient.createClient({ provider });

const outcomeQuestionAddress = new PublicKey("8xB61Ps8gDpdv9LZ5Jb5iC3sAhBctyYq6C112u55mmxt");
const metricQuestionAddress = new PublicKey("4FXN6zSRFtMb7PCxGSqZLeDjex5N61TL2q3zu53ca4Nf");

async function main() {
    const resolveQuestionIx = vaultProgram.resolveQuestionIx(
        outcomeQuestionAddress, payer, [1,0]
    );
    await resolveQuestionIx.rpc();
    console.log(resolveQuestionIx);

    //pause for 2 mins
    console.log("Pausing for 2 mins");
    await new Promise((f) => setTimeout(f, 120000));

    const resolveQuestionIx2 = vaultProgram.resolveQuestionIx(
        metricQuestionAddress, payer, [1,1]
    );
    await resolveQuestionIx2.rpc();
    console.log(resolveQuestionIx2);
}

main();