use serde::{Deserialize, Serialize};

#[derive(Debug, Clone)]
pub enum ProtoError {
    Decode,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ZNodeKind {
    Chunk,
    L0File,
    File,
    Directory,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ZNode {
    pub kind: ZNodeKind,
    pub size: u64,
    pub children: Vec<String>,
}

impl ZNode {
    #[inline]
    pub fn serialize(&self) -> Vec<u8> {
        bincode::serialize(self).expect("ZNode bincode::serialize should not fail")
    }

    #[inline]
    pub fn deserialize(raw: &[u8]) -> Result<Self, ProtoError> {
        bincode::deserialize(raw).map_err(|_| ProtoError::Decode)
    }

    pub fn pretty(&self) -> String {
        let kind = match self.kind {
            ZNodeKind::Chunk => "Chunk",
            ZNodeKind::L0File => "L0File",
            ZNodeKind::File => "File",
            ZNodeKind::Directory => "Directory",
        };
        format!(
            "ZNode {{ kind: {}, size: {}, children: {:?} }}",
            kind, self.size, self.children
        )
    }
}
