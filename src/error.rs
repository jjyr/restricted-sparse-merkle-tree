use crate::{string, H256};

pub type Result<T> = ::core::result::Result<T, Error>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    MissingKey(usize, H256),
    CorruptedProof,
    EmptyProof,
    EmptyKeys,
    IncorrectNumberOfLeaves { expected: usize, actual: usize },
    Store(string::String),
}

impl core::fmt::Display for Error {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
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
            Error::EmptyKeys => {
                write!(f, "Empty keys")?;
            }
            Error::IncorrectNumberOfLeaves { expected, actual } => {
                write!(
                    f,
                    "Incorrect number of leaves, expected {} actual {}",
                    expected, actual
                )?;
            }
            Error::Store(err_msg) => {
                write!(f, "Backend store error: {}", err_msg)?;
            }
        }
        Ok(())
    }
}

#[cfg(feature = "std")]
impl std::error::Error for Error {}
