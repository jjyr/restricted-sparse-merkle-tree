use crate::{
    collections::{BTreeMap, VecDeque},
    error::{Error, Result},
    merge::{hash_leaf, merge},
    traits::Hasher,
    vec::Vec,
    H256, TREE_HEIGHT,
};

type Range = core::ops::Range<usize>;

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

    /// convert merkle proof into CompiledMerkleProof
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

/// An structure optimized for verify merkle proof
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
