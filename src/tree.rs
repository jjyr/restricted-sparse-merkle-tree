use crate::{
    collections::{BTreeMap, VecDeque},
    error::{Error, Result},
    merge::{hash_leaf, merge},
    merkle_proof::MerkleProof,
    traits::{Hasher, Store, Value},
    vec::Vec,
    EXPECTED_PATH_SIZE, H256,
};
use core::{cmp::max, marker::PhantomData};

/// A branch in the SMT
#[derive(Debug, Eq, PartialEq, Clone)]
pub struct BranchNode {
    pub fork_height: u8,
    pub key: H256,
    pub node_type: NodeType,
}

impl BranchNode {
    fn is_pair(&self) -> bool {
        match self.node_type {
            NodeType::Pair(_, _) => true,
            _ => false,
        }
    }

    fn branch(&self, height: u8) -> (&H256, &H256) {
        match self.node_type {
            NodeType::Pair(ref node, ref sibling) => {
                let is_right = self.key.get_bit(height);
                if is_right {
                    (sibling, node)
                } else {
                    (node, sibling)
                }
            }
            _ => unreachable!("should be called on pair node only"),
        }
    }

    fn node(&self) -> &H256 {
        match self.node_type {
            NodeType::Single(ref node) => node,
            _ => unreachable!("should be called on single node only"),
        }
    }

    fn key(&self) -> &H256 {
        &self.key
    }
}

#[derive(Debug, Eq, PartialEq, Clone)]
pub enum NodeType {
    Single(H256),
    Pair(H256, H256),
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
        // store the path, sparse index will ignore zero members
        let mut path = Vec::new();
        if !self.is_empty() {
            let mut node = self.root;
            loop {
                let branch_node = self.store.get_branch(&node)?.expect("stored branch node");
                if branch_node.is_pair() {
                    let height = max(key.fork_height(branch_node.key()), branch_node.fork_height);
                    if height > branch_node.fork_height {
                        // the merge height is higher than node, so we do not need to remove node's branch
                        path.push((height, node));
                        break;
                    } else {
                        self.store.remove_branch(&node)?;
                        let (left, right) = branch_node.branch(height);
                        let is_right = key.get_bit(height);
                        if is_right {
                            node = *right;
                            path.push((height, *left));
                        } else {
                            node = *left;
                            path.push((height, *right));
                        }
                    }
                } else {
                    if &key == branch_node.key() {
                        self.store.remove_leaf(&node)?;
                        self.store.remove_branch(&node)?;
                    } else {
                        path.push((key.fork_height(branch_node.key()), *branch_node.node()));
                    }
                    break;
                }
            }
        }

        // compute and store new leaf
        let mut node = hash_leaf::<H>(&key, &value.to_h256());
        // notice when value is zero the leaf is deleted, so we do not need to store it
        if !node.is_zero() {
            self.store.insert_leaf(node, LeafNode { key, value })?;

            // build at least one branch for leaf
            self.store.insert_branch(
                node,
                BranchNode {
                    key,
                    fork_height: 0,
                    node_type: NodeType::Single(node),
                },
            )?;
        }

        // recompute the tree from bottom to top
        for (height, sibling) in path.into_iter().rev() {
            let is_right = key.get_bit(height);
            let parent = if is_right {
                merge::<H>(&sibling, &node)
            } else {
                merge::<H>(&node, &sibling)
            };

            if !node.is_zero() {
                // node is exists
                let branch_node = BranchNode {
                    key,
                    fork_height: height,
                    node_type: NodeType::Pair(node, sibling),
                };
                self.store.insert_branch(parent, branch_node)?;
            }
            node = parent;
        }
        self.root = node;
        Ok(&self.root)
    }

    /// Get value of a leaf
    /// return zero value if leaf not exists
    pub fn get(&self, key: &H256) -> Result<V> {
        if self.is_empty() {
            return Ok(V::zero());
        }

        let mut node = self.root;
        loop {
            let branch_node = self.store.get_branch(&node)?.expect("stored branch node");
            if branch_node.is_pair() {
                let is_right = key.get_bit(branch_node.fork_height);
                let (left, right) = branch_node.branch(branch_node.fork_height);
                node = if is_right { *right } else { *left };
            } else {
                if key == branch_node.key() {
                    return Ok(self
                        .store
                        .get_leaf(branch_node.node())?
                        .expect("stored leaf node")
                        .value);
                } else {
                    return Ok(V::zero());
                }
            }
        }
    }

    /// fetch merkle path of key into cache
    /// cache: (height, key) -> node
    fn fetch_merkle_path(&self, key: &H256, cache: &mut BTreeMap<(u8, H256), H256>) -> Result<()> {
        let mut node = self.root;
        loop {
            let branch_node = self.store.get_branch(&node)?.expect("stored branch node");
            let height = max(key.fork_height(branch_node.key()), branch_node.fork_height);
            let is_right = key.get_bit(height);
            let mut sibling_key = key.parent_path(height);
            if !is_right {
                // mark sibling's index, sibling on the right path.
                sibling_key.set_bit(height);
            };

            if branch_node.is_pair() {
                if height > branch_node.fork_height {
                    cache.entry((height, sibling_key)).or_insert(node);
                    break;
                } else {
                    let (left, right) = branch_node.branch(height);
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
                    cache.insert((height, sibling_key), sibling);
                }
            } else {
                if key != branch_node.key() {
                    cache.insert((height, sibling_key), *branch_node.node());
                }
                break;
            }
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
        let mut cache: BTreeMap<(u8, H256), H256> = Default::default();
        if !self.is_empty() {
            for k in &keys {
                self.fetch_merkle_path(k, &mut cache)?;
            }
        }

        // (node, height)
        let mut proof: Vec<(H256, u8)> = Vec::with_capacity(EXPECTED_PATH_SIZE * keys.len());
        // key_index -> merkle path height
        let mut leaves_path: Vec<Vec<u8>> = Vec::with_capacity(keys.len());
        leaves_path.resize_with(keys.len(), Default::default);

        let keys_len = keys.len();
        // build merkle proofs from bottom to up
        // (key, height, key_index)
        let mut queue: VecDeque<(H256, u8, usize)> = keys
            .into_iter()
            .enumerate()
            .map(|(i, k)| (k, 0, i))
            .collect();

        while let Some((key, height, leaf_index)) = queue.pop_front() {
            if queue.is_empty() && cache.is_empty() {
                // tree only contains one leaf
                if leaves_path[leaf_index].is_empty() {
                    leaves_path[leaf_index].push(core::u8::MAX);
                }
                break;
            }
            // compute sibling key
            let mut sibling_key = key.parent_path(height);

            let is_right = key.get_bit(height);
            if is_right {
                // sibling on left
                sibling_key.clear_bit(height);
            } else {
                // sibling on right
                sibling_key.set_bit(height);
            }
            if Some((&sibling_key, &height))
                == queue
                    .front()
                    .map(|(sibling_key, height, _leaf_index)| (sibling_key, height))
            {
                // drop the sibling, mark sibling's merkle path
                let (_sibling_key, height, leaf_index) = queue.pop_front().unwrap();
                leaves_path[leaf_index].push(height);
            } else {
                match cache.remove(&(height, sibling_key)) {
                    Some(sibling) => {
                        // save first non-zero sibling's height for leaves
                        proof.push((sibling, height));
                    }
                    None => {
                        // skip zero siblings
                        if !is_right {
                            sibling_key.clear_bit(height);
                        }
                        if height == core::u8::MAX {
                            if leaves_path[leaf_index].is_empty() {
                                leaves_path[leaf_index].push(height);
                            }
                            break;
                        } else {
                            let parent_key = sibling_key;
                            queue.push_back((parent_key, height + 1, leaf_index));
                            continue;
                        }
                    }
                }
            }
            // find new non-zero sibling, append to leaf's path
            leaves_path[leaf_index].push(height);
            if height == core::u8::MAX {
                break;
            } else {
                // get parent_key, which k.get_bit(height) is false
                let parent_key = if is_right { sibling_key } else { key };
                queue.push_back((parent_key, height + 1, leaf_index));
            }
        }
        debug_assert_eq!(leaves_path.len(), keys_len);
        Ok(MerkleProof::new(leaves_path, proof))
    }
}
