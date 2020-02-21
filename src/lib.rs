#![cfg_attr(not(feature = "std"), no_std)]

#[cfg(feature = "blake2b")]
pub mod blake2b;
pub mod error;
pub mod h256;
pub mod hasher;
mod sparse_index;
pub mod store;
#[cfg(test)]
mod tests;
pub mod tree;

pub use h256::H256;

cfg_if::cfg_if! {
    if #[cfg(feature = "std")] {
        use std::collections;
        use std::vec;
    } else {
        extern crate alloc;
        use alloc::collections;
        use alloc::vec;
    }
}
