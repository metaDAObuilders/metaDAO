use super::*;

use anchor_lang::system_program;
use anchor_spl::token;

/// Structure defining the accounts required for initializing a conditional vault.
/// This represents the core setup for creating a vault that can manage conditional tokens
/// based on a specific question's outcomes.

#[event_cpi]
#[derive(Accounts)]
pub struct InitializeConditionalVault<'info> {
    /// The vault account that will be initialized and serve as the central
    /// authority for this prediction market instance.
    /// 
    /// # PDA Derivation
    /// Seeds:
    /// - PDA seeds: ["conditional_vault", question key, underlying token mint key]
    /// - PREFIX: "conditional_vault" (static identifier)
    /// - question.key(): Links vault to specific question
    /// - underlying_token_mint.key(): Associates with underlying token type/ collateral type
    /// 
    /// # Space Allocation
    /// - 8 bytes: Discriminator
    /// - ConditionalVault size: Core struct data
    /// - Dynamic size: 32 bytes per outcome for mint addresses
    /// 
    /// # Authorization
    /// - PDA acts as authority for all conditional token mints
    /// - Controls underlying token account
    /// - Manages token minting and burning operations
    #[account(
        init,
        payer = payer,
        space = 8 + std::mem::size_of::<ConditionalVault>() + (32 * question.num_outcomes()),
        seeds = [
            b"conditional_vault", 
            question.key().as_ref(),
            underlying_token_mint.key().as_ref(),
        ],
        bump
    )]
    pub vault: Box<Account<'info, ConditionalVault>>,

    /// The question account defining the prediction market parameters.
    /// 
    /// # Requirements
    /// - Must be initialized before vault creation
    /// - Determines number of conditional tokens
    /// - Defines resolution criteria
    /// 
    /// # Validation
    /// - Account must exist and be valid
    /// - Number of outcomes must be valid (≥2)
    /// - Cannot be modified after vault creation
    pub question: Account<'info, Question>,

    /// The mint account for the underlying token used as collateral.
    /// This is the token type that will be used for deposits and redemptions
    /// # Characteristics
    /// - Determines decimal precision for all conditional tokens
    /// - Must be a valid SPL token mint
    /// - Can be any fungible token 
    /// 
    /// # Implications
    /// - All conditional tokens will share same decimal precision
    /// - Affects minimum transaction amounts
    /// - Influences precision of payouts
    pub underlying_token_mint: Account<'info, Mint>,

    /// Associated Token Account for the vault's underlying tokens (collateral).
    /// 
    /// # Terminology
    /// - Underlying tokens = Collateral tokens
    /// Examples: 
    /// - USDC as underlying/collateral for a USDC-backed prediction market
    /// - SOL as underlying/collateral for a SOL-backed prediction market
    /// 
    /// # Configuration
    /// - Authority: Vault PDA (derived from question and underlying_token_mint)
    /// - Mint: Must match underlying_token_mint
    /// 
    /// # Account Properties
    /// - Associated Token Account (ATA) derived deterministically
    /// - Stores the underlying tokens (collateral) that back conditional tokens
    /// - Balance must satisfy invariant checks
    /// 
    /// # Usage Flow
    /// 1. Split Operation:
    ///    - User deposits underlying tokens (collateral)
    ///    - Vault mints conditional tokens
    /// 
    /// 2. Merge Operation:
    ///    - User burns equal amounts of all conditional tokens
    ///    - Vault returns underlying tokens (collateral)
    ///
    /// 3. Redemption (after resolution):
    ///    - User burns conditional tokens
    ///    - Vault pays out underlying tokens based on resolution ratios
    /// 
    /// # Invariant Requirements
    /// For unresolved questions:
    /// - Collateral must cover the highest conditional token supply
    /// Example: If YES=1000, NO=2000, collateral must be ≥ 2000
    /// 
    /// For resolved questions:
    /// - Collateral must cover weighted sum of all payouts
    /// Example: If supplies=[1000,2000] with 60/40 split
    /// Required collateral = (1000×0.6) + (2000×0.4) = 1400
    /// 
    /// # Security Considerations
    /// - Only vault PDA can authorize transfers
    /// - Collateral balance checked before every operation
    /// - Cannot be closed while conditional tokens exist
    /// - Must maintain solvency ratio at all times
    #[account(
        associated_token::authority = vault,
        associated_token::mint = underlying_token_mint
    )]
    pub vault_underlying_token_account: Box<Account<'info, TokenAccount>>,

    /// Account that will pay for the initialization costs
    /// 
    /// # Responsibilities
    /// - Funds vault account creation
    /// - Pays for conditional token mint accounts
    /// - Covers rent exemption costs
    /// 
    /// # Requirements
    /// - Must be a signer
    /// - Must have sufficient SOL for all operations
    #[account(mut)]
    pub payer: Signer<'info>,

    /// SPL Token program account for token operations.
    /// Required for:
    /// - Mint initialization
    /// - Token account creation
    /// - Transfer operations
    pub token_program: Program<'info, Token>,

    /// Associated Token Program for ATA operations.
    /// Used for:
    /// - ATA validation
    /// - ATA creation (if needed)
    /// - ATA constraint checking
    pub associated_token_program: Program<'info, AssociatedToken>,

    /// System Program for account operations.
    /// Required for:
    /// - Account creation
    /// - Lamport transfers
    /// - Space allocation
    pub system_program: Program<'info, System>,
}

impl<'info, 'c: 'info> InitializeConditionalVault<'info> {
    /// Handles the initialization of a new conditional vault in the prediction market system.
    /// This is a complex, multi-step process that sets up all necessary components for
    /// a functioning prediction market vault.
    ///
    /// # Initialization Process Overview
    /// 1. Vault Account Setup
    /// 2. Conditional Token Mint Creation
    /// 3. State Initialization
    /// 4. Event Emission
    ///
    /// # Security Considerations
    /// - All PDAs are verified for correctness
    /// - Rent exemption is guaranteed for all accounts
    /// - Authority relationships are properly established
    /// - No gaps in sequence numbers
    ///
    /// # Error Handling
    /// - Account validation errors
    /// - Insufficient funds errors
    /// - Space allocation errors
    /// - CPI (Cross-Program Invocation) errors
    ///
    /// # Returns
    /// * `Ok(())` if initialization succeeds
    /// * `Err(...)` if any step fails
    pub fn handle(ctx: Context<'_, '_, 'c, 'info, Self>) -> Result<()> {
        // Get mutable reference to the vault account being initialized
        let vault = &mut ctx.accounts.vault;

        // Get the decimals from the underlying token mint
        // This will be used for all conditional token mints
        let decimals = ctx.accounts.underlying_token_mint.decimals;

        // Setup iterator for remaining accounts (conditional token mints)
        let remaining_accs = &mut ctx.remaining_accounts.iter();

        // Get the number of outcomes from the question account
        // This determines how many conditional tokens we need to create
        let expected_num_conditional_tokens = ctx.accounts.question.num_outcomes();

        // Initialize vector to store conditional token mint addresses
        let mut conditional_token_mints = vec![];

        // Calculate minimum lamports needed for rent exemption of a mint account
        // This ensures accounts won't be purged
        let mint_lamports = Rent::get()?.minimum_balance(Mint::LEN);

        // Create a mint for each possible outcome
        for i in 0..expected_num_conditional_tokens {
            // Generate PDA for the conditional token mint
            // Seeds: ["conditional_token", vault key, outcome index]
            let (conditional_token_mint_address, pda_bump) = Pubkey::find_program_address(
                &[b"conditional_token", vault.key().as_ref(), &[i as u8]],
                ctx.program_id,
            );

            // Get the next account from remaining accounts and verify it matches expected PDA
            let conditional_token_mint = next_account_info(remaining_accs)?;
            require_eq!(conditional_token_mint.key(), conditional_token_mint_address);

            // Store the mint address in our vector
            conditional_token_mints.push(conditional_token_mint_address);

            // Transfer lamports to the mint account for rent exemption
            let cpi_accounts = system_program::Transfer {
                from: ctx.accounts.payer.to_account_info(),
                to: conditional_token_mint.to_account_info(),
            };
            let cpi_ctx =
                CpiContext::new(ctx.accounts.system_program.to_account_info(), cpi_accounts);
            system_program::transfer(cpi_ctx, mint_lamports)?;

            // Setup PDA signer seeds for the mint account
            let vault_key = vault.key();
            let seeds = &[
                b"conditional_token",
                vault_key.as_ref(),
                &[i as u8],
                &[pda_bump],
            ];
            let signer = &[&seeds[..]];

            // Allocate space for the mint account
            let cpi_accounts = system_program::Allocate {
                account_to_allocate: conditional_token_mint.to_account_info(),
            };
            let cpi_ctx =
                CpiContext::new(ctx.accounts.system_program.to_account_info(), cpi_accounts);
            system_program::allocate(cpi_ctx.with_signer(signer), Mint::LEN as u64)?;

            // Assign the mint account to the token program
            let cpi_accounts = system_program::Assign {
                account_to_assign: conditional_token_mint.to_account_info(),
            };
            let cpi_ctx =
                CpiContext::new(ctx.accounts.system_program.to_account_info(), cpi_accounts);
            system_program::assign(cpi_ctx.with_signer(signer), ctx.accounts.token_program.key)?;

            // Initialize the mint account with proper decimals and authority
            let cpi_program = ctx.accounts.token_program.to_account_info();
            let cpi_accounts = token::InitializeMint2 {
                mint: conditional_token_mint.to_account_info(),
            };
            let cpi_ctx = CpiContext::new(cpi_program, cpi_accounts);

            // When initializing conditional token mints, we explicitly set no freeze authority
            // to improve user experience in wallets like Phantom. Without this, users would see
            // a warning message about tokens being freezable.
            
            // Technical Details
            // - Mint authority: Set to vault PDA (required for minting/burning)
            // - Freeze authority: Set to None (prevents freezing capability)
            // - Decimals: Matched with underlying token
            
            // User Experience Impact
            // - Removes "asset can be frozen" warning in Phantom wallet
            // - Increases user trust in conditional tokens
            // - Maintains all necessary vault functionality

            // Security Considerations
            // - Vault retains mint/burn authority (required for core functionality)
            // - No capability to freeze tokens (improves user trust)
            // - No impact on vault solvency guarantees
            token::initialize_mint2(
                cpi_ctx,
                decimals,
                &vault.key(),  // mint authority = vault
                None,  // freeze authority = none
            )?;
        }

        // Initialize the vault account with all collected data
        vault.set_inner(ConditionalVault {
            question: ctx.accounts.question.key(),
            underlying_token_mint: ctx.accounts.underlying_token_mint.key(),
            underlying_token_account: ctx.accounts.vault_underlying_token_account.key(),
            conditional_token_mints,
            pda_bump: ctx.bumps.vault,
            decimals,
            seq_num: 0,
        });

        // Emit initialization event with timestamp and vault details
        let clock = Clock::get()?;
        emit_cpi!(InitializeConditionalVaultEvent {
            common: CommonFields {
                slot: clock.slot,
                unix_timestamp: clock.unix_timestamp,
            },
            vault: vault.key(),
            question: vault.question,
            underlying_token_mint: vault.underlying_token_mint,
            vault_underlying_token_account: vault.underlying_token_account,
            conditional_token_mints: vault.conditional_token_mints.clone(),
            pda_bump: vault.pda_bump,
            seq_num: vault.seq_num,
        });

        Ok(())
    }
}
