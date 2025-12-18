use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use super::backend::RepoBackend;
use super::error::RepoError;

/// Simple file backend:
///   root/obj-<uuid>
#[derive(Clone)]
pub struct FileBackend {
    root: PathBuf,
}

impl FileBackend {
    pub fn new<P: AsRef<Path>>(root: P) -> Result<Self, RepoError> {
        let root = root.as_ref().to_path_buf();
        fs::create_dir_all(&root).map_err(RepoError::Io)?;
        Ok(Self { root })
    }

    fn path_for(&self, id: &str) -> PathBuf {
        self.root.join(id)
    }
}

impl RepoBackend for FileBackend {
    fn put_raw(&self, raw: &[u8]) -> Result<String, RepoError> {
        let id = format!("obj-{}", uuid::Uuid::new_v4());
        let path = self.path_for(&id);
        let mut f = fs::File::create(path).map_err(RepoError::Io)?;
        f.write_all(raw).map_err(RepoError::Io)?;
        Ok(id)
    }

    fn get_raw(&self, id: &str) -> Result<Vec<u8>, RepoError> {
        let path = self.path_for(id);
        let mut f = fs::File::open(path).map_err(RepoError::Io)?;
        let mut buf = Vec::new();
        f.read_to_end(&mut buf).map_err(RepoError::Io)?;
        Ok(buf)
    }

    fn repo_root_hint(&self) -> Option<&Path> {
        Some(&self.root)
    }
}
