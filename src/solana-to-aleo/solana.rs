use {
    crate::eclipse,
    solana_client::{
        client_error::{ClientError, ClientErrorKind},
        rpc_client::RpcClient,
        rpc_config::RpcBlockConfig,
        rpc_custom_error::JSON_RPC_SERVER_ERROR_LONG_TERM_STORAGE_SLOT_SKIPPED,
        rpc_request::RpcError,
    },
    solana_sdk::{
        clock::Slot, commitment_config::CommitmentConfig, transaction::Transaction, vote,
    },
    solana_transaction_status::{EncodedTransaction, TransactionDetails, UiTransactionEncoding},
    std::{collections::BTreeMap, time::Duration, vec::Vec},
    ticker::Ticker,
};

enum VoteBasket {
    Votes(Vec<eclipse::Vote>),
    Full,
}

pub struct VoteCollector<T>
where
    T: eclipse::ProofGenerator,
{
    seal_threshold: usize,
    proof_generator: T,
    votes: BTreeMap<Slot, VoteBasket>,
}

impl<T: eclipse::ProofGenerator> VoteCollector<T> {
    pub fn new(proof_generation_threshold: usize, proof_generator: T) -> Self {
        VoteCollector {
            seal_threshold: proof_generation_threshold,
            proof_generator,
            votes: BTreeMap::new(),
        }
    }

    fn push_vote(&mut self, slot: Slot, vote: eclipse::Vote) {
        let votes_basket = self
            .votes
            .entry(slot)
            .or_insert_with(|| VoteBasket::Votes(Vec::new()));

        if let VoteBasket::Votes(ref mut votes) = votes_basket {
            votes.push(vote);
            if votes.len() >= self.seal_threshold
                && self
                    .proof_generator
                    .generate_proof(slot, votes.clone())
                    .is_ok()
            {
                // TODO(tuommaki): Once slot has been processed, it's marked as Full and
                // collected votes are dropped, but there's no GC to eventually clean up
                // entries from the tree.
                self.votes.insert(slot, VoteBasket::Full);
            }
        }
    }
}

pub struct BlockProcessor<T>
where
    T: eclipse::ProofGenerator,
{
    client: RpcClient,
    slot_votes: VoteCollector<T>,
}

impl<T: eclipse::ProofGenerator> BlockProcessor<T> {
    pub fn new(clnt: RpcClient, vote_store: VoteCollector<T>) -> Self {
        BlockProcessor {
            client: clnt,
            slot_votes: vote_store,
        }
    }

    pub fn poll_slot_votes(&mut self, start_slot: u64, poll_interval: Duration) {
        let mut current_slot = start_slot;

        for _ in Ticker::new(0.., poll_interval) {
            let current_network_slot = self.client.get_slot().unwrap();
            if current_slot == 0 {
                current_slot = current_network_slot;
            } else if current_slot > current_network_slot {
                println!("latest slot processed, skipping");
                continue;
            } else {
                current_slot += 1;
            }

            println!(
                "current network slot: {}, processing slot {}",
                current_network_slot, current_slot
            );

            let blk_cfg = RpcBlockConfig {
                encoding: Some(UiTransactionEncoding::Base58),
                transaction_details: Some(TransactionDetails::Full),
                rewards: None,
                commitment: Some(CommitmentConfig::confirmed()),
            };

            let txs: Vec<_> = match self.client.get_block_with_config(current_slot, blk_cfg) {
                Ok(blk) => match blk.transactions {
                    Some(transactions) => {
                        println!(
                            "found block {} from parent slot {} with {} transactions",
                            blk.block_height.unwrap(),
                            blk.parent_slot,
                            transactions.len()
                        );

                        transactions
                            .into_iter()
                            .filter_map(|e| match e.transaction {
                                EncodedTransaction::Binary(_, _) => e.transaction.decode(),
                                _ => None,
                            })
                            .collect()
                    }
                    None => continue,
                },
                Err(ClientError { kind, .. }) => {
                    match kind {
                        ClientErrorKind::RpcError(RpcError::RpcResponseError { code, .. }) => {
                            if code == JSON_RPC_SERVER_ERROR_LONG_TERM_STORAGE_SLOT_SKIPPED {
                                println!("slot {} skipped, proceeding to next", current_slot);
                            }
                        }
                        _ => {
                            println!("error: {}", kind);
                        }
                    };

                    continue;
                }
            };

            println!("{} transactions decoded", txs.len());
            self.process_slot_transactions(current_slot, &txs);
        }
    }

    fn process_slot_transactions(&mut self, slot: Slot, txs: &[Transaction]) {
        // Filter Vote Program transactions.
        let txs = txs.iter().filter(|tx| {
            (&tx.message.instructions).iter().any(|ci| {
                tx.message.account_keys[usize::from(ci.program_id_index)] == vote::program::id()
            })
        });

        txs.into_iter().for_each(|tx| {
            let msg = tx.message.serialize();

            tx.message.signer_keys().into_iter().for_each(|sk| {
                (&tx.signatures).iter().for_each(|sig| {
                    if sig.verify(sk.as_ref(), &msg) {
                        println!("successfully verified signature; adding to slot signatures");
                        self.slot_votes.push_vote(
                            slot,
                            eclipse::Vote::new(
                                sig.as_ref().to_vec(),
                                sk.to_bytes().to_vec(),
                                msg.clone(),
                            ),
                        );
                    }
                });
            });
        });
    }
}

/*
#[cfg(test)]
mod tests {
    use {
        crate::eclipse,
        crate::solana::VoteCollector,
        solana_sdk::{clock::Slot, signature::Signature},
        std::str::FromStr,
    };

    struct TestProofGenerator {
        proof: Option<(u64, Vec<eclipse::Vote>)>,
    }
    impl eclipse::ProofGenerator for TestProofGenerator {
        fn generate_proof(&self, slot: u64, votes: Vec<eclipse::Vote>) -> Result<(), String> {
            self.proof = Some((slot, votes));
            Ok(())
        }
    }

    #[test]
    fn vote_collector_sends_votes_at_threshold() {
        let proof_generator = TestProofGenerator { proof: None };
        let mut vote_collector = VoteCollector::new(3, proof_generator);

        let sig = Signature::from_str("5wVomaSMCDciSneCfZYmdoeEAoduEwWbj4wtE7q7qAuuCvFZD7DFzzjnL9UCXsuW9ZjsgYNoM2djSS8KCEgp5ATs").unwrap().as_ref().to_vec();
        let pub_key = bs58::decode("").into_vec().unwrap();
        let msg = bs58::decode("").into_vec().unwrap();
        let slot: Slot = 42;
        let v = eclipse::Vote::new(sig, pub_key, msg);

        // First
        vote_collector.push_vote(slot, v.clone());
        assert_eq!(proof_generator.proof, None);

        // Second
        vote_collector.push_vote(slot, v.clone());
        assert_eq!(proof_generator.proof, None);

        // Third
        vote_collector.push_vote(slot, v.clone());
        assert_ne!(proof_generator.proof, None);
    }
}
*/
