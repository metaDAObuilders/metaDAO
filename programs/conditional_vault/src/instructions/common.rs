use super::*;

/// Account context for interacting with a conditional vault.
/// This structure validates and processes all accounts needed for vault operations
/// such as minting, burning, or settling conditional tokens.
#[event_cpi]
#[derive(Accounts)]
pub struct InteractWithVault<'info> {
    /// The non-mutable question account associated with this vault
    /// Contains outcome information and oracle details
    pub question: Account<'info, Question>,

    /// The vault account being interacted with
    /// Must be associated with the provided question account
    /// Marked as mutable to allow updating vault state
    #[account(mut, has_one = question)]
    pub vault: Account<'info, ConditionalVault>,

    /// The mint account for the underlying token used in this vault.
    /// This account is critical for token validation and decimal precision checking
    /// during transfer operations.
    ///
    /// # Validation
    /// - Must match the mint stored in vault account (`vault.underlying_token_mint`)
    /// - Must be a valid SPL token mint account
    /// - Used to verify decimal precision in token operations
    ///
    /// # Security Considerations
    /// 1. Address validation ensures we're using the correct underlying token
    /// 2. Prevents token substitution attacks
    /// 3. Enables decimal verification in transfer_checked operations
    /// 4. Guards against incorrect token account associations
    ///
    /// # Usage in Operations
    /// - Transfer validation: Used in transfer_checked for decimal verification
    /// - Decimal consistency: Ensures all operations maintain correct precision
    /// - Token validation: Verifies token account relationships
    ///
    /// # Example
    /// ```ignore
    /// token::transfer_checked(
    ///     ctx,
    ///     amount,
    ///     vault_underlying_token_mint.decimals,  // Decimal verification
    /// )?;
    /// ```
    ///
    /// # Error Scenarios
    /// Returns `VaultError::UnderlyingTokenMintMismatch` when:
    /// - Provided mint doesn't match vault's recorded mint
    /// - Incorrect mint account is supplied
    /// - Mint address validation fails
    #[account(address = vault.underlying_token_mint @ VaultError::InvalidVaultUnderlyingTokenMint)]
    pub vault_underlying_token_mint: Account<'info, Mint>,

    /// The vault's underlying token account
    /// Holds the collateral tokens backing the conditional tokens
    /// Must match the account stored in the vault's state
    #[account(
        mut,
        constraint = vault_underlying_token_account.key() == vault.underlying_token_account @ VaultError::InvalidVaultUnderlyingTokenAccount
    )]
    pub vault_underlying_token_account: Account<'info, TokenAccount>,

    /// The authority (user) initiating the interaction
    /// Must sign the transaction to authorize token transfers
    pub authority: Signer<'info>,

    /// The user's token account for the underlying token
    /// Used for depositing/withdrawing underlying tokens/collateral
    /// Must be owned by the authority and match the vault's underlying token mint
    #[account(
        mut,
        token::authority = authority,
        token::mint = vault.underlying_token_mint
    )]
    pub user_underlying_token_account: Account<'info, TokenAccount>,

    /// The SPL Token program account
    /// Required for token operations
    pub token_program: Program<'info, Token>,
}

impl<'info, 'c: 'info> InteractWithVault<'info> {
    /// Retrieves and validates conditional token mints and their corresponding user accounts
    /// from the remaining accounts in the transaction.
    ///
    /// # Arguments
    /// * `ctx` - The context containing all transaction accounts including remaining accounts
    ///
    /// # Returns
    /// * `Result<(Vec<Account<'info, Mint>>, Vec<Account<'info, TokenAccount>>)>`:
    ///   - First vector contains validated conditional token mint accounts
    ///   - Second vector contains validated user token accounts for each conditional token
    ///
    /// # Validation Steps
    /// 1. Verifies correct number of remaining accounts (2 accounts per outcome)
    /// 2. Validates conditional token mint addresses match vault's records
    /// 3. Ensures all mint accounts are valid
    /// 4. Verifies user token accounts correspond to correct mints
    ///
    /// # Errors
    /// * `VaultError::InvalidConditionals` - Incorrect number of remaining accounts
    /// * `VaultError::ConditionalMintMismatch` - Mint address doesn't match vault records
    /// * `VaultError::BadConditionalMint` - Invalid mint account
    /// * `VaultError::BadConditionalTokenAccount` - Invalid token account
    /// * `VaultError::ConditionalTokenMintMismatch` - Token account mint mismatch
    ///
    /// # Example
    /// ```ignore
    /// let (mints, user_accounts) = InteractWithVault::get_mints_and_user_token_accounts(&ctx)?;
    /// // mints[0] is the mint for first outcome
    /// // user_accounts[0] is user's token account for first outcome
    /// ```
    pub fn get_mints_and_user_token_accounts(
        ctx: &Context<'_, '_, 'c, 'info, Self>,
    ) -> Result<(Vec<Account<'info, Mint>>, Vec<Account<'info, TokenAccount>>)> {
        // Get iterator over remaining accounts
        let remaining_accs = &mut ctx.remaining_accounts.iter();

        // Calculate expected number of accounts based on outcomes
        let expected_num_conditional_tokens = ctx.accounts.question.num_outcomes();

        // Verify we have correct number of accounts (2 per outcome: mint + token account)
        require_eq!(
            remaining_accs.len(),
            expected_num_conditional_tokens * 2,
            VaultError::InvalidConditionals
        );

        // Initialize vectors to store validated accounts
        let mut conditional_token_mints = vec![];
        let mut user_conditional_token_accounts = vec![];

        for i in 0..expected_num_conditional_tokens {
            let conditional_token_mint = next_account_info(remaining_accs)?;
            require_eq!(
                ctx.accounts.vault.conditional_token_mints[i],
                conditional_token_mint.key(),
                VaultError::ConditionalMintMismatch
            );

            // Validate and convert to Mint account
            // Note: This should never fail as mints are initialized with vault
            conditional_token_mints.push(
                Account::<Mint>::try_from(conditional_token_mint)
                    .or(Err(VaultError::BadConditionalMint))?,
            );
        }

        // Second pass: validate and collect all user token accounts
        for i in 0..expected_num_conditional_tokens {
            // Get next account info
            let user_conditional_token_account = next_account_info(remaining_accs)?;

            // Validate and convert to TokenAccount
            let user_conditional_token_account =
                Account::<TokenAccount>::try_from(user_conditional_token_account)
                    .or(Err(VaultError::BadConditionalTokenAccount))?;

            // Verify token account's mint matches corresponding conditional token mint
            require_eq!(
                user_conditional_token_account.mint,
                conditional_token_mints[i].key(),
                VaultError::ConditionalTokenMintMismatch
            );

            user_conditional_token_accounts.push(user_conditional_token_account);
        }

        Ok((conditional_token_mints, user_conditional_token_accounts))
    }
}
