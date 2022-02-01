use borsh::{BorshDeserialize, BorshSerialize};

#[derive(BorshSerialize, BorshDeserialize, PartialEq, Debug)]
pub enum EclipseInstruction {
    /// Accounts expected:
    ///
    /// 0. `[signer]`: The account of the account initialise the verification
    /// 1. `[writable]`: Aleo transactions verification results storage account
    VerifyAleoTransaction { tx_id: Vec<u8> },
}
