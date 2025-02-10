use super::*;

/// # Summary
/// Questions represent statements about future events.
///
/// These statements include:
/// - "Will this proposal pass?"
/// - "Who, if anyone, will be hired?"
/// - "How effective will the grant committee deem this grant?"
///
/// Questions have 2 or more possible outcomes. For a question like "will this
/// proposal pass," the outcomes are "yes" and "no." For a question like "who
/// will be hired," the outcomes could be "Alice," "Bob," and "neither."
///
/// Outcomes resolve to a number between 0 and 1. Binary questions like "will
/// this proposal pass" have outcomes that resolve to exactly 0 or 1. You can
/// also have questions with scalar outcomes. For example, the question "how
/// effective will the grant committee deem this grant" could have two outcomes:
/// "ineffective" and "effective." If the grant committee deems the grant 70%
/// effective, the "effective" outcome would resolve to 0.7 and the "ineffective"
/// outcome would resolve to 0.3.
///
/// Once resolved, the sum of all outcome resolutions is exactly 1.

/// # More Details 
/// A Question represents a prediction market's core inquiry and its resolution parameters.
///
/// # Overview
/// Questions are statements about future events that can resolve to multiple outcomes,
/// each with an associated payout weight. The payout system uses fractions to represent 
/// outcome values, enabling precise calculations without floating-point arithmetic.
///
/// # Structure
/// * `question_id`: [u8; 32] - Unique identifier for the question
/// * `oracle`: Pubkey - Authorized resolver's public key
/// * `payout_numerators`: Vec<u32> - Outcome weights (numerators for payout fractions)
/// * `payout_denominator`: u32 - Shared denominator for payout calculations
///
/// # Payout Mechanics
/// Each outcome's value is calculated as: `payout_numerator / payout_denominator`
///
/// ## Resolution States
/// * Unresolved: `payout_denominator = 0`
/// * Resolved: `payout_denominator > 0`
///
/// ## Constraints
/// * Sum of all payouts must equal 1 (or denominator in fractional form)
/// * Minimum of 2 outcomes required
/// * All numerators must be less than or equal to denominator
///
/// # Examples
/// 
/// ## Binary Outcome (Yes/No)
/// ```ignore
/// let binary_question = Question {
///     question_id: [0; 32],
///     oracle: oracle_pubkey,
///     payout_numerators: vec![1, 0],  // Yes = 1, No = 0, Therefore, Yes wins (100%)
///     payout_denominator: 1,
/// };
/// ```
/// The outcome is rigid, with only two options: full payout (100%) or no payout (0%).
///
/// ## Percentage Split
/// ```ignore
/// let percentage_question = Question {
///     question_id: [0; 32],
///     oracle: oracle_pubkey,
///     payout_numerators: vec![70, 30],  // 70/30 split
///     payout_denominator: 100,
/// };
/// ```
/// Calculations:
/// Outcome 1 value = 30/100 = 0.3
/// Outcome 2 value = 70/100 = 0.7
///
/// The outcome weighting is flexible, allowing each outcome to represent a percentage of the payout.
/// 
/// ## Multiple Outcomes
/// ```ignore
/// let multi_outcome = Question {
///     question_id: [0; 32],
///     oracle: oracle_pubkey,
///     payout_numerators: vec![50, 30, 20],  // Three-way split
///     payout_denominator: 100,
/// };
/// ```
///
/// # Real-World Applications
/// 
/// ## Election Predictions
/// Question: "Who will win the election - Alice, Bob, or Carol?"
/// 
/// ```ignore
/// let election = Question {
///     payout_numerators: vec![60, 40, 0],  // Alice 60%, Bob 40%, Carol 0%
///     payout_denominator: 100,
///     // ... other fields ...
/// };
/// ```
/// Calculations:
/// Alice's outcome = 60/100 = 0.6 (60%)
/// Bob's outcome = 40/100 = 0.4 (40%)
/// Carol's outcome = 0/100 = 0 (0%)
///
/// ## Grant Effectiveness
/// ```ignore
/// let grant = Question {
///     payout_numerators: vec![20, 45, 35],  // Low/Medium/High effectiveness
///     payout_denominator: 100,
///     // ... other fields ...
/// };
/// ```
///
/// # Liability Calculation
/// The question's payout structure determines potential liabilities in the vault system.
/// 
/// ## Formula
/// For each outcome i:
/// ```text
/// liability_i = token_supply_i * (payout_numerator_i / payout_denominator)
/// total_liability = sum(liability_i)
/// ```
///
/// ## Example Calculation
/// Given:
/// * Token supplies: [1000, 2000, 3000]
/// * Payout numerators: [20, 30, 50]
/// * Payout denominator: 100
///
/// Liabilities:
/// ```text
/// 1000 * (20/100) = 200
/// 2000 * (30/100) = 600
/// 3000 * (50/100) = 1500
/// Total = 2300
/// ```
///
/// # Security Considerations
/// * Oracle has exclusive resolution authority
/// * Payout values are immutable after resolution
/// * Denominator of 0 prevents premature redemptions
/// * Integer arithmetic prevents rounding errors
///
/// # Implementation Note
/// All calculations use integer arithmetic to maintain precision.
/// Intermediate calculations use u128 to prevent overflow.
#[account]
pub struct Question {
    /// 32 byte unique identifier for the question
    pub question_id: [u8; 32], 
    /// the public key of the oracle authorized to resolve the question
    pub oracle: Pubkey, 
    /// vector of unsigned 32-bit integers (numerators) representing outcome weights
    pub payout_numerators: Vec<u32>, 
    /// the unsigned 32-bit integer (denominator) used to calculate final outcome values
    pub payout_denominator: u32, 
}

/// The payout system uses a fraction-based approach.
/// Each outcome's value = payout_numerators/payout_denominator
/// This allows for precise representation of decimal values without floating-point math.
/// For example, a 70% resolution might use `numerator = 70`, `denominator = 100`.
///
/// Examples
///
/// Definitive "Yes" (100%) or "No" (0%) payout:
/// Question: "Did the project meet its goals?"
/// ```ignore
/// let project_question = Question {
///     payout_numerators: vec![0, 1],  // No = 0, Yes = 1
///     payout_denominator: 1
/// };
/// ```
///
/// The outcome is rigid, with only two options: full payout (100%) or no payout (0%).
///
/// Two Outcomes:
/// ```ignore
/// let question = Question {
///     question_id: [0; 32],
///     oracle: some_pubkey,
///     payout_numerators: vec![30, 70],
///     payout_denominator: 100
/// };
/// ```
///
/// Calculations:
/// Outcome 1 value = 30/100 = 0.3
/// Outcome 2 value = 70/100 = 0.7
///
/// The outcome weighting is flexible, allowing each outcome to represent a percentage of the payout.
///
/// Three Outcomes:
/// ```rust
/// let question = Question {
///     question_id: [0; 32],
///     oracle: some_pubkey,
///     payout_numerators: vec![50, 25, 25],  // Three outcomes
///     payout_denominator: 100
/// };
/// ```
///
/// Calculations:
/// Outcome 1 value = 50/100 = 0.5
/// Outcome 2 value = 25/100 = 0.25
/// Outcome 3 value = 25/100 = 0.25

/// Real Scenarios:
/// Question: "Who will win the election - Alice, Bob, or Carol?"
/// Alice 60%, Bob 40%, Carol 0%
/// ```rust
/// let election_question = Question {
///     payout_numerators: vec![60, 40, 0],
///     payout_denominator: 100
/// };
/// ```

/// Calculations:
/// Alice's outcome = 60/100 = 0.6 (60%)
/// Bob's outcome = 40/100 = 0.4 (40%)
/// Carol's outcome = 0/100 = 0 (0%)

/// Question: "How effective was this grant?"
/// ```rust
/// let grant_question = Question {
///     payout_numerators: vec![20, 45, 35],  // Low, Medium, High effectiveness
///     payout_denominator: 100
/// };
/// ```

/// Calculations:
/// Low effectiveness = 20/100 = 0.2 (20%)
/// Medium effectiveness = 45/100 = 0.45 (45%)
/// High effectiveness = 35/100 = 0.35 (35%)


impl Question {
    /// Returns the number of possible outcomes for this question.
    ///
    /// # Returns
    /// * `usize` - The number of outcomes, derived from payout_numerators length
    ///
    /// # Validation
    /// The implementation ensures:
    /// * Minimum of 2 outcomes
    /// * Matches the number of conditional tokens in associated vaults
    pub fn num_outcomes(&self) -> usize {
        self.payout_numerators.len()
    }

    /// Checks if the question has been resolved by the oracle.
    ///
    /// # Returns
    /// * `bool` - true if resolved (denominator > 0), false otherwise
    ///
    /// # Resolution States
    /// * Unresolved: `payout_denominator = 0`
    /// * Resolved: `payout_denominator > 0`
    ///
    /// # Example
    /// ```ignore
    /// // Unresolved state
    /// assert_eq!(Question {
    ///     payout_numerators: vec![0, 0],
    ///     payout_denominator: 0,
    ///     // ... other fields ...
    /// }.is_resolved(), false);
    ///
    /// // Resolved state
    /// assert_eq!(Question {
    ///     payout_numerators: vec![1, 0],
    ///     payout_denominator: 1,
    ///     // ... other fields ...
    /// }.is_resolved(), true);
    /// ```
    pub fn is_resolved(&self) -> bool {
        self.payout_denominator != 0
    }
}
