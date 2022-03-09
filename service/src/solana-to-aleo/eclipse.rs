use std::result::Result;

#[derive(Debug, Clone, PartialEq)]
pub struct Vote {
    signature: Vec<u8>,
    public_key: Vec<u8>,
    message: Vec<u8>,
}

impl Vote {
    pub fn new(signature: Vec<u8>, public_key: Vec<u8>, message: Vec<u8>) -> Self {
        Vote {
            signature,
            public_key,
            message,
        }
    }
}

pub trait ProofGenerator {
    fn generate_proof(&self, slot: u64, votes: Vec<Vote>) -> Result<(), String>;
}
