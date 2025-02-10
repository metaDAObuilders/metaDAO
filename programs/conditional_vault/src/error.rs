use super::*;

#[error_code]
pub enum VaultError {
    #[msg("An assertion failed")]
    AssertFailed,
    #[msg("Insufficient underlying token balance to mint this amount of conditional tokens")]
    InsufficientUnderlyingTokens,
    #[msg("Insufficient conditional token balance to merge this `amount`")]
    InsufficientConditionalTokens,
    #[msg("This `vault_underlying_token_account` is not this vault's `underlying_token_account`")]
    InvalidVaultUnderlyingTokenAccount,
    #[msg("This `vault_underlying_token_mint` is not this vault's `underlying_token_mint`")]
    InvalidVaultUnderlyingTokenMint,
    #[msg("This conditional token mint is not this vault's conditional token mint")]
    InvalidConditionalTokenMint,
    #[msg("Question needs to be resolved before users can redeem conditional tokens for underlying tokens")]
    CantRedeemConditionalTokens,
    #[msg("Questions need 2 or more conditions")]
    InsufficientNumConditions,
    #[msg("Invalid number of payout numerators")]
    InvalidNumPayoutNumerators,
    #[msg("Client needs to pass in the list of conditional mints for a vault followed by the user's token accounts for those tokens")]
    InvalidConditionals,
    #[msg("Conditional mint not in vault")]
    ConditionalMintMismatch,
    #[msg("Unable to deserialize a conditional token mint")]
    BadConditionalMint,
    #[msg("Unable to deserialize a conditional token account")]
    BadConditionalTokenAccount,
    #[msg("User conditional token account mint does not match conditional mint")]
    ConditionalTokenMintMismatch,
    #[msg("Vault or user token account mint does not match underlying token mint")]
    UnderlyingTokenMintMismatch,
    #[msg("Payouts must sum to 1 or more")]
    PayoutZero,
    #[msg("Question already resolved")]
    QuestionAlreadyResolved,
    #[msg("Conditional token metadata already set")]
    ConditionalTokenMetadataAlreadySet,
}
