use super::*;

impl<'info, 'c: 'info> InteractWithVault<'info> {
    /// Validates whether conditional tokens can be redeemed
    /// This check ensures the question has been resolved before allowing redemption
    ///
    /// # Returns
    /// * `Ok(())` if the question is resolved
    /// * `Err(VaultError::CantRedeemConditionalTokens)` if question isn't resolved
    pub fn validate_redeem_tokens(&self) -> Result<()> {
        require!(
            self.question.is_resolved(),
            VaultError::CantRedeemConditionalTokens
        );

        Ok(())
    }

    /// Handles the redemption of conditional tokens after a question is resolved.
    /// Users can redeem their conditional tokens for underlying tokens based on
    /// the outcome payouts defined in the question.
    ///
    /// # Mathematical Model
    /// For each conditional token type i:
    /// redeemable_amount_i = user_balance_i * payout_numerator_i / payout_denominator
    /// total_redeemable = sum(redeemable_amount_i)
    ///
    /// # Lifecycle
    /// 1. Validates all accounts and balances
    /// 2. Calculates redemption amounts based on payouts
    /// 3. Burns all conditional tokens
    /// 4. Transfers underlying tokens to user
    /// 5. Verifies all state changes
    /// 6. Ensures vault remains solvent
    ///
    /// # Arguments
    /// * `ctx` - Context containing all required accounts and program state
    ///
    /// # Returns
    /// * `Ok(())` on successful redemption
    /// * `Err(VaultError)` on validation or execution failure
    pub fn handle_redeem_tokens(ctx: Context<'_, '_, 'c, 'info, Self>) -> Result<()> {
        let accs = &ctx.accounts;

        // Get and validate all conditional token mints and user token accounts
        let (mut conditional_token_mints, mut user_conditional_token_accounts) =
            Self::get_mints_and_user_token_accounts(&ctx)?;

        // Calculate expected future supplies after burning all user tokens
        let expected_future_supplies: Vec<u64> = conditional_token_mints
            .iter()
            .zip(user_conditional_token_accounts.iter())
            .map(|(mint, account)| mint.supply - account.amount)
            .collect();

        let vault = &accs.vault;
        let question = &accs.question;

        // Generate vault PDA seeds for signing transfers
        let seeds = generate_vault_seeds!(vault);
        let signer = &[&seeds[..]];

        // Store initial balances for verification
        let user_underlying_balance_before = accs.user_underlying_token_account.amount;
        let vault_underlying_balance_before = accs.vault_underlying_token_account.amount;

        // Find maximum amount user could receive (used for safety check)
        // Safe because we validate there are at least two conditional tokens and thus
        // at least two user conditional token accounts
        let max_redeemable = user_conditional_token_accounts
            .iter()
            .map(|account| account.amount)
            .max()
            .unwrap();

        // Calculate and execute redemption for each conditional token
        let mut total_redeemable = 0;

        for (conditional_mint, user_conditional_token_account) in conditional_token_mints
            .iter()
            .zip(user_conditional_token_accounts.iter())
        {
            // Find index of this mint in vault's records for matching payout
            // Safe because we validate all mints belong to vault
            let payout_index = vault
                .conditional_token_mints
                .iter()
                .position(|mint| mint == &conditional_mint.key())
                .unwrap();

            // Calculate redeemable amount based on payouts
            // Uses u128 for intermediate calculation to prevent overflow
            total_redeemable += ((user_conditional_token_account.amount as u128
                * question.payout_numerators[payout_index] as u128)
                / question.payout_denominator as u128) as u64;

            // Burn all conditional tokens of this type
            token::burn(
                CpiContext::new(
                    accs.token_program.to_account_info(),
                    Burn {
                        mint: conditional_mint.to_account_info(),
                        from: user_conditional_token_account.to_account_info(),
                        authority: accs.authority.to_account_info(),
                    },
                ),
                user_conditional_token_account.amount,
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

        // Transfer redeemable underlying tokens to user
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
            total_redeemable,
            accs.vault_underlying_token_mint.decimals,  // Use decimals from mint account
        )?;

        // Safety check: Total redeemable should never exceed max possible payout
        require_gte!(max_redeemable, total_redeemable, VaultError::AssertFailed);

        // Reload accounts and verify all balance changes
        ctx.accounts.user_underlying_token_account.reload()?;
        ctx.accounts.vault_underlying_token_account.reload()?;

        // Verify user received correct amount
        require_eq!(
            ctx.accounts.user_underlying_token_account.amount,
                user_underlying_balance_before + total_redeemable,
                VaultError::AssertFailed
        );

        // Verify vault balance decreased correctly
        require_eq!(
            ctx.accounts.vault_underlying_token_account.amount,
                vault_underlying_balance_before - total_redeemable,
                VaultError::AssertFailed
        );

        // Verify all conditional tokens were burned
        for acc in user_conditional_token_accounts.iter_mut() {
            acc.reload()?;
            require_eq!(acc.amount, 0, VaultError::AssertFailed);
        }

        // Verify all mint supplies decreased correctly
        for (mint, expected_supply) in conditional_token_mints
            .iter_mut()
            .zip(expected_future_supplies.iter())
        {
            mint.reload()?;
            require_eq!(mint.supply, *expected_supply, VaultError::AssertFailed);
        }

        // Verify vault remains solvent after redemption
        ctx.accounts.vault.invariant(
            &ctx.accounts.question,
            conditional_token_mints
                .iter()
                .map(|mint| mint.supply)
                .collect::<Vec<u64>>(),
            ctx.accounts.vault_underlying_token_account.amount,
        )?;

        // Increment vault sequence number
        ctx.accounts.vault.seq_num += 1;

        // Emit event with updated state
        let clock = Clock::get()?;
        emit_cpi!(RedeemTokensEvent {
            common: CommonFields {
                slot: clock.slot,
                unix_timestamp: clock.unix_timestamp,
            },
            user: ctx.accounts.authority.key(),
            vault: ctx.accounts.vault.key(),
            amount: total_redeemable,
            post_user_underlying_balance: ctx.accounts.user_underlying_token_account.amount,
            post_vault_underlying_balance: ctx.accounts.vault_underlying_token_account.amount,
            post_conditional_token_supplies: conditional_token_mints.iter().map(|mint| mint.supply).collect(),
            seq_num: ctx.accounts.vault.seq_num,
        });

        Ok(())
    }
}
