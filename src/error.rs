use crate::H256;

pub type Result<T> = ::std::result::Result<T, Error>;

#[derive(Debug, Clone, PartialEq)]
pub enum Error {
    MissingKey(usize, H256),
    CorruptedProof,
    EmptyProof,
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::MissingKey(height, key) => {
                write!(f, "Missing key at height {}, key {:?}", height, key)?;
            }
            Error::CorruptedProof => {
                write!(f, "Corrupted proof")?;
            }
            Error::EmptyProof => {
                write!(f, "Empty proof")?;
            }
        }
        Ok(())
    }
}

impl std::error::Error for Error {}
