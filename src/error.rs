use crate::H256;

pub type Result<T> = ::core::result::Result<T, Error>;

#[derive(Debug, Clone, PartialEq)]
pub enum Error {
    MissingKey(usize, H256),
    CorruptedProof,
    EmptyProof,
    EmptyKeys,
    EmptyTree,
    IncorrectNumberOfLeaves { expected: usize, actual: usize },
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
            Error::EmptyTree => {
                write!(f, "Empty tree")?;
            }
            Error::IncorrectNumberOfLeaves { expected, actual } => {
                write!(
                    f,
                    "Incorrect number of leaves, expected {} actual {}",
                    expected, actual
                )?;
            }
        }
        Ok(())
    }
}

#[cfg(feature = "std")]
impl std::error::Error for Error {}
