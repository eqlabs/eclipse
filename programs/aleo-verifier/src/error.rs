use thiserror::Error;

use solana_program::program_error::ProgramError;

#[derive(Error, Debug, Copy, Clone)]
pub enum EclipseError {
    /// Invalid instruction
    #[error("Invalid Instruction")]
    InvalidInstruction,
    #[error("NoRentExempt")]
    NotRentExempt,
    #[error("InvalidStateAccount")]
    InvalidStateAccount,
}

impl From<EclipseError> for ProgramError {
    fn from(e: EclipseError) -> Self {
        ProgramError::Custom(e as u32)
    }
}
