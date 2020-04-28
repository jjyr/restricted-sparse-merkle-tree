use crate::{
    collections::{BTreeMap, VecDeque},
    error::{Error, Result},
    traits::{Hasher, Store, Value},
    vec::Vec,
    H256,
};
use core::{cmp::max, marker::PhantomData};

type Range = core::ops::Range<usize>;

/// log2(256) * 2
pub const EXPECTED_PATH_SIZE: usize = 16;
const TREE_HEIGHT: usize = 256;

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

    pub fn is_empty(&self) -> bool {
        self.root.is_zero()
    }

    /// Get backend store
    pub fn store(&self) -> &S {
        &self.store
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

    pub fn compile(self, mut leaves: Vec<(H256, H256)>) -> Result<CompiledMerkleProof> {
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
            .map(|(i, (k, _v))| ((0, k), (i, leaf_program(i))))
            .collect();
        // rebuild the tree from bottom to top
        while !tree_buf.is_empty() {
            // pop_front from tree_buf, the API is unstable
            let &(mut height, key) = tree_buf.keys().next().unwrap();
            let (leaf_index, program) = tree_buf.remove(&(height, key)).unwrap();

            if proof.is_empty() && tree_buf.is_empty() {
                return Ok(CompiledMerkleProof(program.0));
            } else if height == TREE_HEIGHT {
                if !proof.is_empty() {
                    return Err(Error::CorruptedProof);
                }
                return Ok(CompiledMerkleProof(program.0));
            }

            let mut sibling_key = key.parent_path(height as u8);
            if !key.get_bit(height as u8) {
                sibling_key.set_bit(height as u8)
            }

            let (parent_key, parent_program, height) =
                if Some(&(height, sibling_key)) == tree_buf.keys().next() {
                    let (_leaf_index, sibling_program) = tree_buf
                        .remove(&(height, sibling_key))
                        .expect("pop sibling");
                    let parent_key = key.parent_path(height as u8);
                    let parent_program = merge_program(&program, &sibling_program, height as u8)?;
                    (parent_key, parent_program, height)
                } else {
                    let merge_height = leaves_path[leaf_index]
                        .front()
                        .map(|h| *h as usize)
                        .unwrap_or(height);
                    if height != merge_height {
                        debug_assert!(height < merge_height);
                        let parent_key = key.copy_bits(merge_height as u8..);
                        // skip zeros
                        tree_buf.insert((merge_height, parent_key), (leaf_index, program));
                        continue;
                    }
                    let (proof, proof_height) = proof.pop_front().expect("pop proof");
                    debug_assert_eq!(proof_height, leaves_path[leaf_index][0]);
                    let proof_height = proof_height as usize;
                    debug_assert!(height <= proof_height);
                    if height < proof_height {
                        height = proof_height;
                    }

                    let parent_key = key.parent_path(height as u8);
                    let parent_program = proof_program(&program, proof, height as u8);
                    (parent_key, parent_program, height)
                };

            leaves_path[leaf_index].pop_front();
            tree_buf.insert((height + 1, parent_key), (leaf_index, parent_program));
        }

        Err(Error::CorruptedProof)
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

fn leaf_program(leaf_index: usize) -> (Vec<u8>, Option<Range>) {
    let mut program = Vec::with_capacity(1);
    program.push(0x4C);
    (
        program,
        Some(Range {
            start: leaf_index,
            end: leaf_index + 1,
        }),
    )
}

fn proof_program(
    child: &(Vec<u8>, Option<Range>),
    proof: H256,
    height: u8,
) -> (Vec<u8>, Option<Range>) {
    let (child_program, child_range) = child;
    let mut program = Vec::new();
    program.resize(34 + child_program.len(), 0x50);
    program[..child_program.len()].copy_from_slice(child_program);
    program[child_program.len() + 1] = height;
    program[child_program.len() + 2..].copy_from_slice(proof.as_slice());
    (program, child_range.clone())
}

fn merge_program(
    a: &(Vec<u8>, Option<Range>),
    b: &(Vec<u8>, Option<Range>),
    height: u8,
) -> Result<(Vec<u8>, Option<Range>)> {
    let (a_program, a_range) = a;
    let (b_program, b_range) = b;
    let (a_comes_first, range) = if a_range.is_none() || b_range.is_none() {
        let range = if a_range.is_none() { b_range } else { a_range }
            .clone()
            .unwrap();
        (true, range)
    } else {
        let a_range = a_range.clone().unwrap();
        let b_range = b_range.clone().unwrap();
        if a_range.end == b_range.start {
            (
                true,
                Range {
                    start: a_range.start,
                    end: b_range.end,
                },
            )
        } else {
            return Err(Error::NonMergableRange);
        }
    };
    let mut program = Vec::new();
    program.resize(2 + a_program.len() + b_program.len(), 0x48);
    if a_comes_first {
        program[..a_program.len()].copy_from_slice(a_program);
        program[a_program.len()..a_program.len() + b_program.len()].copy_from_slice(b_program);
    } else {
        program[..b_program.len()].copy_from_slice(b_program);
        program[b_program.len()..a_program.len() + b_program.len()].copy_from_slice(a_program);
    }
    program[a_program.len() + b_program.len() + 1] = height;
    Ok((program, Some(range)))
}

#[derive(Debug, Clone)]
pub struct CompiledMerkleProof(pub Vec<u8>);

impl CompiledMerkleProof {
    pub fn compute_root<H: Hasher + Default>(&self, mut leaves: Vec<(H256, H256)>) -> Result<H256> {
        leaves.sort_unstable_by_key(|(k, _v)| *k);
        let mut program_index = 0;
        let mut leave_index = 0;
        let mut stack = Vec::new();
        while program_index < self.0.len() {
            let code = self.0[program_index];
            program_index += 1;
            match code {
                // L
                0x4C => {
                    if leave_index >= leaves.len() {
                        return Err(Error::CorruptedStack);
                    }
                    let (k, v) = leaves[leave_index];
                    stack.push((k, hash_leaf::<H>(&k, &v)));
                    leave_index += 1;
                }
                // P
                0x50 => {
                    if stack.is_empty() {
                        return Err(Error::CorruptedStack);
                    }
                    if program_index + 33 > self.0.len() {
                        return Err(Error::CorruptedProof);
                    }
                    let height = self.0[program_index];
                    program_index += 1;
                    let mut data = [0u8; 32];
                    data.copy_from_slice(&self.0[program_index..program_index + 32]);
                    program_index += 32;
                    let proof = H256::from(data);
                    let (key, value) = stack.pop().unwrap();
                    let parent_key = key.parent_path(height);
                    let parent = if key.get_bit(height) {
                        merge::<H>(&proof, &value)
                    } else {
                        merge::<H>(&value, &proof)
                    };
                    stack.push((parent_key, parent));
                }
                // H
                0x48 => {
                    if stack.len() < 2 {
                        return Err(Error::CorruptedStack);
                    }
                    if program_index >= self.0.len() {
                        return Err(Error::CorruptedProof);
                    }
                    let height = self.0[program_index];
                    program_index += 1;
                    let (key_b, value_b) = stack.pop().unwrap();
                    let (key_a, value_a) = stack.pop().unwrap();
                    let parent_key_a = key_a.copy_bits(height..);
                    let parent_key_b = key_b.copy_bits(height..);
                    let a_set = key_a.get_bit(height);
                    let b_set = key_b.get_bit(height);
                    let mut sibling_key_a = parent_key_a;
                    if !a_set {
                        sibling_key_a.set_bit(height);
                    }
                    // Test if a and b are siblings
                    if !(sibling_key_a == parent_key_b && (a_set ^ b_set)) {
                        return Err(Error::NonSiblings);
                    }
                    let parent = if key_a.get_bit(height) {
                        merge::<H>(&value_b, &value_a)
                    } else {
                        merge::<H>(&value_a, &value_b)
                    };
                    stack.push((parent_key_a, parent));
                }
                _ => return Err(Error::InvalidCode(code)),
            }
        }
        if stack.len() != 1 {
            return Err(Error::CorruptedStack);
        }
        Ok(stack[0].1)
    }

    pub fn verify<H: Hasher + Default>(
        &self,
        root: &H256,
        leaves: Vec<(H256, H256)>,
    ) -> Result<bool> {
        let calculated_root = self.compute_root::<H>(leaves)?;
        Ok(&calculated_root == root)
    }
}
