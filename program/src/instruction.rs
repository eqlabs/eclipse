use borsh::{BorshDeserialize, BorshSerialize};
use solana_program::pubkey::Pubkey;

#[derive(BorshSerialize, BorshDeserialize, PartialEq, Debug)]
pub enum EclipseInstruction {
    /// Accounts expected:
    ///
    /// 0. `[signer]`: The account of the account initialise the verification
    /// 1. `[writable]`: Aleo transactions verification results storage account
    /// 2. `[]`: Aleo Program account
    /// 3. `[]`: System Program account
    VerifyAleoTransaction {
        tx_id: Vec<u8>,
        aleo_verifier_id: Pubkey,
    },
}
