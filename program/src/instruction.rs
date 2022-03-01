use borsh::{BorshDeserialize, BorshSerialize};

#[derive(BorshSerialize, BorshDeserialize, PartialEq, Debug)]
pub enum EclipseInstruction {
    /// Accounts expected:
    ///
    /// 0. `[signer]`: The account of the account initialise the verification
    /// 1. `[writable]`: Aleo transactions verification results storage account
    /// 2. `[]`: Eclipse Program PDA account
    /// 3. `[]`: Aleo Program account
    /// 4. `[]`: System Program account
    VerifyAleoTransaction { tx_id: Vec<u8> },
}

pub const ALEO_VERIFIER: &str = "A1eoProof1111111111111111111111111111111111";
