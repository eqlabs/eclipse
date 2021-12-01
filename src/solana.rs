use {
    solana_client::{
        client_error::{ClientError, ClientErrorKind},
        rpc_client::RpcClient,
        rpc_config::RpcBlockConfig,
        rpc_custom_error::JSON_RPC_SERVER_ERROR_LONG_TERM_STORAGE_SLOT_SKIPPED,
        rpc_request::RpcError,
    },
    solana_sdk::{
        clock::Slot, commitment_config::CommitmentConfig, signature::Signature,
        transaction::Transaction, vote,
    },
    solana_transaction_status::{EncodedTransaction, TransactionDetails, UiTransactionEncoding},
    std::{collections::BTreeMap, sync::mpsc, time::Duration, vec::Vec},
    ticker::Ticker,
};

#[derive(Debug, Clone, PartialEq)]
pub struct Vote {
    signature: Signature,
    pub_key: Vec<u8>,
    message: Vec<u8>,
}

impl Vote {
    pub fn new(sig: Signature, p_key: Vec<u8>, msg: Vec<u8>) -> Self {
        Vote {
            signature: sig,
            pub_key: p_key,
            message: msg,
        }
    }
}

enum VoteBasket {
    Votes(Vec<Vote>),
    Full,
}

pub struct VoteCollector {
    seal_threshold: usize,
    proof_generator: mpsc::Sender<(Slot, Vec<Vote>)>,
    votes: BTreeMap<Slot, VoteBasket>,
}

impl VoteCollector {
    pub fn new(
        proof_generation_threshold: usize,
        proof_generator: mpsc::Sender<(Slot, Vec<Vote>)>,
    ) -> Self {
        VoteCollector {
            seal_threshold: proof_generation_threshold,
            proof_generator,
            votes: BTreeMap::new(),
        }
    }

    fn push_vote(&mut self, slot: Slot, vote: Vote) {
        let votes_basket = self
            .votes
            .entry(slot)
            .or_insert(VoteBasket::Votes(Vec::new()));

        match votes_basket {
            VoteBasket::Votes(ref mut votes) => {
                votes.push(vote);
                if votes.len() >= self.seal_threshold {
                    if self.proof_generator.send((slot, votes.clone())).is_ok() {
                        // TODO(tuommaki): Once slot has been processed, it's marked as Full and
                        // collected votes are dropped, but there's no GC to eventually clean up
                        // entries from the tree.
                        self.votes.insert(slot, VoteBasket::Full);
                    }
                }
            }
            _ => (),
        }
    }
}

pub struct BlockProcessor {
    client: RpcClient,
    slot_votes: VoteCollector,
}

impl BlockProcessor {
    pub fn new(clnt: RpcClient, vote_store: VoteCollector) -> Self {
        BlockProcessor {
            client: clnt,
            slot_votes: vote_store,
        }
    }

    pub fn poll_slot_votes(&mut self, start_slot: u64, poll_interval: Duration) {
        let mut current_slot = start_slot;

        for _ in Ticker::new(0.., poll_interval) {
            let current_block_height = self.client.get_block_height().unwrap();
            if current_slot == 0 {
                current_slot = current_block_height;
            } else if current_slot > current_block_height {
                println!("latest block processed, skipping");
                continue;
            }

            println!(
                "current block height: {}, processing block {}",
                current_block_height, current_slot
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
                        println!("found block with {} transactions", transactions.len());

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
                                current_slot += 1;
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

            current_slot += 1;
        }
    }

    fn process_slot_transactions(&mut self, slot: Slot, txs: &Vec<Transaction>) {
        // Filter Vote Program transactions.
        let txs = txs.into_iter().filter_map(|tx| {
            if (&tx.message.instructions).into_iter().any(|ci| {
                tx.message.account_keys[usize::from(ci.program_id_index)] == vote::program::id()
            }) {
                Some(tx)
            } else {
                None
            }
        });

        txs.into_iter().for_each(|tx| {
            let msg = tx.message.serialize();

            tx.message.signer_keys().into_iter().for_each(|sk| {
                (&tx.signatures).into_iter().for_each(|sig| {
                    if sig.verify(sk.as_ref(), &msg) {
                        println!("successfully verified signature; adding to slot signatures");
                        self.slot_votes
                            .push_vote(slot, Vote::new(*sig, sk.to_bytes().to_vec(), msg.clone()));
                    }
                });
            });
        });
    }
}

#[cfg(test)]
mod tests {
    use {
        crate::solana::{Vote, VoteCollector},
        solana_sdk::{clock::Slot, signature::Signature},
        std::{str::FromStr, sync::mpsc::channel},
    };

    #[test]
    fn vote_collector_sends_votes_at_threshold() {
        let (tx, rx) = channel();
        let mut vote_collector = VoteCollector::new(3, tx);

        let sig = Signature::from_str("5wVomaSMCDciSneCfZYmdoeEAoduEwWbj4wtE7q7qAuuCvFZD7DFzzjnL9UCXsuW9ZjsgYNoM2djSS8KCEgp5ATs").unwrap();
        let pub_key = bs58::decode("").into_vec().unwrap();
        let msg = bs58::decode("").into_vec().unwrap();
        let slot: Slot = 42;
        let v = Vote::new(sig, pub_key, msg);

        // First
        vote_collector.push_vote(slot, v.clone());
        assert!(rx.try_recv().is_err());

        // Second
        vote_collector.push_vote(slot, v.clone());
        assert!(rx.try_recv().is_err());

        // Third
        vote_collector.push_vote(slot, v.clone());

        let received_vote = rx.try_recv();
        assert!(received_vote.is_ok());

        let (slot, votes) = received_vote.unwrap();
        assert_eq!(votes.len(), 3);

        // No more items in channel left
        assert!(rx.try_recv().is_err());

        // Fourth must not do anything as the used slot was already sent for proof generation.
        vote_collector.push_vote(slot, v.clone());
        assert!(rx.try_recv().is_err());
    }
}
