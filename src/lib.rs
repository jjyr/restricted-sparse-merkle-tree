mod blake2b;

use crate::blake2b::new_blake2b;
use lazy_static::lazy_static;
use std::collections::HashMap;

type H256 = [u8; 32];
type TreeCache = HashMap<H256, (H256, H256)>;
const ZERO_HASH: H256 = [0u8; 32];

lazy_static! {
    static ref DEFAULT_TREE: (H256, TreeCache) = compute_default_tree();
    static ref DEFAULT_TREE_ROOT: H256 = DEFAULT_TREE.0;
}

enum Branch {
    Left,
    Right,
}
struct PathIter {
    path: H256,
    bit_index: u8,
    byte_index: u8,
}

impl Iterator for PathIter {
    type Item = Branch;
    fn next(&mut self) -> Option<Self::Item> {
        None
    }
}

fn merge(lhs: &H256, rhs: &H256) -> H256 {
    let mut hash = [0u8; 32];
    let mut hasher = new_blake2b();
    hasher.update(lhs);
    hasher.update(rhs);
    hasher.finalize(&mut hash);
    hash
}

/// precompute default tree
fn compute_default_tree() -> (H256, TreeCache) {
    let mut hash = ZERO_HASH.clone();
    let mut cache: TreeCache = Default::default();
    for _ in 0..256 {
        let parent = merge(&hash, &hash);
        cache.insert(parent, (hash, hash));
        hash = parent;
    }
    (hash, cache)
}

pub struct SparseMerkleTree {
    pub cache: TreeCache,
    pub root: H256,
}

impl Default for SparseMerkleTree {
    fn default() -> Self {
        SparseMerkleTree::new(DEFAULT_TREE.0, DEFAULT_TREE.1.clone())
    }
}

impl SparseMerkleTree {
    pub fn new(root: H256, cache: TreeCache) -> SparseMerkleTree {
        SparseMerkleTree { root, cache }
    }

    /// update a key
    pub fn update(&mut self, key: H256, value: H256) {}
    /// get a value
    pub fn get(&mut self, key: H256) -> H256 {
        unimplemented!()
    }
    /// generate merkle proof
    pub fn gen_proof(&self, key: H256) {}
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn it_works() {
        let tree = SparseMerkleTree::default();
        assert_eq!(tree.cache.len(), 256);
        assert_eq!(
            tree.root,
            [
                196, 132, 51, 8, 180, 167, 239, 184, 118, 169, 184, 200, 14, 177, 93, 124, 168,
                217, 185, 198, 139, 96, 205, 180, 89, 151, 241, 223, 31, 135, 83, 182
            ]
        );
    }
}
