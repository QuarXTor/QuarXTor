use crate::net_core::types::CapabilityKind;
use crate::types::{ClusterId, CodecId, DictId};

/// Capability advertisement of a node (supported codecs/dicts/clusters).
#[derive(Clone, Debug)]
pub enum Capability {
    /// List of supported CodecId.
    Codecs(Vec<CodecId>),

    /// List of supported DictId.
    Dicts(Vec<DictId>),

    /// List of clusters this node can serve.
    Clusters(Vec<ClusterId>),
}

impl Capability {
    pub fn kind(&self) -> CapabilityKind {
        match self {
            Capability::Codecs(_) => CapabilityKind::Codecs,
            Capability::Dicts(_) => CapabilityKind::Dicts,
            Capability::Clusters(_) => CapabilityKind::Clusters,
        }
    }
}
