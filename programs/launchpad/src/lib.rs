//! A smart contract that facilitates the creation of new futarchic DAOs via ICO.
//! 
//! Creators can then create a `Launch` account, specifying the minimum and maximum to raise.
//! `Launch` accounts are associated with a `Dao` account, which is where the USDC will be
//! sent if the launch is successful. 
//! 
//! Funders can then contribute to the `Launch` account and receive tokens in return.
//! They receive 10,000 tokens per USDC contributed, so a price of $0.0001 per token.
//! 
//! At the end, 10% of the USDC and an equivalent amount of tokens are put into a Raydium
//! 1% Pool.
use anchor_lang::prelude::*;

pub mod state;
pub mod instructions;
pub mod error;
pub mod events;

use instructions::*;

declare_id!("AfJJJ5UqxhBKoE3grkKAZZsoXDE9kncbMKvqSHGsCNrE");

#[program]
pub mod launchpad {
    use super::*;

    #[access_control(ctx.accounts.validate(args))]
    pub fn initialize_launch(
        ctx: Context<InitializeLaunch>,
        args: InitializeLaunchArgs,
    ) -> Result<()> {
        InitializeLaunch::handle(ctx, args)
    }
}
