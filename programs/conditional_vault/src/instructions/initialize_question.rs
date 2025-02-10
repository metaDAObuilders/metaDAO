use super::*;

/// Arguments required for initializing a new question in the prediction market.
/// This structure defines the core parameters that uniquely identify and configure
/// a question instance.
#[derive(AnchorSerialize, AnchorDeserialize)]
pub struct InitializeQuestionArgs {
    /// A unique 32-byte identifier for the question
    /// This can be derived from a hash of the question text or other unique parameters
    pub question_id: [u8; 32],

    /// The public key of the oracle authority
    /// This account will have the exclusive permission to resolve the question
    /// by setting the final outcome values
    pub oracle: Pubkey,
    
    /// The number of possible outcomes for this question
    /// For example:
    /// - Binary questions (Yes/No) would have 2 outcomes
    /// - Non binary questions might have 3+ outcomes
    /// Must be >= 2 to be valid
    pub num_outcomes: u8,
}

/// Instruction context for initializing a new question in the prediction market system.
/// This structure validates and processes all accounts needed for question creation.
#[event_cpi]
#[derive(Accounts)]
#[instruction(args: InitializeQuestionArgs)]
pub struct InitializeQuestion<'info> {
    /// The question account to be initialized
    /// Space calculation breakdown:
    /// - 8 bytes: Account discriminator
    /// - 32 bytes: question_id ([u8; 32])
    /// - 32 bytes: oracle (Pubkey)
    /// - 1 byte: padding
    /// - 4 bytes: Vec prefix for payout_numerators
    /// - (num_outcomes * 4) bytes: payout_numerators (Vec<u32>)
    /// - 4 bytes: payout_denominator (u32)
    #[account(
        init,
        payer = payer,
        space = 8 + 32 + 32 + 1 + 4 + (args.num_outcomes as usize * 4) + 4,
        seeds = [
            b"question", 
            args.question_id.as_ref(),
            args.oracle.key().as_ref(),
            &[args.num_outcomes],
        ],
        bump
    )]
    pub question: Box<Account<'info, Question>>,

    /// The account that will pay for account initialization
    /// Must be a signer and must have sufficient SOL to cover rent
    #[account(mut)]
    pub payer: Signer<'info>,

    /// The system program account
    /// Required for account initialization
    pub system_program: Program<'info, System>,
}

impl InitializeQuestion<'_> {
    /// Handles the initialization of a new question in the prediction market.
    /// 
    /// # Arguments
    /// 
    /// * `ctx` - The context containing all relevant accounts for initialization
    /// * `args` - The initialization arguments containing question parameters
    ///
    /// # Steps
    /// 1. Validates the number of outcomes is at least 2
    /// 2. Initializes the question account with default values
    /// 3. Emits an event logging the question creation
    ///
    /// # Example
    /// ```ignore
    /// let args = InitializeQuestionArgs {
    ///     question_id: [0; 32],  // Unique identifier
    ///     oracle: oracle_pubkey,  // Oracle authority
    ///     num_outcomes: 2,        // Binary question (Yes/No)
    /// };
    /// initialize_question(ctx, args)?;
    /// ```
    ///
    /// # Errors
    /// Returns `VaultError::InsufficientNumConditions` if num_outcomes < 2
    pub fn handle(ctx: Context<Self>, args: InitializeQuestionArgs) -> Result<()> {
        // Validate that there are at least 2 possible outcomes
        // This ensures the question is meaningful and can be resolved
        require_gte!(
            args.num_outcomes,
            2,
            VaultError::InsufficientNumConditions
        );

        // Get mutable reference to the question account
        let question = &mut ctx.accounts.question;

        // Destructure the initialization arguments for clarity
        let InitializeQuestionArgs {
            question_id,
            oracle,
            num_outcomes,
        } = args;


        // Initialize the question account with default values:
        // - payout_numerators initialized to zero for each outcome
        // - payout_denominator set to 0 indicating unresolved state
        question.set_inner(Question {
            question_id,
            oracle,
            payout_numerators: vec![0; num_outcomes as usize],
            payout_denominator: 0,
        });

        // Emit an event to log the question initialization
        // This helps with indexing and tracking question creation
        let clock = Clock::get()?;
        emit_cpi!(InitializeQuestionEvent {
            common: CommonFields {
                slot: clock.slot,
                unix_timestamp: clock.unix_timestamp,
            },
            question_id,
            oracle,
            num_outcomes,
            question: question.key(),
        });

        Ok(())
    }
}
