#![cfg_attr(not(feature = "std"), no_std)]

#[cfg(feature = "blake2b")]
pub mod blake2b;
pub mod default_store;
pub mod error;
pub mod h256;
pub mod merge;
pub mod merkle_proof;
#[cfg(test)]
mod tests;
pub mod traits;
pub mod tree;

pub use h256::H256;
pub use merkle_proof::{CompiledMerkleProof, MerkleProof};
pub use tree::SparseMerkleTree;

/// log2(256) * 2
pub const EXPECTED_PATH_SIZE: usize = 16;
pub const TREE_HEIGHT: usize = 256;

cfg_if::cfg_if! {
    if #[cfg(feature = "std")] {
        use std::collections;
        use std::vec;
        use std::string;
    } else {
        extern crate alloc;
        use alloc::collections;
        use alloc::vec;
        use alloc::string;
    }
}
