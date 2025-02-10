use super::*;

/// Represents the operational status of a conditional vault.
///
/// # States
/// * `Active` - Normal operational state, allows token minting/burning
/// * `Finalized` - Completed state, no further modifications allowed
/// * `Reverted` - Cancelled state, indicates vault termination
///
/// # State Transitions
/// ```text
/// Active → Finalized (After successful resolution)
/// Active → Reverted  (In case of cancellation)
/// ```
///
/// # Usage
/// Used to control vault operations and permissions.
#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, PartialEq, Eq)]
pub enum VaultStatus {
    /// Vault is operational
    Active,
    /// Vault has been completed
    Finalized,
    /// Vault has been reversed/cancelled
    Reverted,
}

/// The structure that represents a conditional vault in the prediction market.
/// Manages the relationship between underlying tokens and conditional tokens
/// representing different outcomes.
///
/// # Architecture
/// The vault serves as an escrow system that:
/// 1. Holds underlying tokens as collateral
/// 2. Issues conditional tokens for different outcomes
/// 3. Maintains solvency through invariant checks
/// 4. Handles redemptions after question resolution
///
/// # Account Structure
/// * `question` - Link to the associated question account
/// * `underlying_token_mint` - The collateral token's mint
/// * `underlying_token_account` - Vault's underlying token account/ collateral storage
/// * `conditional_token_mints` - Outcome token mints
/// * `pda_bump` - For PDA derivation
/// * `decimals` - Precision level for all tokens
/// * `seq_num` - Operation sequence tracker
///
/// # Security Model
/// * PDA-based authorization
/// * Invariant-based solvency protection
/// * Atomic operations
/// * Balance verification
#[account]
pub struct ConditionalVault {
    /// Public key of the associated question
    pub question: Pubkey,
    /// Public key of the main token mint
    pub underlying_token_mint: Pubkey,
    /// Public key of the token account
    pub underlying_token_account: Pubkey,
    /// Vector of public keys for conditional tokens
    pub conditional_token_mints: Vec<Pubkey>,
    /// Program Derived Address bump seed
    pub pda_bump: u8,
    /// Token decimal places
    pub decimals: u8,
    /// Sequence number
    pub seq_num: u64,
}

impl ConditionalVault {
    /// Checks that the vault's assets are always greater than its potential
    /// liabilities. Should be called anytime you mint or burn conditional
    /// tokens.
    ///
    /// `conditional_token_supplies` should be in the same order as
    /// `vault.conditional_token_mints`.
    /// 
    /// Typically, it validates the vault's solvency by ensuring assets cover potential liabilities.
    /// This critical safety check prevents the vault from becoming insolvent.
    ///
    /// # Arguments
    /// * `question` - Reference to associated question account
    /// * `conditional_token_supplies` - Vector of current token supplies
    /// * `vault_underlying_balance` - Current vault underlying token balance/ collateral balance
    ///
    /// # Behaviors
    /// ## Unresolved Question State
    /// * Maximum liability equals highest individual token supply
    /// * Any outcome could potentially win with 100% payout
    /// * Must cover worst-case scenario
    ///
    /// ## Resolved Question State
    /// * Liability calculated using actual payout ratios
    /// * Weighted sum of token supplies * payout ratios
    /// * Must cover exact payout requirements
    ///
    /// # Examples
    /// ## Unresolved Binary Market
    /// ```ignore
    /// // Market: "Will SOL > $50k?"
    /// let supplies = vec![1000, 2000];  // [YES, NO] tokens
    /// let balance = 2000;  // Underlying tokens
    /// 
    /// // Safe because balance covers max supply (2000)
    /// vault.invariant(&question, supplies, balance)?;
    /// ```
    ///
    /// ## Resolved Three-Way Split
    /// ```ignore
    /// let supplies = vec![1000, 2000, 3000];
    /// let payouts = vec![30, 50, 20];  // 30%, 50%, 20%
    /// let denominator = 100;
    /// 
    /// // Liability calculation:
    /// // (1000 * 0.3) + (2000 * 0.5) + (3000 * 0.2) = 1900
    /// ```
    ///
    /// # Security Considerations
    /// * Uses u128 for intermediate calculations to prevent overflow
    /// * Checks performed before any mint/burn operations
    /// * Maintains safety margin in unresolved state
    /// * Prevents double-redemption through supply tracking
    pub fn invariant(
        &self,
        question: &Question,
        conditional_token_supplies: Vec<u64>,
        vault_underlying_balance: u64,
    ) -> Result<()> {
        let max_possible_liability = if !question.is_resolved() {
            // Unresolved state: Use maximum supply as liability            
            // safe because conditional_token_supplies is non-empty
            *conditional_token_supplies.iter().max().unwrap()
        } else {
            // Resolved state: Calculate weighted sum of liabilities
            conditional_token_supplies
                .iter()
                .enumerate()
                .map(|(i, supply)| {
                    *supply as u128 * question.payout_numerators[i] as u128
                        / question.payout_denominator as u128
                })
                .sum::<u128>() as u64
        };

        // Ensures vault has enough underlying tokens to cover obligations
        require_gte!(vault_underlying_balance, max_possible_liability, VaultError::AssertFailed);

        Ok(())
    }
}

/// Generates deterministic seeds for vault PDA derivation.
/// 
/// # Seeds Structure
/// 1. `b"conditional_vault"` - Program identifier
/// 2. `question.key()` - Links to specific question
/// 3. `underlying_token_mint.key()` - Links to token type
/// 4. `bump` - Ensures valid PDA
///
/// # Usage
/// ```ignore
/// let seeds = generate_vault_seeds!(vault);
/// let signer_seeds = &[&seeds[..]];
/// ```
///
/// # Security
/// * Deterministic generation prevents address collisions
/// * Incorporates all relevant identifiers
/// * Used for signing vault operations
#[macro_export]
macro_rules! generate_vault_seeds {
    ($vault:expr) => {{
        &[
            b"conditional_vault",
            $vault.question.as_ref(),
            $vault.underlying_token_mint.as_ref(),
            &[$vault.pda_bump],
        ]
    }};
}
