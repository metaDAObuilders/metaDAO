use super::*;

impl<'info, 'c: 'info> InteractWithVault<'info> {
    /// Handles the merging of conditional tokens back into underlying tokens.
    /// This process burns equal amounts of all conditional tokens to reclaim the underlying token.
    ///
    /// # Lifecycle
    /// 1. Validates all account relationships and token balances
    /// 2. Burns conditional tokens from user accounts
    /// 3. Transfers underlying tokens from vault to user
    /// 4. Verifies all balance changes
    /// 5. Ensures vault remains solvent
    /// 6. Emits event with updated state
    ///
    /// # Arguments
    /// * `ctx` - Context containing all required accounts and program state
    /// * `amount` - Amount of each conditional token to merge/burn
    ///
    /// # Account Requirements
    /// * All conditional token accounts must belong to user
    /// * User must have sufficient balance in each conditional token
    /// * Vault must have sufficient underlying tokens
    ///
    /// # Returns
    /// * `Ok(())` on successful merge
    /// * `Err(VaultError)` on validation or execution failure
    pub fn handle_merge_tokens(ctx: Context<'_, '_, 'c, 'info, Self>, amount: u64) -> Result<()> {
        let accs = &ctx.accounts;

        // Get and validate all conditional token mints and user token accounts
        let (mut conditional_token_mints, mut user_conditional_token_accounts) =
            Self::get_mints_and_user_token_accounts(&ctx)?;

        // Verify user has sufficient balance in all conditional tokens
        for conditional_token_account in user_conditional_token_accounts.iter() {
            require!(
                conditional_token_account.amount >= amount,
                VaultError::InsufficientConditionalTokens
            );
        }

        let vault = &accs.vault;

        // Store initial balances for later verification
        let pre_user_underlying_balance = accs.user_underlying_token_account.amount;
        let pre_vault_underlying_balance = accs.vault_underlying_token_account.amount;

        // Calculate expected balances after operation
        // This helps verify the operation completed correctly
        let expected_future_balances: Vec<u64> = user_conditional_token_accounts
            .iter()
            .map(|account| account.amount - amount)
            .collect();
        let expected_future_supplies: Vec<u64> = conditional_token_mints
            .iter()
            .map(|mint| mint.supply - amount)
            .collect();

        // Generate vault PDA seeds for signing transfers
        let seeds = generate_vault_seeds!(vault);
        let signer = &[&seeds[..]];

        // Burn equal amounts of each conditional token
        for (conditional_mint, user_conditional_token_account) in conditional_token_mints
            .iter()
            .zip(user_conditional_token_accounts.iter())
        {
            token::burn(
                CpiContext::new(
                    accs.token_program.to_account_info(),
                    Burn {
                        mint: conditional_mint.to_account_info(),
                        from: user_conditional_token_account.to_account_info(),
                        authority: accs.authority.to_account_info(),
                    },
                ),
                amount,
            )?;
        }

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

        // Transfer `amount` from vault to user
        token::transfer_checked(
            CpiContext::new_with_signer(
                accs.token_program.to_account_info(),
                TransferChecked {
                    from: accs.vault_underlying_token_account.to_account_info(),
                    mint: accs.vault_underlying_token_mint.to_account_info(),  // Added mint account
                    to: accs.user_underlying_token_account.to_account_info(),
                    authority: accs.vault.to_account_info(),
                },
                signer,
            ),
            amount,
            accs.vault_underlying_token_mint.decimals,  // Use decimals from mint account
        )?;

        // Reload account states to verify changes
        ctx.accounts.user_underlying_token_account.reload()?;
        ctx.accounts.vault_underlying_token_account.reload()?;

        // Verify underlying token balances changed correctly
        require_eq!(
            ctx.accounts.user_underlying_token_account.amount,
                pre_user_underlying_balance + amount,
                VaultError::AssertFailed
        );
        require_eq!(
            ctx.accounts.vault_underlying_token_account.amount,
                pre_vault_underlying_balance - amount,
                VaultError::AssertFailed
        );

        // Verify all conditional token supplies decreased correctly
        for (mint, expected_supply) in conditional_token_mints
            .iter_mut()
            .zip(expected_future_supplies.iter())
        {
            mint.reload()?;
            require_eq!(mint.supply, *expected_supply, VaultError::AssertFailed);
        }

        // Verify all user conditional token balances decreased correctly
        for (account, expected_balance) in user_conditional_token_accounts
            .iter_mut()
            .zip(expected_future_balances.iter())
        {
            account.reload()?;
            require_eq!(account.amount, *expected_balance, VaultError::AssertFailed);
        }

        // Verify vault remains solvent after operation
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
        emit_cpi!(MergeTokensEvent {
            common: CommonFields {
                slot: Clock::get()?.slot,
                unix_timestamp: Clock::get()?.unix_timestamp,
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
