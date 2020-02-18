use crate::{
    error::{Error, Result},
    get_bit,
    hasher::Hasher,
    sparse_index::SparseIndex,
    store::Store,
    vec::Vec,
    H256, TREE_HEIGHT, ZERO_HASH,
};
use core::marker::PhantomData;

/// log2(256) * 2
pub const EXPECTED_PATH_SIZE: usize = 16;

/// A branch in the SMT
#[derive(Debug, Eq, PartialEq)]
pub struct BranchNode {
    pub left: H256,
    pub right: H256,
}
/// A leaf in the SMT
#[derive(Debug, Eq, PartialEq)]
pub struct LeafNode {
    pub key: H256,
    pub value: H256,
}
#[derive(Debug, Eq, PartialEq)]
pub enum Node {
    Branch(BranchNode),
    Leaf(LeafNode),
}

impl Node {
    fn branch(self) -> Option<BranchNode> {
        match self {
            Node::Branch(n) => Some(n),
            _ => None,
        }
    }
    fn branch_ref(&self) -> Option<&BranchNode> {
        match self {
            Node::Branch(n) => Some(n),
            _ => None,
        }
    }
    fn leaf_ref(&self) -> Option<&LeafNode> {
        match self {
            Node::Leaf(n) => Some(n),
            _ => None,
        }
    }
}

/// Merge two hash
/// this function optimized for ZERO_HASH
/// if one of lhs or rhs is ZERO_HASH, this function just return another one
fn merge<H: Hasher + Default>(lhs: &H256, rhs: &H256) -> H256 {
    if lhs == &ZERO_HASH {
        return *rhs;
    } else if rhs == &ZERO_HASH {
        return *lhs;
    }
    let mut hasher = H::default();
    hasher.write_h256(lhs);
    hasher.write_h256(rhs);
    hasher.finish()
}

/// hash_leaf = hash(key | value)
pub fn hash_leaf<H: Hasher + Default>(key: &H256, value: &H256) -> H256 {
    let mut hasher = H::default();
    hasher.write_h256(key);
    hasher.write_h256(value);
    hasher.finish()
}

/// Sparse merkle tree
#[derive(Default)]
pub struct SparseMerkleTree<H> {
    store: Store,
    root: H256,
    phantom: PhantomData<H>,
}

impl<H: Hasher + Default> SparseMerkleTree<H> {
    /// Build a merkle tree from root and store
    pub fn new(root: H256, store: Store) -> SparseMerkleTree<H> {
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

    /// Get backend store
    pub fn store(&self) -> &Store {
        &self.store
    }

    /// Update a leaf, return new merkle root
    pub fn update(&mut self, key: H256, value: H256) -> Result<&H256> {
        let mut node = self.root;
        // store the path, sparse index will ignore zero members
        let mut path = SparseIndex::default();
        // walk path from top to bottom
        for height in (0..TREE_HEIGHT).rev() {
            // the descendants are all zeros
            if node == ZERO_HASH {
                path.set_len(TREE_HEIGHT);
                break;
            }
            match self.store.remove(&(height, node)).and_then(|n| n.branch()) {
                Some(BranchNode { left, right }) => {
                    let is_right = get_bit(&key, height);
                    if is_right {
                        node = right;
                        path.push(left);
                    } else {
                        node = left;
                        path.push(right);
                    }
                }
                None => return Err(Error::MissingKey(height, node)),
            };
        }
        // delete previous leaf
        self.store.remove(&(TREE_HEIGHT, node));

        // compute and store new leaf
        let leaf_key = hash_leaf::<H>(&key, &value);
        // store leaf on TREE_HEIGHT, so no other key will conflict with it
        self.store
            .insert((TREE_HEIGHT, leaf_key), Node::Leaf(LeafNode { key, value }));

        // recompute the tree from bottom to top
        let mut node = leaf_key;
        for height in 0..TREE_HEIGHT {
            let is_right = get_bit(&key, height);
            let sibling = path.pop().unwrap_or(ZERO_HASH);
            let (parent, branch_node) = if is_right {
                (
                    merge::<H>(&sibling, &node),
                    Node::Branch(BranchNode {
                        left: sibling,
                        right: node,
                    }),
                )
            } else {
                (
                    merge::<H>(&node, &sibling),
                    Node::Branch(BranchNode {
                        left: node,
                        right: sibling,
                    }),
                )
            };
            self.store.insert((height, parent), branch_node);
            node = parent;
        }
        self.root = node;
        Ok(&self.root)
    }

    /// Get value of a leaf
    pub fn get(&self, key: &H256) -> Result<&H256> {
        let mut node = &self.root;
        for height in (0..TREE_HEIGHT).rev() {
            // children must equals to zero when parent equals to zero
            if node == &ZERO_HASH {
                return Ok(&ZERO_HASH);
            }
            let (left, right) = match self
                .store
                .get(&(height, *node))
                .and_then(|n| n.branch_ref())
            {
                Some(BranchNode { left, right }) => (left, right),
                None => return Err(Error::MissingKey(height, *node)),
            };
            let is_right = get_bit(key, height);
            if is_right {
                node = &right;
            } else {
                node = &left;
            }
        }
        // get leaf node
        self.store
            .get(&(TREE_HEIGHT, *node))
            .and_then(|n| n.leaf_ref().map(|leaf| &leaf.value))
            .ok_or(Error::MissingKey(0, *node))
    }

    /// Generate merkle proof
    pub fn merkle_proof(&self, key: &H256) -> Result<Vec<H256>> {
        // return empty proof for empty tree
        if self.root == ZERO_HASH {
            return Ok(Vec::new());
        }

        let mut node = &self.root;
        // notate the side of the path for each proof item
        let mut path = SparseIndex::default();
        for height in (0..TREE_HEIGHT).rev() {
            // all decendents should just be zeros
            if node == &ZERO_HASH {
                break;
            }
            let (left, right) = match self
                .store
                .get(&(height, *node))
                .and_then(|n| n.branch_ref())
            {
                Some(BranchNode { left, right }) => (left, right),
                None => return Err(Error::MissingKey(height, *node)),
            };

            let is_right = get_bit(key, height);
            if is_right {
                // mark index, we are on the right path!
                node = &right;
                path.push(*left);
            } else {
                node = &left;
                path.push(*right);
            }
        }
        Ok(path.into_vec())
    }
}

/// Verify merkle proof
/// see compute_root_from_proof
pub fn verify_proof<H: Hasher + Default>(
    proof: Vec<H256>,
    root: &H256,
    key: &H256,
    value: &H256,
) -> Result<bool> {
    let calculated_root = compute_root_from_proof::<H>(proof, key, value)?;
    Ok(&calculated_root == root)
}

/// Compute root from proof
/// proof is a array contains generated merkle path and a sparse index
/// NOTICE even we can calculate a root from proof, it only means the proof's format is correct,
/// doesn't represent the proof itself is valid.
///
/// return EmptyProof error when proof is empty
/// return CorruptedProof error when proof is invalid
pub fn compute_root_from_proof<H: Hasher + Default>(
    proof: Vec<H256>,
    key: &H256,
    value: &H256,
) -> Result<H256> {
    // technically, a sparse merkle tree
    // constains at least 1 element to represent sparse index,
    // and constains at most TREE_HEIGHT plus 1 elements
    if proof.is_empty() {
        return Err(Error::EmptyProof);
    }
    let mut path = SparseIndex::from_vec(proof, TREE_HEIGHT).ok_or(Error::CorruptedProof)?;

    let leaf_key = hash_leaf::<H>(key, &value);
    let mut node = leaf_key;
    // verify tree from bottom to top
    for i in 0..TREE_HEIGHT {
        let sibling = path.pop().unwrap_or(ZERO_HASH);
        let is_right = get_bit(&key, i);
        if is_right {
            node = merge::<H>(&sibling, &node);
        } else {
            node = merge::<H>(&node, &sibling);
        }
    }
    if !path.buf().is_empty() {
        return Err(Error::CorruptedProof);
    }
    Ok(node)
}
