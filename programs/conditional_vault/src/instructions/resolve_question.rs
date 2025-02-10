use super::*;

/// Arguments required for resolving a question.
/// Contains the payout ratios for each possible outcome.
#[derive(AnchorSerialize, AnchorDeserialize)]
pub struct ResolveQuestionArgs {
    /// Vector of payout numerators for each outcome
    /// Length must match number of outcomes in question
    /// Sum of numerators becomes the denominator
    /// Example: [70, 30] for a 70/30 split
    pub payout_numerators: Vec<u32>,
}


/// Account context for resolving a question in the prediction market.
/// Only the designated oracle can resolve the question.
#[event_cpi]
#[derive(Accounts)]
#[instruction(args: ResolveQuestionArgs)]
pub struct ResolveQuestion<'info> {
    /// The question account to be resolved
    /// Must be mutable to update resolution state
    /// Must be owned by the oracle attempting resolution
    #[account(mut, has_one = oracle)]
    pub question: Account<'info, Question>,

    /// The oracle account authorized to resolve this question
    /// Must be a signer to prove authority
    pub oracle: Signer<'info>,
}

impl ResolveQuestion<'_> {
    /// Handles the resolution of a question by setting final outcome payouts.
    /// This is a critical operation that determines how conditional tokens
    /// can be redeemed for underlying tokens.
    ///
    /// # Mathematical Model
    /// For each outcome i:
    /// payout_ratio_i = payout_numerator_i / sum(payout_numerators)
    /// redemption_value = token_amount * payout_ratio
    ///
    /// # Arguments
    /// * `ctx` - Context containing question and oracle accounts
    /// * `args` - Resolution arguments containing payout numerators
    ///
    /// # Validation Steps
    /// 1. Ensures question hasn't been previously resolved
    /// 2. Validates number of payout numerators matches outcomes
    /// 3. Verifies non-zero total payout
    ///
    /// # Example Resolutions
    /// ```ignore
    /// // Binary outcome (70/30 split):
    /// ResolveQuestionArgs {
    ///     payout_numerators: vec![70, 30]  // Denominator becomes 100
    /// }
    ///
    /// // Three-way split (50/30/20):
    /// ResolveQuestionArgs {
    ///     payout_numerators: vec![50, 30, 20]  // Denominator becomes 100
    /// }
    ///
    /// // Scalar outcome (partial correctness):
    /// ResolveQuestionArgs {
    ///     payout_numerators: vec![75, 25]  // 75% correct prediction
    /// }
    /// ```
    ///
    /// # Errors
    /// * `VaultError::QuestionAlreadyResolved` - Question was previously resolved
    /// * `VaultError::InvalidNumPayoutNumerators` - Wrong number of payouts
    /// * `VaultError::PayoutZero` - Sum of payouts is zero
    pub fn handle(ctx: Context<Self>, args: ResolveQuestionArgs) -> Result<()> {
        let question = &mut ctx.accounts.question;

        // Verify question hasn't been resolved already
        // payout_denominator == 0 indicates unresolved state
        require_eq!(question.payout_denominator, 0, VaultError::QuestionAlreadyResolved);

        // Ensure number of payout numerators matches number of outcomes
        require_eq!(
            args.payout_numerators.len(),
            question.num_outcomes(),
            VaultError::InvalidNumPayoutNumerators
        );

        // Set resolution values:
        // - denominator becomes sum of all numerators
        // - store provided numerators
        question.payout_denominator = args.payout_numerators.iter().sum();
        question.payout_numerators = args.payout_numerators.clone();

        // Ensure total payout is non-zero
        // This prevents division by zero in redemption calculations
        require_gt!(question.payout_denominator, 0, VaultError::PayoutZero);

        // Emit resolution event for indexing and tracking
        let clock = Clock::get()?;
        emit_cpi!(ResolveQuestionEvent {
            common: CommonFields {
                slot: clock.slot,
                unix_timestamp: clock.unix_timestamp,
            },
            question: question.key(),
            payout_numerators: args.payout_numerators,
        });

        Ok(())
    }
}
