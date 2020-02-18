use super::*;
use crate::{
    blake2b::Blake2bHasher,
    error::Error,
    tree::{hash_leaf, verify_proof, SparseMerkleTree},
};
use proptest::prelude::*;
use rand::{thread_rng, Rng};

#[test]
fn test_default_root() {
    let mut tree = SparseMerkleTree::<Blake2bHasher>::default();
    assert_eq!(tree.store().len(), 0);
    assert_eq!(tree.root(), &ZERO_HASH);

    // should not equals to zero even leaf value is zero
    tree.update(ZERO_HASH, ZERO_HASH).expect("update");
    assert_ne!(tree.root(), &ZERO_HASH);
    assert_eq!(tree.get(&ZERO_HASH).expect("get"), &ZERO_HASH);
}

#[test]
fn test_default_merkle_proof() {
    let tree = SparseMerkleTree::<Blake2bHasher>::default();
    let proof = tree.merkle_proof(&ZERO_HASH).expect("proof");
    assert_eq!(proof.len(), 0);
    assert_eq!(
        verify_proof::<Blake2bHasher>(proof, tree.root(), &ZERO_HASH, &ZERO_HASH),
        Err(Error::EmptyProof)
    );
    // when proof contains only zero sparse index, leaf_hash is root
    let zero_leaf = hash_leaf::<Blake2bHasher>(&ZERO_HASH, &ZERO_HASH);
    assert!(
        verify_proof::<Blake2bHasher>(vec![ZERO_HASH], &zero_leaf, &ZERO_HASH, &ZERO_HASH)
            .expect("verify proof")
    );
}

fn test_construct(key: H256, value: H256) {
    // insert same value to sibling key will construct a different root

    let mut tree = SparseMerkleTree::<Blake2bHasher>::default();
    tree.update(key, value.clone()).expect("update");

    let mut sibling_key = key;
    let i = sibling_key.len() - 1;
    if sibling_key[i] < std::u8::MAX {
        sibling_key[i] += 1;
    } else {
        sibling_key[i] -= 1;
    }
    let mut tree2 = SparseMerkleTree::<Blake2bHasher>::default();
    tree2.update(sibling_key, value).expect("update");
    assert_ne!(tree.root(), tree2.root());
}

fn test_update(key: H256, value: H256) {
    let mut tree = SparseMerkleTree::<Blake2bHasher>::default();
    tree.update(key, value).expect("update");
    assert_eq!(tree.get(&key), Ok(&value));
}

fn test_update_tree_store(key: H256, value: H256, value2: H256) {
    const EXPECTED_LEN: usize = 257;

    let mut tree = SparseMerkleTree::<Blake2bHasher>::default();
    tree.update(key, value).expect("update");
    assert_eq!(tree.store().len(), EXPECTED_LEN);
    tree.update(key, value2).expect("update");
    assert_eq!(tree.store().len(), EXPECTED_LEN);
    assert_eq!(tree.get(&key), Ok(&value2));
}

fn test_merkle_proof(key: H256, value: H256) {
    const EXPECTED_PROOF_SIZE: usize = 16;

    let mut tree = SparseMerkleTree::<Blake2bHasher>::default();
    tree.update(key, value).expect("update");
    let proof = tree.merkle_proof(&key).expect("proof");
    assert!(proof.len() < EXPECTED_PROOF_SIZE);
    assert!(verify_proof::<Blake2bHasher>(proof, tree.root(), &key, &value).expect("verify"));
}

fn random_h256(rng: &mut impl Rng) -> H256 {
    let mut buf = [0u8; 32];
    rng.fill(&mut buf);
    buf
}

fn random_smt(count: usize, rng: &mut impl Rng) -> (SparseMerkleTree<Blake2bHasher>, Vec<H256>) {
    let mut smt = SparseMerkleTree::default();
    let mut keys = Vec::with_capacity(count);
    for _ in 0..count {
        let key = random_h256(rng);
        let value = random_h256(rng);
        smt.update(key, value).unwrap();
        keys.push(key);
    }
    (smt, keys)
}

proptest! {
    #[test]
    fn test_random_update(key: H256, value: H256) {
        test_update(key, value);
    }

    #[test]
    fn test_random_update_tree_store(key: H256, value: H256, value2: H256) {
        test_update_tree_store(key, value, value2);
    }

    #[test]
    fn test_random_construct(key: H256, value: H256) {
        test_construct(key, value);
    }

    #[test]
    fn test_random_merkle_proof(key: H256, value: H256) {
        test_merkle_proof(key, value);
    }

    #[test]
    fn test_smt(rand_count in 2usize..3usize) {
        let mut rng = thread_rng();
        let (smt, keys) = random_smt(rand_count, &mut rng);
        for k in keys {
            let val = smt.get(&k).expect("get value");
            let proof = smt.merkle_proof(&k).expect("gen proof");
            assert!(verify_proof::<Blake2bHasher>(proof, smt.root(), &k, val).expect("verify proof"));
        }
    }

}
