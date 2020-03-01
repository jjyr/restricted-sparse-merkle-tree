#![cfg_attr(not(feature = "std"), no_std)]

#[cfg(feature = "blake2b")]
pub mod blake2b;
pub mod default_store;
pub mod error;
pub mod h256;
#[cfg(test)]
mod tests;
pub mod traits;
pub mod tree;

pub use h256::H256;

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
