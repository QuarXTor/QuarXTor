use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use super::backend::RepoBackend;
use super::error::RepoError;
/// Very simple backend: map<String, Vec<u8>>.
/// Used as a "/dev/null" storage for tests.
#[derive(Clone)]
pub struct InMemBackend {
    inner: Arc<Mutex<HashMap<String, Vec<u8>>>>,
}

impl InMemBackend {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(Mutex::new(HashMap::new())),
        }
    }
}

impl Default for InMemBackend {
    fn default() -> Self {
        Self::new()
    }
}

impl RepoBackend for InMemBackend {
    fn put_raw(&self, raw: &[u8]) -> Result<String, RepoError> {
        let id = format!("obj-{}", uuid::Uuid::new_v4());
        self.inner.lock().unwrap().insert(id.clone(), raw.to_vec());
        Ok(id)
    }

    fn get_raw(&self, id: &str) -> Result<Vec<u8>, RepoError> {
        match self.inner.lock().unwrap().get(id) {
            Some(v) => Ok(v.clone()),
            None => Err(RepoError::NotFound),
        }
    }
}
