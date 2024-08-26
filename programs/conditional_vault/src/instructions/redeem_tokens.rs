use super::*;

impl<'info, 'c: 'info> InteractWithNewVault<'info> {
    pub fn validate_redeem_tokens(&self) -> Result<()> {
        require!(
            self.question.is_resolved(),
            VaultError::CantRedeemConditionalTokens
        );

        Ok(())
    }

    pub fn handle_redeem_tokens(ctx: Context<'_, '_, 'c, 'info, Self>) -> Result<()> {
        let accs = &ctx.accounts;

        let (mut conditional_token_mints, mut user_conditional_token_accounts) =
            Self::get_mints_and_user_token_accounts(&ctx)?;

        // calculate the expected future supplies of the conditional token mints
        // as current supply - user balance
        let expected_future_supplies: Vec<u64> = conditional_token_mints
            .iter()
            .zip(user_conditional_token_accounts.iter())
            .map(|(mint, account)| mint.supply - account.amount)
            .collect();

        let vault = &accs.vault;
        let question = &accs.question;

        let seeds = generate_new_vault_seeds!(vault);
        let signer = &[&seeds[..]];

        let user_underlying_balance_before = accs.user_underlying_token_account.amount;
        let vault_underlying_balance_before = accs.vault_underlying_token_account.amount;
        // safe because there is always at least two conditional tokens and thus
        // at least two user conditional token accounts
        let max_redeemable = user_conditional_token_accounts
            .iter()
            .map(|account| account.amount)
            .max()
            .unwrap();

        let mut total_redeemable = 0;

        for (conditional_mint, user_conditional_token_account) in conditional_token_mints
            .iter()
            .zip(user_conditional_token_accounts.iter())
        {
            // this is safe because we check that every conditional mint is a part of the vault
            let payout_index = vault
                .conditional_token_mints
                .iter()
                .position(|mint| mint == &conditional_mint.key())
                .unwrap();

            total_redeemable += ((user_conditional_token_account.amount as u128
                * question.payout_numerators[payout_index] as u128)
                / question.payout_denominator as u128) as u64;

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

        token::transfer(
            CpiContext::new_with_signer(
                accs.token_program.to_account_info(),
                Transfer {
                    from: accs.vault_underlying_token_account.to_account_info(),
                    to: accs.user_underlying_token_account.to_account_info(),
                    authority: accs.vault.to_account_info(),
                },
                signer,
            ),
            total_redeemable,
        )?;

        assert!(max_redeemable >= total_redeemable);

        ctx.accounts.user_underlying_token_account.reload()?;
        ctx.accounts.vault_underlying_token_account.reload()?;

        assert!(
            ctx.accounts.user_underlying_token_account.amount
                == user_underlying_balance_before + total_redeemable
        );

        assert!(
            ctx.accounts.vault_underlying_token_account.amount
                == vault_underlying_balance_before - total_redeemable
        );

        for acc in user_conditional_token_accounts.iter_mut() {
            acc.reload()?;
            assert!(acc.amount == 0);
        }

        for (mint, expected_supply) in conditional_token_mints.iter_mut().zip(expected_future_supplies.iter()) {
            mint.reload()?;
            assert!(mint.supply == *expected_supply);
        }

        ctx.accounts.vault.invariant(
            &ctx.accounts.question,
            conditional_token_mints.iter().map(|mint| mint.supply).collect::<Vec<u64>>(),
            ctx.accounts.vault_underlying_token_account.amount,
        )?;

        Ok(())
    }
}
