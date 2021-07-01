use crate::{
    error::{Error, Result},
    merge::{hash_leaf, merge},
    traits::Hasher,
    vec::Vec,
    H256,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MerkleProof {
    // leaf path represented by bitmap
    leaves_path: Vec<H256>,
    // needed sibling node hash
    proof: Vec<H256>,
}

impl MerkleProof {
    /// Create MerkleProof
    /// leaves_path: contains height of non-zero siblings
    /// proof: contains merkle path for each leaves it's height
    pub fn new(leaves_path: Vec<H256>, proof: Vec<H256>) -> Self {
        MerkleProof { leaves_path, proof }
    }

    /// Destruct the structure, useful for serialization
    pub fn take(self) -> (Vec<H256>, Vec<H256>) {
        let MerkleProof { leaves_path, proof } = self;
        (leaves_path, proof)
    }

    /// number of leaves required by this merkle proof
    pub fn leaves_count(&self) -> usize {
        self.leaves_path.len()
    }

    /// return the inner leaves_path vector
    pub fn leaves_path(&self) -> &Vec<H256> {
        &self.leaves_path
    }

    /// return proof merkle path
    pub fn proof(&self) -> &Vec<H256> {
        &self.proof
    }

    /// convert merkle proof into CompiledMerkleProof
    pub fn compile(self) -> CompiledMerkleProof {
        let (leaves_path, proof) = self.take();
        let leaves_len = leaves_path.len();
        let mut data = vec![0u8; (leaves_len + proof.len()) * 32];
        for (idx, path) in leaves_path.into_iter().enumerate() {
            let offset = idx * 32;
            data[offset..offset + 32].copy_from_slice(path.as_slice());
        }
        for (idx, sibling_node_hash) in proof.into_iter().enumerate() {
            let offset = (leaves_len + idx) * 32;
            data[offset..offset + 32].copy_from_slice(sibling_node_hash.as_slice());
        }
        CompiledMerkleProof(data)
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
        // sort leaves
        leaves.sort_unstable_by_key(|(k, _v)| *k);

        let (leaves_path, proof) = self.take();

        let mut proof_index = 0;
        // (path_index, key, node_hash)
        let mut current_nodes: Vec<(usize, H256, H256)> = leaves
            .into_iter()
            .enumerate()
            .map(|(path_idx, (key, value))| (path_idx, key, hash_leaf::<H>(&key, &value)))
            .collect();
        let mut next_nodes: Vec<(usize, H256, H256)> = Default::default();
        for height in 0..=core::u8::MAX {
            let mut key_idx = 0;
            while key_idx < current_nodes.len() {
                let (path_idx_a, key_a, node_a) = current_nodes[key_idx];
                let parent_key_a = key_a.parent_path(height);

                let mut non_sibling_nodes = Vec::with_capacity(2);
                if key_idx + 1 < current_nodes.len() {
                    let (path_idx_b, key_b, node_b) = current_nodes[key_idx + 1];
                    let parent_key_b = key_b.parent_path(height);
                    if parent_key_a == parent_key_b {
                        let parent_node = merge::<H>(height, &parent_key_a, &node_a, &node_b);
                        next_nodes.push((path_idx_a, key_a, parent_node));
                        key_idx += 2;
                    } else {
                        non_sibling_nodes.push((path_idx_a, key_a, node_a, parent_key_a));
                        if key_idx + 2 == current_nodes.len() {
                            non_sibling_nodes.push((path_idx_b, key_b, node_b, parent_key_b));
                        }
                    }
                } else {
                    non_sibling_nodes.push((path_idx_a, key_a, node_a, parent_key_a));
                }

                for (path_idx, current_key, current_node, parent_key) in
                    non_sibling_nodes.into_iter()
                {
                    let path = leaves_path[path_idx];
                    let none_zero_sibling = path.get_bit(height);
                    let is_right = current_key.is_right(height);
                    let (left, right) = if none_zero_sibling {
                        if proof_index == proof.len() {
                            return Err(Error::CorruptedProof);
                        }
                        let sibling_node = proof[proof_index];
                        proof_index += 1;
                        if is_right {
                            (sibling_node, current_node)
                        } else {
                            (current_node, sibling_node)
                        }
                    } else if is_right {
                        (H256::zero(), current_node)
                    } else {
                        (current_node, H256::zero())
                    };
                    let node_hash = merge::<H>(height, &parent_key, &left, &right);
                    next_nodes.push((path_idx, current_key, node_hash));
                    key_idx += 1;
                }
            }
            current_nodes = core::mem::take(&mut next_nodes);
        }

        if proof_index != proof.len() {
            return Err(Error::CorruptedProof);
        }

        if current_nodes.len() != 1 {
            Err(Error::CorruptedProof)
        } else {
            Ok(current_nodes[0].2)
        }
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

/// An structure optimized for verify merkle proof
#[derive(Debug, Clone)]
pub struct CompiledMerkleProof(pub Vec<u8>);

impl CompiledMerkleProof {
    pub fn compute_root<H: Hasher + Default>(&self, leaves: Vec<(H256, H256)>) -> Result<H256> {
        if self.0.len() % 32 != 0 {
            return Err(Error::CorruptedProof);
        }
        if self.0.len() / 32 < leaves.len() {
            return Err(Error::CorruptedProof);
        }

        let sibling_node_size = self.0.len() / 32 - leaves.len();
        let mut data = [0u8; 32];
        let mut leaves_path = Vec::with_capacity(leaves.len());
        let mut proof = Vec::with_capacity(sibling_node_size);
        for idx in 0..leaves.len() {
            let offset = idx * 32;
            data.copy_from_slice(&self.0[offset..offset + 32]);
            leaves_path.push(H256::from(data));
        }
        for idx in 0..sibling_node_size {
            let offset = (idx + leaves.len()) * 32;
            data.copy_from_slice(&self.0[offset..offset + 32]);
            proof.push(H256::from(data));
        }
        MerkleProof::new(leaves_path, proof).compute_root::<H>(leaves)
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

impl Into<Vec<u8>> for CompiledMerkleProof {
    fn into(self) -> Vec<u8> {
        self.0
    }
}
