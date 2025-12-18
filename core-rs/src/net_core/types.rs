/// NodeId: abstract node identifier in the network.
pub type NodeId = u64;

/// Version tuple for the network protocol.
#[derive(Clone, Debug)]
pub struct ProtocolVersion {
    pub major: u16,
    pub minor: u16,
}

/// Capability kind identifier.
#[derive(Clone, Debug)]
pub enum CapabilityKind {
    Codecs,
    Dicts,
    Clusters,
}
