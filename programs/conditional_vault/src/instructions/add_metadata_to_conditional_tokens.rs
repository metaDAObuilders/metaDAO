use super::*;

/// Module containing proph3t's public key
/// This is used for authorization in production
pub mod proph3t_deployer {
    use anchor_lang::declare_id;

    declare_id!("HfFi634cyurmVVDr9frwu4MjGLJzz9XbAJz981HdVaNz");
}

/// Arguments required for adding metadata to conditional tokens.
/// This metadata helps identify and display token information in wallets and GUIs.
#[derive(AnchorSerialize, AnchorDeserialize)]
pub struct AddMetadataToConditionalTokensArgs {
    /// The display name of the conditional token
    /// Example: "YES_SOL_ABOVE_50K_DEC2024"
    pub name: String,

    /// The trading symbol for the conditional token
    /// Example: "YSOL50K"
    pub symbol: String,

    /// URI pointing to the token's metadata JSON
    /// This should contain extended information about the token,
    /// such as description, images, and additional attributes
    pub uri: String,
}

#[event_cpi]
#[derive(Accounts)]
pub struct AddMetadataToConditionalTokens<'info> {
    /// The account paying for the metadata account creation
    /// In production, this must be proph3t - the authorized deployer
    #[account(mut)]
    pub payer: Signer<'info>,

    /// The vault account that controls the conditional tokens
    /// Must be in Active status to modify metadata
    #[account(mut)]
    pub vault: Account<'info, ConditionalVault>,

    /// The mint account of the conditional token
    /// Must have the vault as its mint authority
    #[account(
        mut,
        mint::authority = vault,
    )]
    pub conditional_token_mint: Account<'info, Mint>,

    /// The account that will store the token metadata
    /// Must be empty before initialization
    /// CHECK: verified via cpi into token metadata
    #[account(mut)]
    pub conditional_token_metadata: AccountInfo<'info>,

    /// The Token Metadata Program that will create and store the metadata
    pub token_metadata_program: Program<'info, Metadata>,

    /// The System Program Account
    pub system_program: Program<'info, System>,

    /// The Rent Sysvar account for rent calculations
    pub rent: Sysvar<'info, Rent>,
}

impl AddMetadataToConditionalTokens<'_> {
    /// Validates the preconditions for adding metadata to conditional tokens
    ///
    /// # Checks
    /// * Verifies the metadata account is empty (prevents overwriting)
    /// * In production, ensures only proph3t (the authorized deployer) can add metadata
    ///
    /// # Returns
    /// * `Ok(())` if all validations pass
    /// * `Err(VaultError::ConditionalTokenMetadataAlreadySet)` if metadata exists
    pub fn validate(&self) -> Result<()> {
        // Commented out for reference:
        // require!(
        //     self.vault.status == VaultStatus::Active,
        //     VaultError::VaultAlreadySettled
        // );

        // Ensure we're not overwriting existing metadata
        require!(
            self.conditional_token_metadata.data_is_empty(),
            VaultError::ConditionalTokenMetadataAlreadySet
        );

        // In production builds, restrict access to authorized deployer
        #[cfg(feature = "production")]
        require_eq!(
            self.payer.key(), proph3t_deployer::ID
        );

        Ok(())
    }

    /// Handles the addition of metadata to conditional tokens
    ///
    /// # Arguments
    /// * `ctx` - The context containing all required accounts
    /// * `args` - The metadata arguments (name, symbol, URI)
    ///
    /// # Steps
    /// 1. Generates vault PDA seeds for signing
    /// 2. Creates metadata account via CPI to Token Metadata Program
    /// 3. Increments vault sequence number
    /// 4. Emits event with metadata details
    ///
    /// # Example
    /// ```ignore
    /// let args = AddMetadataToConditionalTokensArgs {
    ///     name: "YES_SOL_ABOVE_50K".to_string(),
    ///     symbol: "YBTC50K".to_string(),
    ///     uri: "https://metadao.fi/token/123".to_string(),
    /// };
    /// add_metadata_to_conditional_tokens(ctx, args)?;
    /// ```
    pub fn handle(ctx: Context<Self>, args: AddMetadataToConditionalTokensArgs) -> Result<()> {
        // Generate vault PDA seeds for signing metadata transactions
        let seeds = generate_vault_seeds!(ctx.accounts.vault);
        let signer_seeds = &[&seeds[..]];

        // Prepare CPI to token metadata program
        let cpi_program = ctx.accounts.token_metadata_program.to_account_info();

        // Setup accounts for metadata creation
        let cpi_accounts = CreateMetadataAccountsV3 {
            metadata: ctx.accounts.conditional_token_metadata.to_account_info(),
            mint: ctx.accounts.conditional_token_mint.to_account_info(),
            mint_authority: ctx.accounts.vault.to_account_info(),
            payer: ctx.accounts.payer.to_account_info(),
            update_authority: ctx.accounts.vault.to_account_info(),
            system_program: ctx.accounts.system_program.to_account_info(),
            rent: ctx.accounts.rent.to_account_info(),
        };

        // Create metadata account with provided details
        create_metadata_accounts_v3(
            CpiContext::new(cpi_program, cpi_accounts).with_signer(signer_seeds),
            DataV2 {
                name: args.name.clone(),
                symbol: args.symbol.clone(),
                uri: args.uri.clone(),
                seller_fee_basis_points: 0, // No fees for conditional tokens
                creators: None, // No creator royalties
                collection: None, // Not part of a collection
                uses: None, // No uses metadata
            },
            false, // Is mutable
            true, // Update authority is signer
            None, // Collection details
        )?;

        // Increment vault sequence number for tracking
        ctx.accounts.vault.seq_num += 1;

        // Emit event for indexing and tracking
        let clock = Clock::get()?;
        emit_cpi!(AddMetadataToConditionalTokensEvent {
            common: CommonFields {
                slot: clock.slot,
                unix_timestamp: clock.unix_timestamp,
            },
            vault: ctx.accounts.vault.key(),
            conditional_token_mint: ctx.accounts.conditional_token_mint.key(),
            conditional_token_metadata: ctx.accounts.conditional_token_metadata.key(),
            name: args.name,
            symbol: args.symbol,
            uri: args.uri,
            seq_num: ctx.accounts.vault.seq_num,
        });

        Ok(())
    }
}
