use {
    snarkvm::{dpc::traits::Network, dpc::transition::Transition},
    snarkvm_utilities::{
        io::{Read, Result as IoResult, Write},
        FromBytes, ToBytes,
    },
};

pub(crate) struct Input<N: Network> {
    pub transition: Transition<N>,
    pub inner_circuit_id: N::InnerCircuitID,
    pub ledger_root: N::LedgerRoot,
    pub local_transitions_root: N::TransactionID,
}

impl<N: Network> FromBytes for Input<N> {
    #[inline]
    fn read_le<R: Read>(mut reader: R) -> IoResult<Self> {
        let transition: Transition<N> = FromBytes::read_le(&mut reader)?;
        let inner_circuit_id: N::InnerCircuitID = FromBytes::read_le(&mut reader)?;
        let ledger_root: N::LedgerRoot = FromBytes::read_le(&mut reader)?;
        let local_transitions_root: N::TransactionID = FromBytes::read_le(&mut reader)?;

        Ok(Self {
            transition,
            inner_circuit_id,
            ledger_root,
            local_transitions_root,
        })
    }
}

impl<N: Network> ToBytes for Input<N> {
    #[inline]
    fn write_le<W: Write>(&self, mut writer: W) -> IoResult<()> {
        self.transition.write_le(&mut writer)?;
        self.inner_circuit_id.write_le(&mut writer)?;
        self.ledger_root.write_le(&mut writer)?;
        self.local_transitions_root.write_le(&mut writer)
    }
}
