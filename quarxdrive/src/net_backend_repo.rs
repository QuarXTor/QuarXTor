use std::io;
use std::sync::{Arc, Mutex};

use crate::net_backend::NetBackend;
use crate::repo::{QuarxRepo, RepoBackend, RepoError};

/// NetBackend adapter on top of QuarxRepo.
///
/// Currently a thin layer:
///   - put_blocks_raw  -> repo.put_object_raw (and remember last_blocks_id)
///   - get_blocks_raw  -> repo.get_object_raw(last_blocks_id)
///   - put_object_raw  -> repo.put_object_raw (and remember last_object_id)
///   - get_object_raw  -> repo.get_object_raw(last_object_id)
///
/// Everything is in-process, without disk IO inside this adapter, but via a real Repo.
#[derive(Clone)]
pub struct RepoNetBackend<B: RepoBackend + 'static> {
    repo: QuarxRepo<B>,
    last_blocks: Arc<Mutex<Option<String>>>,
    last_object: Arc<Mutex<Option<String>>>,
}

impl<B: RepoBackend + 'static> RepoNetBackend<B> {
    pub fn new(repo: QuarxRepo<B>) -> Self {
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
        let id = self.repo.put_object_raw(raw).map_err(to_io)?;
        {
            let mut guard = self.last_blocks.lock().unwrap();
            *guard = Some(id.clone());
        }
        eprintln!(
            "[RepoNetBackend] put_blocks_raw: {} bytes -> id={}",
            len, id
        );
        Ok(())
    }

    fn get_blocks_raw(&self) -> io::Result<Vec<u8>> {
        let opt_id = { self.last_blocks.lock().unwrap().clone() };
        match opt_id {
            Some(id) => {
                let data = self.repo.get_object_raw(&id).map_err(to_io)?;
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
        let id = self.repo.put_object_raw(raw).map_err(to_io)?;
        {
            let mut guard = self.last_object.lock().unwrap();
            *guard = Some(id.clone());
        }
        eprintln!(
            "[RepoNetBackend] put_object_raw: {} bytes -> id={}",
            len, id
        );
        Ok(())
    }

    fn get_object_raw(&self) -> io::Result<Vec<u8>> {
        let opt_id = { self.last_object.lock().unwrap().clone() };
        match opt_id {
            Some(id) => {
                let data = self.repo.get_object_raw(&id).map_err(to_io)?;
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

fn to_io(e: RepoError) -> io::Error {
    io::Error::other(format!("{:?}", e))
}
