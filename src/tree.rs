use crate::{
    collections::{BTreeMap, VecDeque},
    error::{Error, Result},
    merge::{hash_leaf, merge},
    merkle_proof::MerkleProof,
    traits::{Hasher, Store, Value},
    vec::Vec,
    EXPECTED_PATH_SIZE, H256, TREE_HEIGHT,
};
use core::{cmp::max, marker::PhantomData};

/// A branch in the SMT
#[derive(Debug, Eq, PartialEq, Clone)]
pub struct BranchNode {
    pub fork_height: u8,
    pub key: H256,
    pub node: H256,
    pub sibling: H256,
}

impl BranchNode {
    fn branch(&self, height: u8) -> (&H256, &H256) {
        let is_right = self.key.get_bit(height);
        if is_right {
            (&self.sibling, &self.node)
        } else {
            (&self.node, &self.sibling)
        }
    }
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

    /// Get backend store
    pub fn store(&self) -> &S {
        &self.store
    }

    /// Clear all items from a tree
    pub fn clear(&mut self) -> Result<()> {
        // Collect all keys first before deleting them to prevent and potential
        // disorder in store.
        let keys: Vec<H256> = self.store.leaf_iter()?.map(|node| node.key).collect();
        for key in keys {
            self.update(key, V::zero())?;
        }
        Ok(())
    }

    /// Update a leaf, return new merkle root
    /// set to zero value to delete a key
    pub fn update(&mut self, key: H256, value: V) -> Result<&H256> {
        // store the path, sparse index will ignore zero members
        let mut path: BTreeMap<_, _> = Default::default();
        // walk path from top to bottom
        let mut node = self.root;
        let mut branch = self.store.get_branch(&node)?;
        let mut height = branch
            .as_ref()
            .map(|b| max(b.key.fork_height(&key), b.fork_height))
            .unwrap_or(0);
        // branch.is_none() represents the descendants are zeros, so we can stop the loop
        while branch.is_some() {
            let branch_node = branch.unwrap();
            let fork_height = max(key.fork_height(&branch_node.key), branch_node.fork_height);
            if height > branch_node.fork_height {
                // branch node is a sibling
                path.insert(fork_height, node);
                break;
            }
            // branch node is parent if height is less than branch_node's height
            // remove it from store
            self.store.remove_branch(&node)?;
            let (left, right) = branch_node.branch(height);
            let is_right = key.get_bit(height);
            let sibling;
            if is_right {
                if &node == right {
                    break;
                }
                sibling = *left;
                node = *right;
            } else {
                if &node == left {
                    break;
                }
                sibling = *right;
                node = *left;
            }
            path.insert(height, sibling);
            // get next branch and fork_height
            branch = self.store.get_branch(&node)?;
            if let Some(branch_node) = branch.as_ref() {
                height = max(key.fork_height(&branch_node.key), branch_node.fork_height);
            }
        }
        // delete previous leaf
        if let Some(leaf) = self.store.get_leaf(&node)? {
            if leaf.key == key {
                self.store.remove_leaf(&node)?;
            }
        }

        // compute and store new leaf
        let mut node = hash_leaf::<H>(&key, &value.to_h256());
        // notice when value is zero the leaf is deleted, so we do not need to store it
        if !node.is_zero() {
            self.store.insert_leaf(node, LeafNode { key, value })?;
        }
        // build at least one branch for leaf
        self.store.insert_branch(
            node,
            BranchNode {
                key,
                fork_height: 0,
                node,
                sibling: H256::zero(),
            },
        )?;

        // recompute the tree from bottom to top
        while !path.is_empty() {
            // pop from path
            let height = path.iter().next().map(|(height, _)| *height).unwrap();
            let sibling = path.remove(&height).unwrap();

            let is_right = key.get_bit(height as u8);
            let parent = if is_right {
                merge::<H>(&sibling, &node)
            } else {
                merge::<H>(&node, &sibling)
            };

            let branch_node = BranchNode {
                fork_height: height as u8,
                sibling,
                node,
                key,
            };
            self.store.insert_branch(parent, branch_node)?;
            node = parent;
        }
        self.root = node;
        Ok(&self.root)
    }

    /// Get value of a leaf
    /// return zero value if leaf not exists
    pub fn get(&self, key: &H256) -> Result<V> {
        let mut node = self.root;
        // children must equals to zero when parent equals to zero
        while !node.is_zero() {
            let branch_node = match self.store.get_branch(&node)? {
                Some(branch_node) => branch_node,
                None => {
                    break;
                }
            };
            let is_right = key.get_bit(branch_node.fork_height as u8);
            let (left, right) = branch_node.branch(branch_node.fork_height as u8);
            if is_right {
                node = *right;
            } else {
                node = *left;
            }
            if branch_node.fork_height == 0 {
                break;
            }
        }

        // return zero is leaf_key is zero
        if node.is_zero() {
            return Ok(V::zero());
        }
        // get leaf node
        match self.store.get_leaf(&node)? {
            Some(leaf) if &leaf.key == key => Ok(leaf.value),
            _ => Ok(V::zero()),
        }
    }

    /// fetch merkle path of key into cache
    /// cache: (height, key) -> node
    fn fetch_merkle_path(
        &self,
        key: &H256,
        cache: &mut BTreeMap<(usize, H256), H256>,
    ) -> Result<()> {
        let mut node = self.root;
        let mut height = self
            .store
            .get_branch(&node)?
            .map(|b| max(b.key.fork_height(&key), b.fork_height))
            .unwrap_or(0);
        while !node.is_zero() {
            // the descendants are zeros, so we can break the loop
            if node.is_zero() {
                break;
            }
            match self.store.get_branch(&node)? {
                Some(branch_node) => {
                    if height <= branch_node.fork_height {
                        // node is child
                    } else {
                        let fork_height =
                            max(key.fork_height(&branch_node.key), branch_node.fork_height);

                        let is_right = key.get_bit(fork_height as u8);
                        let mut sibling_key = key.parent_path(fork_height as u8);
                        if is_right {
                        } else {
                            // mark sibling's index, sibling on the right path.
                            sibling_key.set_bit(height as u8);
                        };
                        if !node.is_zero() {
                            cache
                                .entry((fork_height as usize, sibling_key))
                                .or_insert(node);
                        }
                        break;
                    }
                    let (left, right) = branch_node.branch(height);
                    let is_right = key.get_bit(height);
                    let sibling;
                    if is_right {
                        if &node == right {
                            break;
                        }
                        sibling = *left;
                        node = *right;
                    } else {
                        if &node == left {
                            break;
                        }
                        sibling = *right;
                        node = *left;
                    }
                    let mut sibling_key = key.parent_path(height as u8);
                    if is_right {
                    } else {
                        // mark sibling's index, sibling on the right path.
                        sibling_key.set_bit(height as u8);
                    };
                    cache.insert((height as usize, sibling_key), sibling);
                    if let Some(branch_node) = self.store.get_branch(&node)? {
                        let fork_height =
                            max(key.fork_height(&branch_node.key), branch_node.fork_height);
                        height = fork_height;
                    }
                }
                None => break,
            };
        }
        Ok(())
    }

    /// Generate merkle proof
    pub fn merkle_proof(&self, mut keys: Vec<H256>) -> Result<MerkleProof> {
        if keys.is_empty() {
            return Err(Error::EmptyKeys);
        }

        // sort keys
        keys.sort_unstable();

        // fetch all merkle path
        let mut cache: BTreeMap<(usize, H256), H256> = Default::default();
        for k in &keys {
            self.fetch_merkle_path(k, &mut cache)?;
        }

        // (node, height)
        let mut proof: Vec<(H256, u8)> = Vec::with_capacity(EXPECTED_PATH_SIZE * keys.len());
        // key_index -> merkle path height
        let mut leaves_path: Vec<Vec<u8>> = Vec::with_capacity(keys.len());
        leaves_path.resize_with(keys.len(), Default::default);

        let keys_len = keys.len();
        // build merkle proofs from bottom to up
        // (key, height, key_index)
        let mut queue: VecDeque<(H256, usize, usize)> = keys
            .into_iter()
            .enumerate()
            .map(|(i, k)| (k, 0, i))
            .collect();

        while let Some((key, height, leaf_index)) = queue.pop_front() {
            if queue.is_empty() && cache.is_empty() || height == TREE_HEIGHT {
                // tree only contains one leaf
                if leaves_path[leaf_index].is_empty() {
                    leaves_path[leaf_index].push(core::u8::MAX);
                }
                break;
            }
            // compute sibling key
            let mut sibling_key = key.parent_path(height as u8);

            let is_right = key.get_bit(height as u8);
            if is_right {
                // sibling on left
                sibling_key.clear_bit(height as u8);
            } else {
                // sibling on right
                sibling_key.set_bit(height as u8);
            }
            if Some((&sibling_key, &height))
                == queue
                    .front()
                    .map(|(sibling_key, height, _leaf_index)| (sibling_key, height))
            {
                // drop the sibling, mark sibling's merkle path
                let (_sibling_key, height, leaf_index) = queue.pop_front().unwrap();
                leaves_path[leaf_index].push(height as u8);
            } else {
                match cache.remove(&(height, sibling_key)) {
                    Some(sibling) => {
                        debug_assert!(height <= core::u8::MAX as usize);
                        // save first non-zero sibling's height for leaves
                        proof.push((sibling, height as u8));
                    }
                    None => {
                        // skip zero siblings
                        if !is_right {
                            sibling_key.clear_bit(height as u8);
                        }
                        let parent_key = sibling_key;
                        queue.push_back((parent_key, height + 1, leaf_index));
                        continue;
                    }
                }
            }
            // find new non-zero sibling, append to leaf's path
            leaves_path[leaf_index].push(height as u8);
            if height < TREE_HEIGHT {
                // get parent_key, which k.get_bit(height) is false
                let parent_key = if is_right { sibling_key } else { key };
                queue.push_back((parent_key, height + 1, leaf_index));
            }
        }
        debug_assert_eq!(leaves_path.len(), keys_len);
        Ok(MerkleProof::new(leaves_path, proof))
    }
}
