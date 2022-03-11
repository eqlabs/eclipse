use {
    crate::{
        instruction::{parse_program_instruction, ProgramInstruction},
        state,
    },
    solana_program::{
        account_info::{next_account_info, AccountInfo},
        clock::Clock,
        entrypoint,
        entrypoint::ProgramResult,
        msg,
        program::invoke_signed,
        program_error::ProgramError,
        pubkey::Pubkey,
        rent::Rent,
        system_instruction,
        sysvar::Sysvar,
    },
};

// Declare and export the program's entrypoint.
entrypoint!(process_instruction);

pub fn process_instruction(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    instruction_data: &[u8],
) -> ProgramResult {
    match parse_program_instruction(instruction_data)? {
        ProgramInstruction::CreateBucket {
            data,
            size,
            bump_seed,
        } => Processor::create_bucket(program_id, accounts, data, size as usize, bump_seed),
        ProgramInstruction::PutIntoBucket { data, offset } => {
            Processor::put_into_bucket(program_id, accounts, data, offset as usize)
        }
    }
}

fn adler32(data: &[u8]) -> u32 {
    let mut hash = adler32::RollingAdler32::new();
    hash.update_buffer(data);
    hash.hash()
}

pub struct Processor;
impl Processor {
    fn create_bucket(
        program_id: &Pubkey,
        accounts: &[AccountInfo],
        data: &[u8],
        size: usize,
        bump_seed: u8,
    ) -> ProgramResult {
        let account_info_iter = &mut accounts.iter();

        let authority_account = next_account_info(account_info_iter)?;
        let payer_account = next_account_info(account_info_iter)?;
        let data_bucket_account = next_account_info(account_info_iter)?;
        let system_program_account = next_account_info(account_info_iter)?;

        let authority_key = *authority_account.signer_key().ok_or_else(|| {
            msg!("Authority account must be a signer");
            ProgramError::MissingRequiredSignature
        })?;

        msg!("accounts parsed.");

        let payer_key = *payer_account.signer_key().ok_or_else(|| {
            msg!("Payer account must be a signer");
            ProgramError::MissingRequiredSignature
        })?;

        // Use a derived address to ensure that an address table can never be
        // initialized more than once at the same address.
        let adler_csum = adler32(data);
        let adler_csum = adler_csum.to_be_bytes();
        let adler_csum = adler_csum.as_slice();

        let derived_data_bucket_key = Pubkey::create_program_address(
            &[
                b"solana-data-packer".as_ref(),
                authority_key.as_ref(),
                adler_csum,
                &[bump_seed],
            ],
            program_id,
        )?;

        msg!("constructed program address");

        let data_bucket_key = *data_bucket_account.unsigned_key();
        if data_bucket_key != derived_data_bucket_key {
            msg!(
                "Data bucket address must match derived address: {}",
                derived_data_bucket_key
            );
            return Err(ProgramError::InvalidArgument);
        }

        let current_slot = Clock::get()?.slot;
        let data_bucket_len = 72 + size;
        let data_bucket = state::DataBucket {
            meta: state::DataBucketMeta {
                last_updated_slot: current_slot,
                authority: Some(authority_key),
            },
            data: data.to_vec(),
        };

        msg!("constructed data bucket object");

        let rent = Rent::default();
        let required_lamports = rent
            .minimum_balance(data_bucket_len)
            .max(1)
            .saturating_sub(data_bucket_account.lamports());

        msg!("creating the data bucket");

        invoke_signed(
            &system_instruction::create_account(
                &payer_key,
                &data_bucket_key,
                required_lamports,
                data_bucket_len as u64,
                program_id,
            ),
            &[
                data_bucket_account.clone(),
                authority_account.clone(),
                system_program_account.clone(),
            ],
            &[&[
                b"solana-data-packer",
                authority_key.as_ref(),
                adler_csum,
                &[bump_seed],
            ]],
        )?;

        msg!("storing data on the data bucket account");

        // Finally store the data in the bucket.
        data_bucket_account
            .serialize_data(&data_bucket)
            .map_err(|_| ProgramError::InvalidAccountData)
    }

    fn put_into_bucket(
        _program_id: &Pubkey,
        accounts: &[AccountInfo],
        data: &[u8],
        offset: usize,
    ) -> ProgramResult {
        let account_info_iter = &mut accounts.iter();

        let authority_account = next_account_info(account_info_iter)?;
        let _payer_account = next_account_info(account_info_iter)?;
        let data_bucket_account = next_account_info(account_info_iter)?;
        let _system_program_account = next_account_info(account_info_iter)?;

        let mut data_bucket: state::DataBucket = data_bucket_account
            .deserialize_data()
            .map_err(|_| ProgramError::InvalidAccountData)?;

        if data_bucket.meta.authority.unwrap() != *authority_account.signer_key().unwrap() {
            return Err(ProgramError::InvalidArgument);
        }

        // Ensure there's enough space for new data.
        if data_bucket.data.len() < (offset + data.len()) {
            data_bucket.data.resize(offset + data.len(), 0);
        }

        data_bucket.data[offset..].copy_from_slice(data.as_ref());
        data_bucket.meta.last_updated_slot = Clock::get()?.slot;

        data_bucket_account
            .serialize_data(&data_bucket)
            .map_err(|_| ProgramError::InvalidAccountData)?;

        Ok(())
    }
}
