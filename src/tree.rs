use crate::{
    collections::{BTreeMap, VecDeque},
    error::{Error, Result},
    hasher::Hasher,
    store::{Entry, Store},
    vec::Vec,
    H256,
};
use core::{
    cmp::{max, Ordering},
    marker::PhantomData,
};

/// log2(256) * 2
pub const EXPECTED_PATH_SIZE: usize = 16;
const TREE_HEIGHT: usize = 256;

#[derive(Debug, Hash, Ord, Eq, PartialEq)]
pub enum Key {
    Node(H256),
    Leaf(H256),
}

impl Key {
    pub fn inner(&self) -> &H256 {
        match self {
            Key::Node(inner) => inner,
            Key::Leaf(inner) => inner,
        }
    }

    fn type_id(&self) -> usize {
        match self {
            Key::Node(_) => 0,
            Key::Leaf(_) => 1,
        }
    }
}

impl PartialOrd for Key {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        let self_type = self.type_id();
        let other_type = other.type_id();
        if self_type != other_type {
            return self_type.partial_cmp(&other_type);
        }
        self.inner().partial_cmp(other.inner())
    }
}

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
pub struct LeafNode {
    pub key: H256,
    pub value: H256,
}

#[derive(Debug, Eq, PartialEq, Clone)]
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
    if lhs.is_zero() {
        return *rhs;
    } else if rhs.is_zero() {
        return *lhs;
    }
    let mut hasher = H::default();
    hasher.write_h256(lhs);
    hasher.write_h256(rhs);
    hasher.finish()
}

/// hash_leaf = hash(key | value)
/// zero value represent delete the key, this function return zero for zero value
fn hash_leaf<H: Hasher + Default>(key: &H256, value: &H256) -> H256 {
    if value.is_zero() {
        return H256::zero();
    }
    let mut hasher = H::default();
    hasher.write_h256(key);
    hasher.write_h256(value);
    hasher.finish()
}

/// Sparse merkle tree
#[derive(Default, Debug)]
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

    pub fn is_empty(&self) -> bool {
        self.root.is_zero()
    }

    /// Get backend store
    pub fn store(&self) -> &Store {
        &self.store
    }

    /// Update a leaf, return new merkle root
    /// set a key to zero to delete the key
    pub fn update(&mut self, key: H256, value: H256) -> Result<&H256> {
        // store the path, sparse index will ignore zero members
        let mut path: BTreeMap<_, _> = Default::default();
        // walk path from top to bottom
        let mut node = self.root;
        let mut branch = self
            .store
            .get(&Key::Node(node))
            .map(Clone::clone)
            .and_then(|n| n.branch());
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
            self.store.remove(&Key::Node(node));
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
            branch = self
                .store
                .get(&Key::Node(node))
                .map(Clone::clone)
                .and_then(|n| n.branch());
            if let Some(branch_node) = branch.as_ref() {
                height = max(key.fork_height(&branch_node.key), branch_node.fork_height);
            }
        }
        // delete previous leaf
        if let Entry::Occupied(entry) = self.store.entry(Key::Leaf(node)) {
            if entry.get().leaf_ref().map(|leaf| &leaf.key) == Some(&key) {
                entry.remove();
            }
        }

        // compute and store new leaf
        let mut node = hash_leaf::<H>(&key, &value);
        // notice when value is zero the leaf is deleted, so we do not need to store it
        if !node.is_zero() {
            self.store
                .insert(Key::Leaf(node), Node::Leaf(LeafNode { key, value }));
        }
        // build at least one branch for leaf
        self.store.insert(
            Key::Node(node),
            Node::Branch(BranchNode {
                key,
                fork_height: 0,
                node,
                sibling: H256::zero(),
            }),
        );

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
            self.store
                .insert(Key::Node(parent), Node::Branch(branch_node));
            node = parent;
        }
        self.root = node;
        Ok(&self.root)
    }

    /// Get value of a leaf
    pub fn get(&self, key: &H256) -> Result<&H256> {
        const ZERO: H256 = H256::zero();
        let mut node = &self.root;
        // children must equals to zero when parent equals to zero
        while !node.is_zero() {
            let branch_node = match self
                .store
                .get(&Key::Node(*node))
                .and_then(|n| n.branch_ref())
            {
                Some(branch_node) => branch_node,
                None => {
                    break;
                }
            };
            let is_right = key.get_bit(branch_node.fork_height as u8);
            let (left, right) = branch_node.branch(branch_node.fork_height as u8);
            if is_right {
                node = right;
            } else {
                node = left;
            }
            if branch_node.fork_height == 0 {
                break;
            }
        }

        // return zero is leaf_key is zero
        if node.is_zero() {
            return Ok(&ZERO);
        }
        // get leaf node
        match self.store.get(&Key::Leaf(*node)).and_then(|n| n.leaf_ref()) {
            Some(leaf) if &leaf.key == key => Ok(&leaf.value),
            _ => Ok(&ZERO),
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
            .get(&Key::Node(node))
            .and_then(|n| n.branch_ref())
            .map(|b| max(b.key.fork_height(&key), b.fork_height))
            .unwrap_or(0);
        while !node.is_zero() {
            // the descendants are zeros, so we can break the loop
            if node.is_zero() {
                break;
            }
            match self
                .store
                .get(&Key::Node(node))
                .and_then(|n| n.branch_ref())
            {
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
                    if let Some(branch_node) = self
                        .store
                        .get(&Key::Node(node))
                        .and_then(|n| n.branch_ref())
                    {
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

        if self.root.is_zero() {
            return Err(Error::EmptyTree);
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

#[derive(Debug, Clone)]
pub struct MerkleProof {
    leaves_path: Vec<Vec<u8>>,
    proof: Vec<(H256, u8)>,
}

impl MerkleProof {
    /// Create MerkleProof
    /// leaves_path: contains height of non-zero siblings
    /// proof: contains merkle path for each leaves it's height
    pub fn new(leaves_path: Vec<Vec<u8>>, proof: Vec<(H256, u8)>) -> Self {
        MerkleProof { leaves_path, proof }
    }

    /// Destruct the structure, useful for serialization
    pub fn take(self) -> (Vec<Vec<u8>>, Vec<(H256, u8)>) {
        let MerkleProof { leaves_path, proof } = self;
        (leaves_path, proof)
    }

    /// number of leaves required by this merkle proof
    pub fn leaves_count(&self) -> usize {
        self.leaves_path.len()
    }

    /// return the inner leaves_path vector
    pub fn leaves_path(&self) -> &Vec<Vec<u8>> {
        &self.leaves_path
    }

    /// return proof merkle path
    pub fn proof(&self) -> &Vec<(H256, u8)> {
        &self.proof
    }

    /// Compute root from proof
    /// leaves: a vector of (key, value)
    ///
    /// return EmptyProof error when proof is empty
    /// return CorruptedProof error when proof is invalid
    pub fn compute_root<H: Hasher + Default>(self, mut leaves: Vec<(H256, H256)>) -> Result<H256> {
        if leaves.is_empty() {
            return Err(Error::EmptyKeys);
        } else if leaves.len() != self.leaves_count() {
            return Err(Error::IncorrectNumberOfLeaves {
                expected: self.leaves_count(),
                actual: leaves.len(),
            });
        }

        let (leaves_path, proof) = self.take();
        let mut leaves_path: Vec<VecDeque<_>> = leaves_path.into_iter().map(Into::into).collect();
        let mut proof: VecDeque<_> = proof.into();

        // sort leaves
        leaves.sort_unstable_by_key(|(k, _v)| *k);
        // tree_buf: (height, key) -> (key_index, node)
        let mut tree_buf: BTreeMap<_, _> = leaves
            .into_iter()
            .enumerate()
            .map(|(i, (k, v))| ((0, k), (i, hash_leaf::<H>(&k, &v))))
            .collect();
        // rebuild the tree from bottom to top
        while !tree_buf.is_empty() {
            // pop_front from tree_buf, the API is unstable
            let (&(mut height, key), &(leaf_index, node)) = tree_buf.iter().next().unwrap();
            tree_buf.remove(&(height, key));

            if proof.is_empty() && tree_buf.is_empty() {
                return Ok(node);
            } else if height == TREE_HEIGHT {
                if !proof.is_empty() {
                    return Err(Error::CorruptedProof);
                }
                return Ok(node);
            }

            let mut sibling_key = key.parent_path(height as u8);
            if !key.get_bit(height as u8) {
                sibling_key.set_bit(height as u8)
            }
            let (sibling, sibling_height) =
                if Some(&(height, sibling_key)) == tree_buf.keys().next() {
                    let (_leaf_index, sibling) = tree_buf
                        .remove(&(height, sibling_key))
                        .expect("pop sibling");
                    (sibling, height)
                } else {
                    let merge_height = leaves_path[leaf_index]
                        .front()
                        .map(|h| *h as usize)
                        .unwrap_or(height);
                    if height != merge_height {
                        debug_assert!(height < merge_height);
                        let parent_key = key.copy_bits(merge_height as u8..);
                        // skip zeros
                        tree_buf.insert((merge_height, parent_key), (leaf_index, node));
                        continue;
                    }
                    let (node, height) = proof.pop_front().expect("pop proof");
                    debug_assert_eq!(height, leaves_path[leaf_index][0]);
                    (node, height as usize)
                };
            debug_assert!(height <= sibling_height);
            if height < sibling_height {
                height = sibling_height;
            }
            // skip zero merkle path
            let parent_key = key.parent_path(height as u8);

            let parent = if key.get_bit(height as u8) {
                merge::<H>(&sibling, &node)
            } else {
                merge::<H>(&node, &sibling)
            };
            leaves_path[leaf_index].pop_front();
            tree_buf.insert((height + 1, parent_key), (leaf_index, parent));
        }

        Err(Error::CorruptedProof)
    }

    /// Verify merkle proof
    /// see compute_root_from_proof
    pub fn verify<H: Hasher + Default>(
        self,
        root: &H256,
        leaves: Vec<(H256, H256)>,
    ) -> Result<bool> {
        let calculated_root = self.compute_root::<H>(leaves)?;
        Ok(&calculated_root == root)
    }
}
