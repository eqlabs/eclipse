use {
    solana_client::rpc_client::RpcClient,
    solana_sdk::{
        instruction::{AccountMeta, Instruction},
        message::Message,
        pubkey::Pubkey,
        signer::keypair::Keypair,
        signer::Signer,
        system_program,
        transaction::Transaction,
    },
};

const MAX_CHUNK_SIZE: usize = 768;

pub async fn upload(
    solana_client: &RpcClient,
    program_id: &Pubkey,
    author: &Keypair,
    payer: &Keypair,
    data: &[u8],
) -> anyhow::Result<()> {
    // Data Bucket Account
    let (data_bucket_account_pubkey, bump_seed) = Pubkey::find_program_address(
        &[b"solana-data-packer".as_ref(), author.pubkey().as_ref()],
        program_id,
    );

    let data = data.to_vec();
    let total_size = data.len();
    let (mut chunk, mut data) = data.split_at(768);

    println!("Saving data to account: {:?}", data_bucket_account_pubkey);

    // First create the data bucket.
    let serialized_bucket = eclipse_uploader::instruction::ProgramInstruction::CreateBucket {
            data: chunk,
            size: total_size as u32,
            bump_seed,
        }.serialize();

    let instruction = Instruction {
        program_id: *program_id,
        accounts: vec![
            AccountMeta::new(author.pubkey(), true),
            AccountMeta::new(payer.pubkey(), true),
            AccountMeta::new(data_bucket_account_pubkey, false),
            AccountMeta::new_readonly(system_program::id(), false),
        ],
        data: serialized_bucket,
    };

    let latest_blockhash = solana_client
        .get_latest_blockhash()
        .expect("failed to fetch latest blockhash");

    let message = Message::new(&[instruction], Some(&author.pubkey()));
    let transaction = Transaction::new(&[author, payer], message, latest_blockhash);

    send_transaction(solana_client, transaction).await?;

    let mut offset = chunk.len();

    // Then send rest of the data.
    while !data.is_empty() {
        if data.len() > MAX_CHUNK_SIZE {
            (chunk, data) = data.split_at(MAX_CHUNK_SIZE);
        } else {
            chunk = data;
            data = &[];
        }

        let serialized_bucket = eclipse_uploader::instruction::ProgramInstruction::PutIntoBucket{
            data: chunk,
            offset: offset as u32,
        }.serialize();

        let instruction = Instruction {
            program_id: *program_id,
            accounts: vec![
                AccountMeta::new(author.pubkey(), true),
                AccountMeta::new(payer.pubkey(), true),
                AccountMeta::new(data_bucket_account_pubkey, false),
                AccountMeta::new_readonly(system_program::id(), false),
            ],
            data: serialized_bucket,
        };

        let latest_blockhash = solana_client
            .get_latest_blockhash()
            .expect("failed to fetch latest blockhash");

        let message = Message::new(&[instruction], Some(&author.pubkey()));
        let transaction = Transaction::new(&[author, payer], message, latest_blockhash);

        send_transaction(solana_client, transaction).await?;
        offset += chunk.len();
    }

    println!(
        "Verification stored at Account: {:?}",
        data_bucket_account_pubkey
    );

    Ok(())
}

async fn send_transaction(
    solana_client: &RpcClient,
    transaction: Transaction,
) -> solana_client::client_error::Result<()> {
    println!("Sending transaction...");
    let result = solana_client.send_and_confirm_transaction_with_spinner(&transaction);
    println!("Solana onchain program result: {:?}", result);
    result.map(|_| ())
}
