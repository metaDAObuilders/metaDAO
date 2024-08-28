import initializeQuestion from "./unit/initializeQuestion.test.js";
import initializeConditionalVault from "./unit/initializeConditionalVault.test.js";
import resolveQuestion from "./unit/resolveQuestion.test.js";
import splitTokens from "./unit/splitTokens.test.js";
import mergeTokens from "./unit/mergeTokens.test.js";
import redeemTokens from "./unit/redeemTokens.test.js";
import addMetadataToConditionalTokens from "./unit/addMetadataToConditionalTokens.test.js";
import binaryPredictionMarket from "./integration/binaryPredictionMarket.test.js";
import scalarGrantMarket from "./integration/scalarGrantMarket.test.js";

export default function suite() {
  it("binary prediction market", binaryPredictionMarket);
  it("scalar grant market", scalarGrantMarket);
  describe("#initialize_question", initializeQuestion);
  describe("#initialize_conditional_vault", initializeConditionalVault);
  describe("#resolve_question", resolveQuestion);
  describe("#split_tokens", splitTokens);
  describe("#merge_tokens", mergeTokens);
  describe("#redeem_tokens", redeemTokens);
  describe("#add_metadata_to_conditional_tokens", addMetadataToConditionalTokens);
}
