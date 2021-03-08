use crate::{string, H256};

pub type Result<T> = ::core::result::Result<T, Error>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    MissingBranch(H256),
    MissingLeaf(H256),
    CorruptedProof,
    EmptyProof,
    EmptyKeys,
    IncorrectNumberOfLeaves { expected: usize, actual: usize },
    Store(string::String),
    CorruptedStack,
    NonSiblings,
    InvalidCode(u8),
    NonMergableRange,
}

impl core::fmt::Display for Error {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Error::MissingBranch(node) => {
                write!(f, "Corrupted store, missing branch {:?}", node)?;
            }
            Error::MissingLeaf(node) => {
                write!(f, "Corrupted store, missing leaf {:?}", node)?;
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
            Error::CorruptedStack => {
                write!(f, "Corrupted compiled proof stack")?;
            }
            Error::NonSiblings => {
                write!(f, "Merging non-siblings in compiled stack")?;
            }
            Error::InvalidCode(code) => {
                write!(f, "Invalid compiled proof code: {}", code)?;
            }
            Error::NonMergableRange => {
                write!(f, "Ranges can not be merged")?;
            }
        }
        Ok(())
    }
}

#[cfg(feature = "std")]
impl std::error::Error for Error {}
