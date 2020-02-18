#![cfg_attr(not(feature = "std"), no_std)]

#[cfg(feature = "blake2b")]
pub mod blake2b;
pub mod error;
pub mod hasher;
pub mod sparse_index;
pub mod store;
#[cfg(test)]
mod tests;
pub mod tree;

pub type H256 = [u8; 32];

/// zero hash
pub const ZERO_HASH: H256 = [0u8; 32];
/// SMT tree height
pub const TREE_HEIGHT: usize = 256;
const BYTE_SIZE: usize = 8;

/// enable a bit on flag, i can be 0..256
fn set_bit(flag: &mut H256, i: usize) {
    let byte_pos = i / BYTE_SIZE;
    let bit_pos = i % BYTE_SIZE;
    flag[byte_pos] |= 1 << bit_pos as u8;
}

/// check a bit on flag, i can be 0..256
fn get_bit(flag: &H256, i: usize) -> bool {
    let byte_index = i / BYTE_SIZE;
    let bit_pos = i % BYTE_SIZE;
    let bit = flag[byte_index] >> bit_pos & 1;
    bit != 0
}

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
