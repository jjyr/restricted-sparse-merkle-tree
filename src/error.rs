use crate::H256;
use failure::Fail;

#[derive(Debug, Fail, Clone, PartialEq)]
pub enum Error {
    #[fail(
        display = "Missing key, store maybe corrupt: height {} key {:?}",
        _0, _1
    )]
    MissingKey(usize, H256),
    #[fail(display = "Corrupted proof")]
    CorruptedProof,
    #[fail(display = "Empty proof")]
    EmptyProof,
}

pub type Result<T> = ::std::result::Result<T, Error>;
