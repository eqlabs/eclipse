use {crate::eclipse, std::result::Result};

pub struct DummyProofGenerator {}

impl eclipse::ProofGenerator for DummyProofGenerator {
    fn generate_proof(&self, slot: u64, votes: Vec<eclipse::Vote>) -> Result<(), String> {
        println!(
            "proof generated for slot {} with {} votes",
            slot,
            votes.len()
        );
        Ok(())
    }
}
