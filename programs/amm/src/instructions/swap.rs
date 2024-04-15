use anchor_lang::prelude::*;
use anchor_spl::associated_token::AssociatedToken;
use anchor_spl::token::*;

use crate::generate_vault_seeds;
use crate::state::*;
use crate::utils::{token_transfer_signed, token_transfer};

#[derive(Accounts)]
pub struct Swap<'info> {
    #[account(mut)]
    pub user: Signer<'info>,
    #[account(
        mut,
        has_one = base_mint,
        has_one = quote_mint,
    )]
    pub amm: Account<'info, Amm>,
    pub base_mint: Account<'info, Mint>,
    pub quote_mint: Account<'info, Mint>,
    #[account(
        mut,
        associated_token::mint = base_mint,
        associated_token::authority = user,
    )]
    pub user_ata_base: Account<'info, TokenAccount>,
    #[account(
        mut,
        associated_token::mint = quote_mint,
        associated_token::authority = user,
    )]
    pub user_ata_quote: Account<'info, TokenAccount>,
    #[account(
        mut,
        associated_token::mint = base_mint,
        associated_token::authority = amm,
    )]
    pub vault_ata_base: Account<'info, TokenAccount>,
    #[account(
        mut,
        associated_token::mint = quote_mint,
        associated_token::authority = amm,
    )]
    pub vault_ata_quote: Account<'info, TokenAccount>,
    pub associated_token_program: Program<'info, AssociatedToken>,
    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
}

pub fn handler(
    ctx: Context<Swap>,
    swap_type: SwapType,
    input_amount: u64,
    output_amount_min: u64,
) -> Result<()> {
    let Swap {
        user,
        amm,
        base_mint,
        quote_mint,
        user_ata_base,
        user_ata_quote,
        vault_ata_base,
        vault_ata_quote,
        associated_token_program: _,
        token_program,
        system_program: _,
    } = ctx.accounts;

    assert!(input_amount > 0);
    assert!(amm.total_ownership > 0);

    amm.update_twap(Clock::get()?.slot);

    let output_amount = amm.swap(input_amount, swap_type)?;

    // let base_amount_start = amm.base_amount as u128;
    // let quote_amount_start = amm.quote_amount as u128;

    // let k = base_amount_start.checked_mul(quote_amount_start).unwrap();

    // let input_amount_minus_fee = input_amount
    //     .checked_mul(BPS_SCALE.checked_sub(amm.swap_fee_bps).unwrap())
    //     .unwrap()
    //     .checked_div(BPS_SCALE)
    //     .unwrap() as u128;

    let base_mint_key = base_mint.key();
    let quote_mint_key = quote_mint.key();

    let seeds = generate_vault_seeds!(base_mint_key, quote_mint_key, amm.bump);

    // let output_amount = if is_quote_to_base {
    //     let temp_quote_amount = quote_amount_start
    //         .checked_add(input_amount_minus_fee)
    //         .unwrap();

    //     // for rounding up, if we have, a = b / c, we use: a = (b + (c - 1)) / c
    //     let temp_base_amount = k
    //         .checked_add(temp_quote_amount.checked_sub(1).unwrap())
    //         .unwrap()
    //         .checked_div(temp_quote_amount)
    //         .unwrap();

    //     let output_amount_base = base_amount_start
    //         .checked_sub(temp_base_amount)
    //         .unwrap()
    //         .to_u64()
    //         .unwrap();

    //     amm.quote_amount = amm.quote_amount.checked_add(input_amount).unwrap();
    //     amm.base_amount = amm.base_amount.checked_sub(output_amount_base).unwrap();

    //     // send user quote tokens to vault

    match swap_type {
        SwapType::Buy => {
            token_transfer(
                input_amount,
                token_program,
                user_ata_quote,
                vault_ata_quote,
                &user,
            )?;

            // send vault base tokens to user
            token_transfer_signed(
                output_amount,
                token_program,
                vault_ata_base,
                user_ata_base,
                amm,
                seeds,
            )?;
        }
        SwapType::Sell => {
            // send user base tokens to vault
            token_transfer(
                input_amount,
                token_program,
                &user_ata_base,
                &vault_ata_base,
                &user,
            )?;

            // send vault quote tokens to user
            token_transfer_signed(
                output_amount,
                token_program,
                vault_ata_quote,
                user_ata_quote,
                amm,
                seeds,
            )?;
        }
    }

    //     output_amount_base
    // } else {
    //     let temp_base_amount = base_amount_start
    //         .checked_add(input_amount_minus_fee)
    //         .unwrap();

    //     // for rounding up, if we have, a = b / c, we use: a = (b + (c - 1)) / c
    //     let temp_quote_amount = k
    //         .checked_add(temp_base_amount.checked_sub(1).unwrap())
    //         .unwrap()
    //         .checked_div(temp_base_amount)
    //         .unwrap();

    //     let output_amount_quote = quote_amount_start
    //         .checked_sub(temp_quote_amount)
    //         .unwrap()
    //         .to_u64()
    //         .unwrap();

    //     amm.base_amount = amm.base_amount.checked_add(input_amount).unwrap();
    //     amm.quote_amount = amm.quote_amount.checked_sub(output_amount_quote).unwrap();

    //     output_amount_quote
    // };

    // let new_k = (amm.base_amount as u128)
    //     .checked_mul(amm.quote_amount as u128)
    //     .unwrap();

    // assert!(new_k >= k); // with non-zero fees, k should always increase
    assert!(output_amount >= output_amount_min);

    Ok(())
}
