use super::*;

impl<'info, 'c: 'info> InteractWithVault<'info> {
    /// Handles the splitting of underlying tokens into conditional tokens.
    /// This operation creates equal amounts of each conditional token type
    /// in exchange for underlying tokens, enabling participation in the prediction market.
    ///
    /// # Mathematical Model
    /// For amount X of underlying tokens:
    /// - Vault receives: X underlying tokens
    /// - User receives: X tokens of each conditional outcome
    /// - Conservation: Total value is preserved across split
    ///
    /// # Arguments
    /// * `ctx` - Context containing all required accounts
    /// * `amount` - Amount of underlying tokens to split into conditional tokens
    ///
    /// # Example
    /// ```ignore
    /// // Splitting 100 tokens in a binary outcome market:
    /// // Input: 100 underlying tokens
    /// // Output: 100 YES tokens AND 100 NO tokens
    /// handle_split_tokens(ctx, 100)?;
    /// ```
    pub fn handle_split_tokens(ctx: Context<'_, '_, 'c, 'info, Self>, amount: u64) -> Result<()> {
        let accs = &ctx.accounts;

        // Get and validate all conditional token mints and user token accounts
        let (mut conditional_token_mints, mut user_conditional_token_accounts) =
            Self::get_mints_and_user_token_accounts(&ctx)?;

        // Record initial state for verification
        let pre_vault_underlying_balance = accs.vault_underlying_token_account.amount;
        // Store initial balances of user's conditional tokens
        let pre_conditional_user_balances = user_conditional_token_accounts
            .iter()
            .map(|acc| acc.amount)
            .collect::<Vec<u64>>();
        // Store initial supplies of conditional tokens
        let pre_conditional_mint_supplies = conditional_token_mints
            .iter()
            .map(|mint| mint.supply)
            .collect::<Vec<u64>>();

        // Verify user has sufficient underlying tokens
        require!(
            accs.user_underlying_token_account.amount >= amount,
            VaultError::InsufficientUnderlyingTokens
        );

        // Generate vault PDA seeds for signing mint operations
        let vault = &accs.vault;

        let seeds = generate_vault_seeds!(vault);
        let signer = &[&seeds[..]];

        // Validate token accounts and decimals
        require_eq!(
            accs.user_underlying_token_account.mint,
            accs.vault_underlying_token_mint.key(),
            VaultError::UnderlyingTokenMintMismatch
        );
        require_eq!(
            accs.vault_underlying_token_account.mint,
            accs.vault_underlying_token_mint.key(),
            VaultError::UnderlyingTokenMintMismatch
        );
        require_eq!(
            accs.vault_underlying_token_mint.decimals,
            accs.vault.decimals,
            VaultError::AssertFailed
        );

        // Transfer underlying tokens from user to vault with decimal verification
        token::transfer_checked(
            CpiContext::new(
                accs.token_program.to_account_info(),
                TransferChecked {
                    from: accs.user_underlying_token_account.to_account_info(),
                    mint: accs.vault_underlying_token_mint.to_account_info(),
                    to: accs.vault_underlying_token_account.to_account_info(),
                    authority: accs.authority.to_account_info(),
                },
            ),
            amount,
            accs.vault_underlying_token_mint.decimals,
        )?;

        // Mint conditional tokens to user
        // Creates equal amounts of each type
        for (conditional_mint, user_conditional_token_account) in conditional_token_mints
            .iter()
            .zip(user_conditional_token_accounts.iter())
        {
            token::mint_to(
                CpiContext::new_with_signer(
                    accs.token_program.to_account_info(),
                    MintTo {
                        mint: conditional_mint.to_account_info(),
                        to: user_conditional_token_account.to_account_info(),
                        authority: accs.vault.to_account_info(),
                    },
                    signer,
                ),
                amount,
            )?;
        }

        // === Verify State Changes ===

        // Verify vault received underlying tokens
        ctx.accounts.vault_underlying_token_account.reload()?;
        require_eq!(
            ctx.accounts.vault_underlying_token_account.amount,
                 pre_vault_underlying_balance + amount,
                 VaultError::AssertFailed
        );

        // Verify conditional token supplies increased correctly
        for (i, mint) in conditional_token_mints.iter_mut().enumerate() {
            mint.reload()?;
            require_eq!(mint.supply, pre_conditional_mint_supplies[i] + amount, VaultError::AssertFailed);
        }

        // Verify user received correct amounts of conditional tokens
        for (i, acc) in user_conditional_token_accounts.iter_mut().enumerate() {
            acc.reload()?;
            require_eq!(acc.amount, pre_conditional_user_balances[i] + amount, VaultError::AssertFailed);
        }

        // Verify vault remains solvent after split operation
        ctx.accounts.vault.invariant(
            &ctx.accounts.question,
            conditional_token_mints
                .iter()
                .map(|mint| mint.supply)
                .collect::<Vec<u64>>(),
            ctx.accounts.vault_underlying_token_account.amount,
        )?;

        // Increment vault sequence number for tracking
        ctx.accounts.vault.seq_num += 1;

        // Emit event with updated state
        let clock = Clock::get()?;
        emit_cpi!(SplitTokensEvent {
            common: CommonFields {
                slot: clock.slot,
                unix_timestamp: clock.unix_timestamp,
            },
            user: ctx.accounts.authority.key(),
            vault: ctx.accounts.vault.key(),
            amount,
            post_user_underlying_balance: ctx.accounts.user_underlying_token_account.amount,
            post_vault_underlying_balance: ctx.accounts.vault_underlying_token_account.amount,
            post_user_conditional_token_balances: user_conditional_token_accounts.iter().map(|account| account.amount).collect(),
            post_conditional_token_supplies: conditional_token_mints.iter().map(|mint| mint.supply).collect(),
            seq_num: ctx.accounts.vault.seq_num,
        });
        Ok(())
    }
}
