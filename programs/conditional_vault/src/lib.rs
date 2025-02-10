use anchor_lang::prelude::*;
use anchor_spl::metadata::{
    create_metadata_accounts_v3, mpl_token_metadata::types::DataV2, CreateMetadataAccountsV3,
    Metadata,
};
use anchor_spl::{
    associated_token::AssociatedToken,
    token::{self, Burn, Mint, MintTo, Token, TokenAccount, TransferChecked},
};

pub mod error;
pub mod instructions;
pub mod state;
pub mod events;

pub use error::VaultError;
pub use instructions::*;
pub use state::*;
pub use events::*;

#[cfg(not(feature = "no-entrypoint"))]
use solana_security_txt::security_txt;

#[cfg(not(feature = "no-entrypoint"))]
security_txt! {
    name: "conditional_vault",
    project_url: "https://metadao.fi",
    contacts: "email:metaproph3t@protonmail.com",
    policy: "The market will decide whether we pay a bug bounty.",
    source_code: "https://github.com/metaDAOproject/futarchy",
    source_release: "v0.4",
    auditors: "Neodyme (v0.3)",
    acknowledgements: "DCF = (CF1 / (1 + r)^1) + (CF2 / (1 + r)^2) + ... (CFn / (1 + r)^n)"
}

declare_id!("VLTX1ishMBbcX3rdBWGssxawAo1Q2X2qxYFYqiGodVg");

#[program]
pub mod conditional_vault {
    use super::*;

    pub fn initialize_question(
        ctx: Context<InitializeQuestion>,
        args: InitializeQuestionArgs,
    ) -> Result<()> {
        InitializeQuestion::handle(ctx, args)
    }

    pub fn resolve_question(
        ctx: Context<ResolveQuestion>,
        args: ResolveQuestionArgs,
    ) -> Result<()> {
        ResolveQuestion::handle(ctx, args)
    }

    pub fn initialize_conditional_vault<'c: 'info, 'info>(
        ctx: Context<'_, '_, 'c, 'info, InitializeConditionalVault<'info>>,
    ) -> Result<()> {
        InitializeConditionalVault::handle(ctx)
    }

    pub fn split_tokens<'c: 'info, 'info>(
        ctx: Context<'_, '_, 'c, 'info, InteractWithVault<'info>>,
        amount: u64,
    ) -> Result<()> {
        InteractWithVault::handle_split_tokens(ctx, amount)
    }

    pub fn merge_tokens<'c: 'info, 'info>(
        ctx: Context<'_, '_, 'c, 'info, InteractWithVault<'info>>,
        amount: u64,
    ) -> Result<()> {
        InteractWithVault::handle_merge_tokens(ctx, amount)
    }

    #[access_control(ctx.accounts.validate_redeem_tokens())]
    pub fn redeem_tokens<'c: 'info, 'info>(
        ctx: Context<'_, '_, 'c, 'info, InteractWithVault<'info>>,
    ) -> Result<()> {
        InteractWithVault::handle_redeem_tokens(ctx)
    }

    #[access_control(ctx.accounts.validate())]
    pub fn add_metadata_to_conditional_tokens(
        ctx: Context<AddMetadataToConditionalTokens>,
        args: AddMetadataToConditionalTokensArgs,
    ) -> Result<()> {
        AddMetadataToConditionalTokens::handle(ctx, args)
    }
}
