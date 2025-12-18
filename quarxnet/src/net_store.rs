use std::fs::{self, File};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

#[derive(Clone)]
pub struct NetStore {
    inner: Arc<Mutex<NetStoreInner>>,
}

struct NetStoreInner {
    root: PathBuf,
}

impl NetStore {
    pub fn new<P: AsRef<Path>>(root: P) -> io::Result<Self> {
        let root = root.as_ref().to_path_buf();
        fs::create_dir_all(&root)?;
        Ok(NetStore {
            inner: Arc::new(Mutex::new(NetStoreInner { root })),
        })
    }

    pub fn put_blocks_raw(&self, raw: &[u8]) -> io::Result<()> {
        let inner = self.inner.lock().unwrap();
        let path = inner.root.join("last.blocks");
        let mut f = File::create(path)?;
        f.write_all(raw)
    }

    pub fn get_blocks_raw(&self) -> io::Result<Vec<u8>> {
        let inner = self.inner.lock().unwrap();
        let path = inner.root.join("last.blocks");
        match File::open(path) {
            Ok(mut f) => {
                let mut buf = Vec::new();
                f.read_to_end(&mut buf)?;
                Ok(buf)
            }
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(Vec::new()),
            Err(e) => Err(e),
        }
    }

    pub fn put_object_raw(&self, raw: &[u8]) -> io::Result<()> {
        let inner = self.inner.lock().unwrap();
        let path = inner.root.join("last.object");
        let mut f = File::create(path)?;
        f.write_all(raw)
    }

    pub fn get_object_raw(&self) -> io::Result<Vec<u8>> {
        let inner = self.inner.lock().unwrap();
        let path = inner.root.join("last.object");
        match File::open(path) {
            Ok(mut f) => {
                let mut buf = Vec::new();
                f.read_to_end(&mut buf)?;
                Ok(buf)
            }
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(Vec::new()),
            Err(e) => Err(e),
        }
    }
}
