use std::io;

#[derive(Debug)]
pub enum RepoError {
    Io(io::Error),
    InvalidInput,
    NotFound,
    CodecError,
}

impl From<io::Error> for RepoError {
    fn from(e: io::Error) -> Self {
        RepoError::Io(e)
    }
}
