use anchor_lang::prelude::*;

declare_id!("GW1r76tkZDNpdKf7BD7ap1EtPvnQb592apWuaKWCyckd");

const MAX_QUESTION_LENGTH: usize = 200;
const MAX_OPTION_LENGTH: usize = 30;
const MAX_OPTIONS: usize = 4;

#[program]
pub mod voting_platform {
    use super::*;

    pub fn create_poll(
        ctx: Context<CreatePoll>,
        poll_id: u64,
        question: String,
        options: Vec<String>,
        duration: i64,
    ) -> Result<()> {
        // Validate inputs
        require!(question.len() <= MAX_QUESTION_LENGTH, VotingError::QuestionTooLong);
        require!(options.len() >= 2, VotingError::InsufficientOptions);
        require!(options.len() <= MAX_OPTIONS, VotingError::TooManyOptions);

        for option in &options {
            require!(option.len() <= MAX_OPTION_LENGTH, VotingError::OptionTooLong);
        }

        let poll = &mut ctx.accounts.poll;
        
        poll.poll_id = poll_id;
        poll.creator = ctx.accounts.creator.key();
        poll.created_at = Clock::get()?.unix_timestamp;
        poll.option_count = options.len() as u8;
        
        // Copy question to fixed-size array
        let question_bytes = question.as_bytes();
        poll.question[..question_bytes.len()].copy_from_slice(question_bytes);
        poll.question_length = question_bytes.len() as u16;
        
        // Copy options to fixed-size arrays
        for (i, option) in options.iter().enumerate() {
            let option_bytes = option.as_bytes();
            poll.options[i][..option_bytes.len()].copy_from_slice(option_bytes);
            poll.option_lengths[i] = option_bytes.len() as u8;
        }
        
        Ok(())
    }

    pub fn cast_vote(
        ctx: Context<CastVote>,
        option_index: u8,
    ) -> Result<()> {
        let poll = &mut ctx.accounts.poll;
        let voter_record = &mut ctx.accounts.voter_record;
        let clock = Clock::get()?;
            
        require!(
            (option_index as usize) < poll.option_count as usize,
            VotingError::InvalidOption
        );

        require!(!voter_record.has_voted, VotingError::AlreadyVoted);
        

        poll.votes[option_index as usize] += 1;
        
        voter_record.has_voted = true;
        voter_record.voted_option = option_index;
        voter_record.voted_at = clock.unix_timestamp;
        voter_record.voter = ctx.accounts.voter.key();
        voter_record.poll = poll.key();
        
        emit!(VoteCast {
            poll: poll.key(),
            voter: ctx.accounts.voter.key(),
            option_index,
            timestamp: clock.unix_timestamp,
        });
        
        Ok(())
    }

}

#[derive(Accounts)]
#[instruction(poll_id: u64, question: String, options: Vec<String>)]
pub struct CreatePoll<'info> {
    #[account(
        init,
        payer = creator,
        space = Poll::LEN,
        seeds = [b"poll", creator.key().as_ref(), &poll_id.to_le_bytes()],
        bump
    )]
    pub poll: Account<'info, Poll>,
    #[account(mut)]
    pub creator: Signer<'info>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct CastVote<'info> {
    #[account(mut)]
    pub poll: Account<'info, Poll>,
    #[account(
        init,
        payer = voter,
        space = VoterRecord::LEN,
        seeds = [b"voter", poll.key().as_ref(), voter.key().as_ref()],
        bump
    )]
    pub voter_record: Account<'info, VoterRecord>,
    #[account(mut)]
    pub voter: Signer<'info>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct ClosePoll<'info> {
    #[account(mut)]
    pub poll: Account<'info, Poll>,
    pub creator: Signer<'info>,
}

#[account]
pub struct Poll {
    pub poll_id: u64,                                    // 8 bytes
    pub creator: Pubkey,                                 // 32 bytes
    pub question: [u8; MAX_QUESTION_LENGTH],             // 200 bytes
    pub question_length: u16,                            // 2 bytes
    pub options: [[u8; MAX_OPTION_LENGTH]; MAX_OPTIONS], // 200 bytes (50 * 4)
    pub option_lengths: [u8; MAX_OPTIONS],               // 4 bytes
    pub votes: [u64; MAX_OPTIONS],                       // 32 bytes (8 * 4)
    pub option_count: u8,                                // 1 byte
    pub created_at: i64,                                 // 8 bytes
}
impl Poll {
    pub const LEN: usize = 8 + // discriminator
    8 + // poll_id
    32 + // creator
    MAX_QUESTION_LENGTH + // question
    2 + // question_length
    (MAX_OPTION_LENGTH * MAX_OPTIONS) + // options
    MAX_OPTIONS + // option_lengths
    (8 * MAX_OPTIONS) + // votes
    1 + // option_count
    8; // created_at

    pub fn get_question(&self) -> String {
        String::from_utf8_lossy(&self.question[..self.question_length as usize]).to_string()
    }

    pub fn get_option(&self, index: usize) -> Option<String> {
        if index >= self.option_count as usize {
            return None;
        }
        Some(String::from_utf8_lossy(&self.options[index][..self.option_lengths[index] as usize]).to_string())
    }

    pub fn get_all_options(&self) -> Vec<String> {
        let mut options = Vec::new();
        for i in 0..self.option_count as usize {
            if let Some(option) = self.get_option(i) {
                options.push(option);
            }
        }
        options
    }
}

#[account]
pub struct VoterRecord {
    pub has_voted: bool,    // 1 byte
    pub voted_option: u8,   // 1 byte
    pub voted_at: i64,      // 8 bytes
    pub voter: Pubkey,      // 32 bytes
    pub poll: Pubkey,       // 32 bytes
}

impl VoterRecord {
    pub const LEN: usize = 8 + // discriminator
        1 + // has_voted
        1 + // voted_option
        8 + // voted_at
        32 + // voter
        32; // poll
}

#[event]
pub struct VoteCast {
    pub poll: Pubkey,
    pub voter: Pubkey,
    pub option_index: u8,
    pub timestamp: i64,
}

#[error_code]
pub enum VotingError {
    #[msg("Insufficient options provided. At least 2 options required.")]
    InsufficientOptions,
    #[msg("Too many options provided. Maximum 10 options allowed.")]
    TooManyOptions,
    #[msg("Question is too long. Maximum 200 characters allowed.")]
    QuestionTooLong,
    #[msg("Option is too long. Maximum 50 characters allowed.")]
    OptionTooLong,
    #[msg("Invalid option index.")]
    InvalidOption,
    #[msg("User has already voted for this poll.")]
    AlreadyVoted,
    #[msg("Unauthorized action.")]
    Unauthorized,
}