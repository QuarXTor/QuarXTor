use super::backend::RepoBackend;
use std::path::Path;
use super::error::RepoError;
use super::znode::ZNode;

/// High-level interface:
///   - Z-nodes
///   - object trees
///   - MultiRecipe/L0/L1/L2 (later)
///
/// Currently just pass-through to backend + ZNode helpers.
#[derive(Clone)]
pub struct QuarxRepo<B: RepoBackend> {
    pub(crate) backend: B,
}

impl<B: RepoBackend> QuarxRepo<B> {
    pub fn new(backend: B) -> Self {
        Self { backend }
    }

    /// Save object (no Z-node logic yet).
    pub fn put_object_raw(&self, raw: &[u8]) -> Result<String, RepoError> {
        self.backend.put_raw(raw)
    }

    /// Read object.
    pub fn get_object_raw(&self, id: &str) -> Result<Vec<u8>, RepoError> {
        self.backend.get_raw(id)
    }

    /// Save Z-node as object.
    pub fn put_znode(&self, zn: &ZNode) -> Result<String, RepoError> {
        let raw = zn.serialize();
        self.put_object_raw(&raw)
    }

    /// Read Z-node by id.
    pub fn get_znode(&self, id: &str) -> Result<ZNode, RepoError> {
        let raw = self.get_object_raw(id)?;
        super::znode::deserialize_znode(&raw)
    }

    /// Get list of child ZNode IDs.
    pub fn list_children(&self, id: &str) -> Result<Vec<String>, RepoError> {
        let zn = self.get_znode(id)?;
        Ok(zn.children.clone())
    }

    /// Recursive tree dump in ls -al style (simplified).
    ///
    /// Line format:
    ///   <indent><type><perms> <blocks> <size> <id>
    ///
    /// type  : 'd' for Directory, '-' for File
    /// perms : fixed rwx/r-- (stub)
    /// blocks: 1 + children.len() (stub for future quark blocks)
    pub fn dump_tree(&self, id: &str, depth: usize) -> Result<String, RepoError> {
        use crate::repo::znode::ZNodeKind;

        let zn = self.get_znode(id)?;
        let indent = "  ".repeat(depth);

        let (kind_ch, perms) = match zn.kind {
            ZNodeKind::Directory => ('d', "rwxr-xr-x"),
            ZNodeKind::File | ZNodeKind::L0File | ZNodeKind::Chunk => ('-', "rw-r--r--"),
        };

        // Stub for "quark blocks": later real number of L0/L1 blocks will go here.
        let blocks: u64 = 1 + zn.children.len() as u64;

        let line = format!(
            "{indent}{kind}{perms} {blocks:4} {size:8} {id}\n",
            indent = indent,
            kind = kind_ch,
            perms = perms,
            blocks = blocks,
            size = zn.size,
            id = id,
        );

        let mut out = line;

        for child in zn.children {
            out.push_str(&self.dump_tree(&child, depth + 1)?);
        }

        Ok(out)
    }

    pub fn backend_root_hint(&self) -> Option<&Path> {
        self.backend.repo_root_hint()
    }
}
