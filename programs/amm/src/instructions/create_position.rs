use anchor_lang::prelude::*;

use crate::error::ErrorCode;
use crate::state::*;

#[derive(Accounts)]
pub struct CreatePosition<'info> {
    #[account(mut)]
    pub user: Signer<'info>,
    pub amm: Account<'info, Amm>,
    #[account(
        init,
        payer = user,
        space = 8 + std::mem::size_of::<AmmPosition>(),
        seeds = [
            AMM_POSITION_SEED_PREFIX,
            amm.key().as_ref(),
            user.key().as_ref(),
        ],
        bump
    )]
    pub amm_position: Account<'info, AmmPosition>,
    pub system_program: Program<'info, System>,
}

pub fn handler(ctx: Context<CreatePosition>) -> Result<()> {
    let CreatePosition {
        user,
        amm,
        amm_position,
        system_program: _,
    } = ctx.accounts;

    amm_position.user = user.key();
    amm_position.amm = amm.key();
    amm_position.ownership = 0;

    Ok(())
}
