use {
    crate::{error::EclipseError, instruction::EclipseInstruction, state::AleoVerified},
    borsh::{BorshDeserialize, BorshSerialize},
    solana_program::{
        account_info::{next_account_info, AccountInfo},
        entrypoint::ProgramResult,
        msg,
        program::invoke_signed,
        pubkey::Pubkey,
        system_instruction,
        sysvar::{rent::Rent, Sysvar},
    },
};

pub struct Processor;

impl Processor {
    pub fn process(
        program_id: &Pubkey,
        accounts: &[AccountInfo],
        instruction_data: &[u8],
    ) -> ProgramResult {
        let instruction = EclipseInstruction::try_from_slice(instruction_data)?;
        match instruction {
            EclipseInstruction::VerifyAleoTransaction { tx_id } => {
                Self::process_aleo_tx_verification(accounts, program_id, tx_id)
            }
        }
    }
    pub fn process_aleo_tx_verification(
        accounts: &[AccountInfo],
        program_id: &Pubkey,
        tx_id: Vec<u8>,
    ) -> ProgramResult {
        let account_info_iter = &mut accounts.iter();
        let authority_account = next_account_info(account_info_iter)?;
        let state_account = next_account_info(account_info_iter)?;
        let system_program_account = next_account_info(account_info_iter)?;

        // Call AleoVerifier native program

        let (verified_pda, verified_acc_bump) = Pubkey::find_program_address(
            &[
                b"AleoTx".as_ref(),
                tx_id.as_ref(),
                authority_account.key.as_ref(),
            ],
            program_id,
        );
        if verified_pda != *state_account.key {
            return Err(EclipseError::InvalidStateAccount.into());
        }

        // Only successfully verified tx are stored
        // length is bump for the account + authority_account
        // TODO: tx_id byte length
        let stored_tx_len: usize = 1 + 32 + 32;

        let rent = Rent::get()?;
        let rent_lamports = rent.minimum_balance(stored_tx_len);

        let create_tx_pda_ix = &system_instruction::create_account(
            authority_account.key,
            state_account.key,
            rent_lamports,
            stored_tx_len.try_into().unwrap(),
            program_id,
        );

        msg!("Storing new verified Aleo tx");

        invoke_signed(
            create_tx_pda_ix,
            &[
                authority_account.clone(),
                state_account.clone(),
                system_program_account.clone(),
            ],
            &[&[
                b"AleoTx".as_ref(),
                tx_id.as_ref(),
                authority_account.key.as_ref(),
                &[verified_acc_bump],
            ]],
        )?;

        let mut state = <AleoVerified>::try_from_slice(&state_account.data.borrow())?;
        state.bump = verified_acc_bump;
        state.authority = *authority_account.key;
        state.tx_id = tx_id.to_vec();

        state.serialize(&mut &mut state_account.data.borrow_mut()[..])?;

        Ok(())
    }
}