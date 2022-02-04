use {
    crate::{error::EclipseError, instruction::EclipseInstruction, state::AleoVerified},
    borsh::{BorshDeserialize, BorshSerialize},
    solana_program::{
        account_info::{next_account_info, AccountInfo},
        entrypoint::ProgramResult,
        instruction::{AccountMeta, Instruction},
        msg,
        program::{invoke, invoke_signed},
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
            EclipseInstruction::VerifyAleoTransaction {
                tx_id,
                aleo_verifier_id,
            } => Self::process_aleo_tx_verification(accounts, program_id, tx_id, &aleo_verifier_id),
        }
    }
    pub fn process_aleo_tx_verification(
        accounts: &[AccountInfo],
        program_id: &Pubkey,
        tx_id: Vec<u8>,
        aleo_verifier_id: &Pubkey,
    ) -> ProgramResult {
        let account_info_iter = &mut accounts.iter();

        let authority_account = next_account_info(account_info_iter)?;
        let state_account = next_account_info(account_info_iter)?;
        let pda_account = next_account_info(account_info_iter)?;
        let aleo_account = next_account_info(account_info_iter)?;
        let _eclipse_program = next_account_info(account_info_iter)?;
        let aleo_program = next_account_info(account_info_iter)?;
        let system_program_account = next_account_info(account_info_iter)?;

        // Call AleoVerifier native program
        let instruction = Instruction::new_with_bytes(
            *aleo_verifier_id,
            tx_id.as_ref(),
            vec![
                AccountMeta::new(*pda_account.key, true),
                AccountMeta::new(*aleo_account.key, false),
                AccountMeta::new(*authority_account.key, false),
            ],
        );

        msg!("instruction update with auth: {:?}", instruction);

        let (_, bump_seed) = Pubkey::find_program_address(&[b"eclipse"], program_id);
        let r = invoke_signed(
            &instruction,
            &[
                authority_account.clone(),
                aleo_account.clone(),
                aleo_program.clone(),
                pda_account.clone(),
            ],
            &[&[&b"eclipse"[..], &[bump_seed]]],
        );

        msg!("result native invoke: {:?}", r);

        r?;

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
        // length is bump for the account + authority_account + tx_id
        // TODO: tx_id exact byte length 1511
        // This is for rent calculation only
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
            &[state_account.clone(), system_program_account.clone()],
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
