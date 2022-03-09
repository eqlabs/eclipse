use borsh::{BorshDeserialize, BorshSerialize};
use solana_program::pubkey::Pubkey;

#[derive(BorshSerialize, BorshDeserialize, PartialEq, Debug)]
pub struct AleoVerified {
    // If the tx_id is not the expected length,
    // it will not be verified therefore not stored
    pub tx_id: [u8; 32],
    pub bump: u8,
    // The public key that submitted the tx for verification
    pub authority: Pubkey,
}
