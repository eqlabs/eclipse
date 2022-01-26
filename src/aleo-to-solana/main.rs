use {
    anyhow::Result,
    clap::{
        crate_description, crate_name, crate_version, value_t, App, AppSettings, Arg, SubCommand,
    },
    derivative::Derivative,
    jsonrpsee::{
        http_client::{HttpClient, HttpClientBuilder},
        rpc_params,
    },
    jsonrpsee_core::client::ClientT,
    serde::{Deserialize, Serialize},
    snarkvm::{
        dpc::testnet2::Testnet2,
        dpc::traits::Network,
        prelude::{Block, Transaction as SnarkVMTransaction},
    },
    snarkvm_algorithms::merkle_tree::*,
    snarkvm_utilities::ToBytes,
    solana_clap_utils::{
        fee_payer::fee_payer_arg,
        input_parsers::keypair_of,
        input_validators::{is_keypair, is_url},
        keypair::signer_from_path,
    },
    solana_client::rpc_client::RpcClient,
    solana_program::{
        instruction::{AccountMeta, Instruction},
        message::Message,
        pubkey::Pubkey,
    },
    solana_sdk::{
        signature::Signer, signer::keypair::Keypair, transaction::Transaction as SolanaTransaction,
    },
    std::{process::exit, str::FromStr, sync::Arc, thread::sleep, time::Duration},
};

mod aleo_proof;

type Error = Box<dyn std::error::Error>;
type CommandResult = Result<(), Error>;

struct Eclipse {
    solana_client: RpcClient,
    solana_keypair: Keypair,
    snarkos_client: HttpClient,
    fee_payer: Box<dyn Signer>,
}

#[derive(Derivative)]
#[derivative(Clone(bound = "N: Network"), Debug(bound = "N: Network"))]
pub struct PhantomTree<N: Network> {
    #[derivative(Debug = "ignore")]
    tree: Arc<MerkleTree<N::TransactionIDParameters>>,
}

impl<N: Network> PhantomTree<N> {
    /// Initializes an empty local transitions tree.
    pub fn new() -> Result<Self> {
        Ok(Self {
            tree: Arc::new(MerkleTree::<N::TransactionIDParameters>::new::<
                N::TransitionID,
            >(
                Arc::new(N::transaction_id_parameters().clone()), &vec![]
            )?),
        })
    }

    /// Returns the local transitions root.
    pub fn root(&self) -> N::TransactionID {
        (*self.tree.root()).into()
    }
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
            Arg::with_name("solana_keypair")
                .long("solana_keypair")
                .validator(is_keypair)
                .value_name("KEYPAIR")
                .takes_value(true)
                .required(true)
                .help("Solana signer keypair path"),
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
        .arg(fee_payer_arg().short("p").global(true))
        .subcommand(
            SubCommand::with_name("verify_proofs").about("Call native Aleo Proof Verifier program"),
        )
        .get_matches();
    let mut wallet_manager = None;
    let eclipse = {
        let cli_config = if let Some(config_file) = matches.value_of("config_file") {
            solana_cli_config::Config::load(config_file).unwrap_or_default()
        } else {
            solana_cli_config::Config::default()
        };
        let solana_json_rpc_url = value_t!(matches, "solana_json_rpc_url", String)
            .unwrap_or_else(|_| cli_config.json_rpc_url.clone());

        let fee_payer = signer_from_path(
            &matches,
            matches
                .value_of("fee_payer")
                .unwrap_or(&cli_config.keypair_path),
            "fee_payer",
            &mut wallet_manager,
        )
        .unwrap_or_else(|e| {
            eprintln!("error: {}", e);
            exit(1);
        });

        let solana_keypair =
            keypair_of(&matches, "solana_keypair").expect("invalid solana keypair");
        let snarkos_client = HttpClientBuilder::default()
            .build(matches.value_of("snarkos_json_rpc_url").unwrap())?;

        Eclipse {
            solana_client: RpcClient::new(solana_json_rpc_url),
            solana_keypair,
            snarkos_client,
            fee_payer,
        }
    };

    let _ = match matches.subcommand() {
        ("verify_proofs", _) => eclipse.verify_proofs(),
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
    async fn verify_proofs(&self) -> Result<()> {
        let mut cur_block: Block<Testnet2>;
        let mut prev_block: Option<Block<Testnet2>> = None;

        loop {
            println!("fetching latestblock");
            let response: serde_json::Value =
                self.snarkos_client.request("latestblock", None).await?;

            println!("parsing block");
            cur_block = serde_json::from_value(response)?;

            println!("checking if it's new");
            if let Some(ref pb) = prev_block {
                if pb.hash() == cur_block.hash() {
                    println!(
                        "sleeping: prev_block == cur_block ({} == {})",
                        pb.hash(),
                        cur_block.hash()
                    );
                    // Sleep
                    sleep(Duration::from_millis(5000));
                    continue;
                }
            }

            println!("processing block");
            self.process_block(&cur_block).await?;

            prev_block = Some(cur_block);
        }
    }

    async fn process_block(&self, block: &Block<Testnet2>) -> anyhow::Result<()> {
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
                            for t in tx.transaction.transitions() {
                                // Initialize a local transitions tree.
                                let tree = PhantomTree::<Testnet2>::new()
                                    .expect("simple constructor must always succeed");

                                let input_bytes = aleo_proof::Input::<Testnet2> {
                                    transition: t.clone(),
                                    inner_circuit_id: tx.transaction.inner_circuit_id(),
                                    ledger_root: tx.transaction.ledger_root(),
                                    local_transitions_root: tree.root(),
                                }
                                .to_bytes_le()?;

                                println!("length of input_bytes: {}", input_bytes.len());

                                self.command_verify_proof(input_bytes.as_ref()).await?;
                            }
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

    async fn command_verify_proof(&self, data: &[u8]) -> anyhow::Result<()> {
        let program_id = Pubkey::from_str("A1eoProof1111111111111111111111111111111111")
            .expect("failed to set program_id");

        let dest_pubkey =
            Pubkey::create_with_seed(&self.solana_keypair.pubkey(), "my fuzzy seed", &program_id)?;
        let instruction = Instruction::new_with_bytes(
            program_id,
            data,
            vec![
                AccountMeta::new(self.solana_keypair.pubkey(), true),
                AccountMeta::new(dest_pubkey, false),
            ],
        );

        let latest_blockhash = self
            .solana_client
            .get_latest_blockhash()
            .expect("failed to fetch latest blockhash");

        let message = Message::new(&[instruction], Some(&self.solana_keypair.pubkey()));
        let transaction =
            SolanaTransaction::new(&[&self.solana_keypair], message, latest_blockhash);

        self.send_transaction(transaction).await?;
        Ok(())
    }

    async fn send_transaction(
        &self,
        transaction: SolanaTransaction,
    ) -> solana_client::client_error::Result<()> {
        println!("Sending transaction...");
        let signature = self
            .solana_client
            .send_and_confirm_transaction_with_spinner(&transaction)?;
        println!("Signature: {}", signature);
        Ok(())
    }
}
