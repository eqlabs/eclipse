use {
    anyhow::Result,
    clap::{
        crate_description, crate_name, crate_version, value_t, App, AppSettings, Arg, SubCommand,
    },
    jsonrpsee::{
        http_client::{HttpClient, HttpClientBuilder},
        rpc_params,
    },
    jsonrpsee_core::client::ClientT,
    serde::{Deserialize, Serialize},
    snarkvm::dpc::testnet2::Testnet2,
    snarkvm::prelude::{Block, Transaction as SnarkVMTransaction},
    snarkvm::utilities::ToBytes,
    solana_clap_utils::{
        input_parsers::{keypair_of, value_of},
        input_validators::{is_keypair, is_url},
    },
    solana_client::rpc_client::RpcClient,
    solana_program::{
        instruction::{AccountMeta, Instruction},
        message::Message,
        pubkey::Pubkey,
        system_program,
    },
    solana_sdk::{
        signature::Signer, signer::keypair::Keypair, transaction::Transaction as SolanaTransaction,
    },
    std::{process::exit, str::FromStr, thread::sleep, time::Duration},
};

mod aleo_proof;
mod uploader;

struct Eclipse {
    solana_client: RpcClient,
    author_keypair: Keypair,
    payer_keypair: Keypair,
    snarkos_client: HttpClient,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    solana_logger::setup_with("solana=debug");

    let matches = App::new(crate_name!())
        .about(crate_description!())
        .version(crate_version!())
        .setting(AppSettings::SubcommandRequiredElseHelp)
        .arg({
            let arg = Arg::with_name("config_file")
                .short("C")
                .long("config")
                .value_name("PATH")
                .takes_value(true)
                .global(true)
                .help("Configuration file to use");
            if let Some(ref config_file) = *solana_cli_config::CONFIG_FILE {
                arg.default_value(config_file)
            } else {
                arg
            }
        })
        .arg(
            Arg::with_name("author_keypair")
                .long("author_keypair")
                .validator(is_keypair)
                .value_name("KEYPAIR")
                .takes_value(true)
                .required(true)
                .help("Solana signer keypair path"),
        )
        .arg(
            Arg::with_name("payer_keypair")
                .long("payer_keypair")
                .validator(is_keypair)
                .value_name("KEYPAIR")
                .takes_value(true)
                .required(true)
                .help("Solana payer keypair path"),
        )
        .arg(
            Arg::with_name("solana_json_rpc_url")
                .long("solana_url")
                .value_name("URL")
                .takes_value(true)
                .validator(is_url)
                .default_value("http://127.0.0.1:8899")
                .help("JSON RPC URL for the Solana cluster.  Default from the configuration file."),
        )
        .arg(
            Arg::with_name("snarkos_json_rpc_url")
                .long("snarkos_url")
                .value_name("URL")
                .takes_value(true)
                .validator(is_url)
                .default_value("http://127.0.0.1:3032")
                .help("JSON RPC URL for the Aleo cluster.  Default from the configuration file."),
        )
        .subcommand(
            SubCommand::with_name("verify_proofs")
                .about("Call Eclipse Onchain Program to verify Aleo Transaction Proof")
                .arg(
                    Arg::with_name("uploader_program_id")
                        .long("uploader_program_id")
                        .value_name("PUBKEY")
                        .takes_value(true)
                        .help("Eclipse on-chain uploader program id"),
                )
                .arg(
                    Arg::with_name("verifier_program_id")
                        .long("verifier_program_id")
                        .value_name("PUBKEY")
                        .takes_value(true)
                        .help("Eclipse on-chain Aleo verifier program id"),
                ),
        )
        .get_matches();
    let eclipse = {
        let cli_config = if let Some(config_file) = matches.value_of("config_file") {
            solana_cli_config::Config::load(config_file).unwrap_or_default()
        } else {
            solana_cli_config::Config::default()
        };
        let solana_json_rpc_url = value_t!(matches, "solana_json_rpc_url", String)
            .unwrap_or_else(|_| cli_config.json_rpc_url.clone());

        let author_keypair =
            keypair_of(&matches, "author_keypair").expect("invalid solana author keypair");
        let payer_keypair =
            keypair_of(&matches, "payer_keypair").expect("invalid solana payer keypair");
        let snarkos_client = HttpClientBuilder::default()
            .build(matches.value_of("snarkos_json_rpc_url").unwrap())?;

        Eclipse {
            solana_client: RpcClient::new(solana_json_rpc_url),
            author_keypair,
            payer_keypair,
            snarkos_client,
        }
    };

    let verifier_program_id;
    let uploader_program_id;
    let _ = match matches.subcommand() {
        ("verify_proofs", Some(args)) => {
            uploader_program_id = Pubkey::new(
                &bs58::decode(value_of::<String>(args, "uploader_program_id").unwrap())
                    .into_vec()
                    .unwrap(),
            );
            verifier_program_id = Pubkey::new(
                &bs58::decode(value_of::<String>(args, "verifier_program_id").unwrap())
                    .into_vec()
                    .unwrap(),
            );
            eclipse.verify_proofs(&uploader_program_id, &verifier_program_id)
        }
        _ => unreachable!(),
    }
    .await
    .map_err(|err| {
        eprintln!("{}", err);
        exit(1);
    });

    Ok(())
}

impl Eclipse {
    async fn verify_proofs(&self, uploader_program_id: &Pubkey, verifier_program_id: &Pubkey) -> Result<()> {
        let mut cur_block: Block<Testnet2>;
        let mut prev_block: Option<Block<Testnet2>> = None;

        loop {
            println!("Fetching latestblock from Aleo RPC-API");
            let response: serde_json::Value =
                self.snarkos_client.request("latestblock", None).await?;

            println!("Parsing block");
            cur_block = serde_json::from_value(response)?;

            println!("Verifying it is new block");
            if let Some(ref pb) = prev_block {
                if pb.hash() == cur_block.hash() {
                    println!(
                        "Sleeping: prev_block == cur_block ({} == {})",
                        pb.hash(),
                        cur_block.hash()
                    );
                    // Sleep
                    sleep(Duration::from_millis(5000));
                    continue;
                }
            }

            println!("Processing block");
            self.process_block(&cur_block, uploader_program_id, verifier_program_id).await?;

            prev_block = Some(cur_block);
        }
    }

    async fn process_block(
        &self,
        block: &Block<Testnet2>,
        uploader_program_id: &Pubkey,
        verifier_program_id: &Pubkey,
    ) -> anyhow::Result<()> {
        for tx_id in block.transactions().transaction_ids() {
            let response: Result<serde_json::Value, _> = self
                .snarkos_client
                .request("gettransaction", rpc_params!(tx_id.to_string()))
                .await;

            /// Additional metadata included with a transaction response
            #[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
            pub struct GetTransactionResponse {
                pub transaction: SnarkVMTransaction<Testnet2>,
                #[serde(skip)]
                pub metadata: String,
            }

            match response {
                Ok(response) => {
                    let response: Result<GetTransactionResponse, _> =
                        serde_json::from_value(response);

                    match response {
                        Ok(tx) => {
                            let tx_bytes = tx.transaction.to_bytes_le()?;


                            // Upload Aleo transaction to Solana Account
                            let tx_account = uploader::upload(&self.solana_client, uploader_program_id, &self.author_keypair, &self.payer_keypair, tx_bytes.as_ref()).await?;

                            let tx_id_bytes = tx.transaction.transaction_id().to_bytes_le()?;
                            self.command_verify_proof(tx_id_bytes.as_ref(), verifier_program_id, &tx_account)
                                .await?;
                        }
                        Err(err) => {
                            println!("error: failed to deserialize transaction: {}", err);
                        }
                    }
                }
                Err(err) => {
                    println!("failed to get transaction: {}", err);
                }
            }
        }

        Ok(())
    }

    async fn command_verify_proof(
        &self,
        tx_id: &[u8],
        eclipse_program_id: &Pubkey,
        tx_account: &Pubkey,
    ) -> anyhow::Result<()> {
        let aleo_program_id = Pubkey::from_str("A1eoProof1111111111111111111111111111111111")
            .expect("failed to set program_id");

        // Account to store sucesssful verification
        let (state_account_pubkey, _) = Pubkey::find_program_address(
            &[
                b"AleoTx".as_ref(),
                tx_id,
                self.author_keypair.pubkey().as_ref(),
            ],
            eclipse_program_id,
        );

        let instruction = Instruction {
            program_id: *eclipse_program_id,
            accounts: vec![
                AccountMeta::new(self.author_keypair.pubkey(), true),
                AccountMeta::new(state_account_pubkey, false),
                AccountMeta::new(*tx_account, false),
                AccountMeta::new_readonly(aleo_program_id, false),
                AccountMeta::new_readonly(system_program::id(), false),
            ],
            data: tx_id.to_vec(),
        };

        let latest_blockhash = self
            .solana_client
            .get_latest_blockhash()
            .expect("failed to fetch latest blockhash");

        let message = Message::new(&[instruction], Some(&self.author_keypair.pubkey()));
        let transaction =
            SolanaTransaction::new(&[&self.author_keypair], message, latest_blockhash);

        self.send_transaction(transaction).await?;
        println!("Verification stored at Account: {:?}", state_account_pubkey);
        Ok(())
    }

    async fn send_transaction(
        &self,
        transaction: SolanaTransaction,
    ) -> solana_client::client_error::Result<()> {
        self
            .solana_client
            .send_and_confirm_transaction_with_spinner(&transaction)?;
        Ok(())
    }
}
