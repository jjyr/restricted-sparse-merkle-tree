use crate::H256;
use failure::Fail;

#[derive(Debug, Fail, Clone, PartialEq)]
pub enum Error {
    #[fail(display = "Missing key, backend cache maybe corrupt: {:?}", _0)]
    MissingKey(H256),
    #[fail(display = "Compress proof error, reason: {:?}", _0)]
    CompressProof(String),
    #[fail(display = "Decompress proof error, reason: {:?}", _0)]
    DecompressProof(String),
}

pub type Result<T> = ::std::result::Result<T, Error>;
