use crate::{
    error::{Error, Result},
    merge::{hash_leaf, merge},
    merkle_proof::MerkleProof,
    traits::{Hasher, Store, Value},
    vec::Vec,
    H256,
};
use core::marker::PhantomData;

/// The branch key
#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub struct BranchKey {
    height: u8,
    node_key: H256,
}

impl BranchKey {
    pub fn new(height: u8, node_key: H256) -> BranchKey {
        BranchKey { height, node_key }
    }
}

/// A branch in the SMT
#[derive(Debug, Eq, PartialEq, Clone)]
pub struct BranchNode {
    left: H256,
    right: H256,
}

/// A leaf in the SMT
#[derive(Debug, Eq, PartialEq, Clone)]
pub struct LeafNode<V> {
    pub key: H256,
    pub value: V,
}

/// Sparse merkle tree
#[derive(Default, Debug)]
pub struct SparseMerkleTree<H, V, S> {
    store: S,
    root: H256,
    phantom: PhantomData<(H, V)>,
}

impl<H: Hasher + Default, V: Value, S: Store<V>> SparseMerkleTree<H, V, S> {
    /// Build a merkle tree from root and store
    pub fn new(root: H256, store: S) -> SparseMerkleTree<H, V, S> {
        SparseMerkleTree {
            root,
            store,
            phantom: PhantomData,
        }
    }

    /// Merkle root
    pub fn root(&self) -> &H256 {
        &self.root
    }

    /// Check empty of the tree
    pub fn is_empty(&self) -> bool {
        self.root.is_zero()
    }

    /// Destroy current tree and retake store
    pub fn take_store(self) -> S {
        self.store
    }

    /// Get backend store
    pub fn store(&self) -> &S {
        &self.store
    }

    /// Get mutable backend store
    pub fn store_mut(&mut self) -> &mut S {
        &mut self.store
    }

    /// Update a leaf, return new merkle root
    /// set to zero value to delete a key
    pub fn update(&mut self, key: H256, value: V) -> Result<&H256> {
        // compute and store new leaf
        let node = hash_leaf::<H>(&key, &value.to_h256());
        // notice when value is zero the leaf is deleted, so we do not need to store it
        if !node.is_zero() {
            self.store.insert_leaf(key, LeafNode { key, value })?;
        } else {
            self.store.remove_leaf(&key)?;
        }

        // recompute the tree from bottom to top
        let mut current_key = key;
        let mut current_node = node;
        for height in 0..=core::u8::MAX {
            let parent_key = current_key.parent_path(height);
            let parent_branch_key = BranchKey::new(height, parent_key);
            let (left, right) =
                if let Some(parent_branch) = self.store.get_branch(&parent_branch_key)? {
                    if current_key.is_right(height) {
                        (parent_branch.left, current_node)
                    } else {
                        (current_node, parent_branch.right)
                    }
                } else if current_key.is_right(height) {
                    (H256::zero(), current_node)
                } else {
                    (current_node, H256::zero())
                };

            if !left.is_zero() || !right.is_zero() {
                // insert or update branch
                self.store
                    .insert_branch(parent_branch_key, BranchNode { left, right })?;
            } else {
                // remove empty branch
                self.store.remove_branch(&parent_branch_key)?;
            }
            // prepare for next round
            current_key = parent_key;
            current_node = merge::<H>(height, &parent_key, &left, &right);
        }

        self.root = current_node;
        Ok(&self.root)
    }

    /// Get value of a leaf
    /// return zero value if leaf not exists
    pub fn get(&self, key: &H256) -> Result<V> {
        if self.is_empty() {
            return Ok(V::zero());
        }
        Ok(self
            .store
            .get_leaf(key)?
            .map(|node| node.value)
            .unwrap_or_else(V::zero))
    }

    /// Generate merkle proof
    pub fn merkle_proof(&self, mut keys: Vec<H256>) -> Result<MerkleProof> {
        if keys.is_empty() {
            return Err(Error::EmptyKeys);
        }

        // sort keys
        keys.sort_unstable();

        // Collect leaf paths
        let mut leaves_path: Vec<H256> = Default::default();
        for current_key in &keys {
            let mut path = H256::zero();
            for height in 0..=core::u8::MAX {
                let parent_key = current_key.parent_path(height);
                let parent_branch_key = BranchKey::new(height, parent_key);
                if let Some(parent_branch) = self.store.get_branch(&parent_branch_key)? {
                    let sibling = if current_key.is_right(height) {
                        parent_branch.left
                    } else {
                        parent_branch.right
                    };
                    if !sibling.is_zero() {
                        path.set_bit(height);
                    }
                } else {
                    // The key is not in the tree (support non-inclusion proof)
                }
            }
            leaves_path.push(path);
        }

        // Collect sibling node hashes
        let mut proof: Vec<H256> = Default::default();
        let mut current_keys: Vec<H256> = keys;
        let mut next_keys: Vec<H256> = Default::default();
        for height in 0..=core::u8::MAX {
            let mut key_idx = 0;
            while key_idx < current_keys.len() {
                let key_a = current_keys[key_idx];
                let parent_key_a = key_a.parent_path(height);

                let mut non_sibling_keys = Vec::with_capacity(2);
                if key_idx + 1 < current_keys.len() {
                    // There are more than 2 keys left
                    let key_b = current_keys[key_idx + 1];
                    let parent_key_b = key_b.parent_path(height);
                    if parent_key_a == parent_key_b {
                        next_keys.push(key_a);
                        key_idx += 2;
                    } else {
                        non_sibling_keys.push((key_a, parent_key_a));
                        if key_idx + 2 == current_keys.len() {
                            non_sibling_keys.push((key_b, parent_key_b));
                        }
                    }
                } else {
                    non_sibling_keys.push((key_a, parent_key_a));
                }

                for (current_key, parent_key) in non_sibling_keys.into_iter() {
                    let parent_branch_key = BranchKey::new(height, parent_key);
                    if let Some(parent_branch) = self.store.get_branch(&parent_branch_key)? {
                        let sibling = if current_key.is_right(height) {
                            parent_branch.left
                        } else {
                            parent_branch.right
                        };
                        if !sibling.is_zero() {
                            proof.push(sibling);
                        }
                    } else {
                        // The key is not in the tree (support non-inclusion proof)
                    }
                    next_keys.push(current_key);
                    key_idx += 1;
                }
            }
            current_keys = core::mem::take(&mut next_keys);
        }
        Ok(MerkleProof::new(leaves_path, proof))
    }
}
