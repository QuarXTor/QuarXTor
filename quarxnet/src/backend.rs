use std::fs;
use std::io;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

const MAX_FRAME_SIZE: usize = 16 * 1024 * 1024;
const IO_CHUNK_SIZE: usize = 64 * 1024;

/// Minimal repository backend: put/get raw blob by id.
pub trait RepoBackend: Send + Sync {
    fn put_raw(&self, raw: &[u8]) -> io::Result<String>;
    fn get_raw(&self, id: &str) -> io::Result<Vec<u8>>;
    fn repo_root_hint(&self) -> Option<&Path> {
        None
    }
}

/// Simple file backend: root/obj-<uuid>.
#[derive(Clone)]
pub struct FileBackend {
    root: PathBuf,
}

impl FileBackend {
    pub fn new<P: AsRef<Path>>(root: P) -> io::Result<Self> {
        let root = root.as_ref().to_path_buf();
        fs::create_dir_all(&root)?;
        Ok(Self { root })
    }

    fn path_for(&self, id: &str) -> PathBuf {
        self.root.join(id)
    }
}

impl RepoBackend for FileBackend {
    fn put_raw(&self, raw: &[u8]) -> io::Result<String> {
        let id = format!("obj-{}", uuid::Uuid::new_v4());
        let path = self.path_for(&id);
        let mut f = fs::File::create(path)?;
        f.write_all(raw)?;
        Ok(id)
    }

    fn get_raw(&self, id: &str) -> io::Result<Vec<u8>> {
        let path = self.path_for(id);
        let mut f = fs::File::open(path)?;
        let mut buf = Vec::new();
        let mut chunk = vec![0u8; IO_CHUNK_SIZE];
        loop {
            let n = f.read(&mut chunk)?;
            if n == 0 {
                break;
            }
            if buf.len() + n > MAX_FRAME_SIZE {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("object exceeds max frame size {}", MAX_FRAME_SIZE),
                ));
            }
            buf.extend_from_slice(&chunk[..n]);
        }
        Ok(buf)
    }

    fn repo_root_hint(&self) -> Option<&Path> {
        Some(&self.root)
    }
}

/// Minimal Repo wrapper over a backend.
#[derive(Clone)]
pub struct Repo<B: RepoBackend> {
    backend: B,
}

impl<B: RepoBackend> Repo<B> {
    pub fn new(backend: B) -> Self {
        Self { backend }
    }

    pub fn put_object_raw(&self, raw: &[u8]) -> io::Result<String> {
        self.backend.put_raw(raw)
    }

    pub fn get_object_raw(&self, id: &str) -> io::Result<Vec<u8>> {
        self.backend.get_raw(id)
    }

    pub fn repo_root_hint(&self) -> Option<&Path> {
        self.backend.repo_root_hint()
    }
}

/// Backend abstraction for the network layer.
pub trait NetBackend: Send + Sync {
    fn put_blocks_raw(&self, raw: &[u8]) -> io::Result<()>;
    fn get_blocks_raw(&self) -> io::Result<Vec<u8>>;
    fn put_object_raw(&self, raw: &[u8]) -> io::Result<()>;
    fn get_object_raw(&self) -> io::Result<Vec<u8>>;
}

/// NetBackend adapter over Repo storing last ids.
#[derive(Clone)]
pub struct RepoNetBackend<B: RepoBackend + 'static> {
    repo: Repo<B>,
    last_blocks: Arc<Mutex<Option<String>>>,
    last_object: Arc<Mutex<Option<String>>>,
}

impl<B: RepoBackend + 'static> RepoNetBackend<B> {
    pub fn new(repo: Repo<B>) -> Self {
        Self {
            repo,
            last_blocks: Arc::new(Mutex::new(None)),
            last_object: Arc::new(Mutex::new(None)),
        }
    }
}

impl<B: RepoBackend + 'static> NetBackend for RepoNetBackend<B> {
    fn put_blocks_raw(&self, raw: &[u8]) -> io::Result<()> {
        let len = raw.len();
        let id = self.repo.put_object_raw(raw)?;
        {
            let mut guard = self.last_blocks.lock().unwrap();
            *guard = Some(id.clone());
        }
        eprintln!("[RepoNetBackend] put_blocks_raw: {} bytes -> id={}", len, id);
        Ok(())
    }

    fn get_blocks_raw(&self) -> io::Result<Vec<u8>> {
        let opt_id = { self.last_blocks.lock().unwrap().clone() };
        match opt_id {
            Some(id) => {
                let data = self.repo.get_object_raw(&id)?;
                eprintln!(
                    "[RepoNetBackend] get_blocks_raw: id={} -> {} bytes",
                    id,
                    data.len()
                );
                Ok(data)
            }
            None => {
                eprintln!("[RepoNetBackend] get_blocks_raw: no last_blocks id, returning empty");
                Ok(Vec::new())
            }
        }
    }

    fn put_object_raw(&self, raw: &[u8]) -> io::Result<()> {
        let len = raw.len();
        let id = self.repo.put_object_raw(raw)?;
        {
            let mut guard = self.last_object.lock().unwrap();
            *guard = Some(id.clone());
        }
        eprintln!("[RepoNetBackend] put_object_raw: {} bytes -> id={}", len, id);
        Ok(())
    }

    fn get_object_raw(&self) -> io::Result<Vec<u8>> {
        let opt_id = { self.last_object.lock().unwrap().clone() };
        match opt_id {
            Some(id) => {
                let data = self.repo.get_object_raw(&id)?;
                eprintln!(
                    "[RepoNetBackend] get_object_raw: id={} -> {} bytes",
                    id,
                    data.len()
                );
                Ok(data)
            }
            None => {
                eprintln!("[RepoNetBackend] get_object_raw: no last_object id, returning empty");
                Ok(Vec::new())
            }
        }
    }
}
