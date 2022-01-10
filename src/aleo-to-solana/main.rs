use {
    anyhow::Result,
    core::arch::x86_64::_rdtsc,
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
        prelude::{Block, Transaction},
    },
    snarkvm_algorithms::merkle_tree::*,
    std::{sync::Arc, thread::sleep, time::Duration},
};

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
    // Assuming that the snarkOS is running locally, by default 3032 is the rpc server port
    let url = format!("http://{}", "127.0.0.1:3032");
    let client = HttpClientBuilder::default().build(url)?;
    let mut cur_block: Block<Testnet2>;
    let mut prev_block: Option<Block<Testnet2>> = None;

    loop {
        println!("fetching latestblock");
        let response: serde_json::Value = client.request("latestblock", None).await?;

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
        process_block(&cur_block, &client).await?;

        prev_block = Some(cur_block);
    }
}

async fn process_block(block: &Block<Testnet2>, client: &HttpClient) -> anyhow::Result<()> {
    for tx_id in block.transactions().transaction_ids() {
        let response: Result<serde_json::Value, _> = client
            .request("gettransaction", rpc_params!(tx_id.to_string()))
            .await;

        /// Additional metadata included with a transaction response
        #[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
        pub struct GetTransactionResponse {
            pub transaction: Transaction<Testnet2>,
            #[serde(skip)]
            pub metadata: String,
        }

        match response {
            Ok(response) => {
                let response: Result<GetTransactionResponse, _> = serde_json::from_value(response);

                match response {
                    Ok(tx) => {
                        let mut start: u64;
                        unsafe {
                            start = _rdtsc();
                        };

                        let mut is_valid = tx.transaction.is_valid();

                        let mut end: u64;
                        unsafe {
                            end = _rdtsc();
                        };

                        let mut instructions = end - start;

                        println!(
                            "Transaction validation - TSC: {}, is_valid: {}",
                            instructions, is_valid
                        );

                        for t in tx.transaction.transitions() {
                            // Initialize a local transitions tree.
                            let tree = PhantomTree::<Testnet2>::new()
                                .expect("simple constructor must always succeed");

                            unsafe {
                                start = _rdtsc();
                            };
                            is_valid = t.verify(
                                tx.transaction.inner_circuit_id(),
                                tx.transaction.ledger_root(),
                                tree.root(),
                            );
                            unsafe {
                                end = _rdtsc();
                            };
                            instructions = end - start;
                            println!(
                                "transition validation - TSC: {}, is_valid: {}",
                                instructions, is_valid
                            );
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
