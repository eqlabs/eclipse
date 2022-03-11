use {
    serde::{Deserialize, Serialize},
    solana_program::{clock::Slot, pubkey::Pubkey},
};

#[derive(Debug, Serialize, Deserialize, PartialEq, Clone)]
pub struct DataBucket {
    pub meta: DataBucketMeta,
    pub data: Vec<u8>,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Clone)]
pub struct DataBucketMeta {
    /// The slot that the bucket was last updated. To ensure consistent data
    /// one should only consume bucket if last_updated_slot is older than
    /// the current bank's slot.
    pub last_updated_slot: Slot,

    /// Authority address which must sign for each modification.
    pub authority: Option<Pubkey>,
}
