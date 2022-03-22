use {
    crate::{error::EclipseError, state::AleoVerified},
    borsh::{BorshDeserialize, BorshSerialize},
    solana_program::{
        account_info::{next_account_info, AccountInfo},
        entrypoint::ProgramResult,
        instruction::{AccountMeta, Instruction},
        msg,
        program::invoke_signed,
        pubkey::Pubkey,
        system_instruction,
        sysvar::{rent::Rent, Sysvar},
    },
    std::str::FromStr,
};

const ALEO_VERIFIER: &str = "A1eoProof1111111111111111111111111111111111";

pub struct Processor;

impl Processor {
    /// Accounts expected:
    ///
    /// 0. `[SIGNER]`: The account of the account initialise the verification
    /// 1. `[WRITE]`: Aleo transactions verification results storage account
    /// 2. `[]`: Aleo transaction data storage account
    /// 3. `[]`: Aleo Program account
    /// 4. `[]`: System Program account
    pub fn process(
        program_id: &Pubkey,
        accounts: &[AccountInfo],
        instruction_data: &[u8],
    ) -> ProgramResult {
        let account_info_iter = &mut accounts.iter();

        let authority_account = next_account_info(account_info_iter)?;
        let state_account = next_account_info(account_info_iter)?;
        let tx_data_account = next_account_info(account_info_iter)?;
        let aleo_program = next_account_info(account_info_iter)?;
        let system_program_account = next_account_info(account_info_iter)?;

        let (verified_pda, verified_acc_bump) = Pubkey::find_program_address(
            &[
                b"AleoTx".as_ref(),
                instruction_data,
                authority_account.key.as_ref(),
            ],
            program_id,
        );
        if verified_pda != *state_account.key {
            return Err(EclipseError::InvalidStateAccount.into());
        }

        // Call AleoVerifier native program to verify that Aleo tx proofs are valid.
        let aleo_verifier_id = Pubkey::from_str(ALEO_VERIFIER).expect("failed to set program_id");
        let instruction = Instruction::new_with_bytes(
            aleo_verifier_id,
            instruction_data,
            vec![AccountMeta::new_readonly(*tx_data_account.key, false)],
        );
        let (_, bump_seed) = Pubkey::find_program_address(&[b"eclipse"], program_id);
        invoke_signed(
            &instruction,
            &[aleo_program.clone(), tx_data_account.clone()],
            &[&[&b"eclipse"[..], &[bump_seed]]],
        )?;

        //
        // AleoVerifier native program successfully returned. Now create PDA to store verification
        // result.
        //
        // Only successfully verified tx are stored.
        // Length is bump for the account + authority_account + tx_id.
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

        invoke_signed(
            create_tx_pda_ix,
            &[
                authority_account.clone(),
                state_account.clone(),
                system_program_account.clone(),
            ],
            &[&[
                b"AleoTx".as_ref(),
                instruction_data,
                authority_account.key.as_ref(),
                &[verified_acc_bump],
            ]],
        )?;

        let mut state = AleoVerified::deserialize(&mut &state_account.data.borrow()[..])?;
        state.tx_id = instruction_data
            .try_into()
            .map_err(|_| EclipseError::InvalidStateAccount)
            .unwrap();
        state.bump = bump_seed;
        state.authority = *authority_account.key;
        state.serialize(&mut &mut state_account.data.borrow_mut()[..])?;

        msg!("New verified Aleo Tx Stored at {:?}", state_account.key);

        Ok(())
    }
}
