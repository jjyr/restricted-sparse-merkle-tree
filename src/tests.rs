use super::*;
use crate::{
    blake2b::Blake2bHasher,
    error::Error,
    tree::{hash_leaf, SparseMerkleTree},
};
use proptest::prelude::*;

#[test]
fn test_default_root() {
    let mut tree = SparseMerkleTree::<Blake2bHasher>::default();
    assert_eq!(tree.store().len(), 0);
    assert_eq!(tree.root(), &H256::zero());

    // should not equals to zero even leaf value is zero
    tree.update(H256::zero(), H256::zero()).expect("update");
    assert_ne!(tree.root(), &H256::zero());
    assert_eq!(tree.get(&H256::zero()).expect("get"), &H256::zero());
}

#[test]
fn test_default_merkle_proof() {
    let tree = SparseMerkleTree::<Blake2bHasher>::default();
    let proof_result = tree.merkle_proof(vec![H256::zero()]);
    assert_eq!(proof_result.unwrap_err(), Error::EmptyTree);
    // hash_leaf is not equals to zero
    let zero_leaf = hash_leaf::<Blake2bHasher>(&H256::zero(), &H256::zero());
    assert_ne!(zero_leaf, H256::zero());
    // one leaf tree
    let mut tree = SparseMerkleTree::<Blake2bHasher>::default();
    tree.update(H256::zero(), H256::zero()).expect("update");
    assert_eq!(tree.root(), &zero_leaf);
}

fn test_construct(key: H256, value: H256) {
    // insert same value to sibling key will construct a different root

    let mut tree = SparseMerkleTree::<Blake2bHasher>::default();
    tree.update(key, value.clone()).expect("update");

    let mut sibling_key = key;
    if sibling_key.get_bit(0) {
        sibling_key.clear_bit(0);
    } else {
        sibling_key.set_bit(0);
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
    let proof = tree.merkle_proof(vec![key]).expect("proof");
    assert!(proof.proof().len() < EXPECTED_PROOF_SIZE);
    assert!(proof
        .verify::<Blake2bHasher>(tree.root(), vec![(key, value)])
        .expect("verify"));
}

fn new_smt(pairs: Vec<(H256, H256)>) -> SparseMerkleTree<Blake2bHasher> {
    let mut smt = SparseMerkleTree::default();
    for (key, value) in pairs {
        smt.update(key, value).unwrap();
    }
    smt
}

fn leaves(
    min_leaves: usize,
    max_leaves: usize,
) -> impl Strategy<Value = (Vec<(H256, H256)>, usize)> {
    prop::collection::vec(
        prop::array::uniform2(prop::array::uniform32(0u8..)),
        min_leaves..=max_leaves,
    )
    .prop_flat_map(|mut pairs| {
        pairs.dedup_by_key(|[k, _v]| *k);
        let len = pairs.len();
        (
            Just(
                pairs
                    .into_iter()
                    .map(|[k, v]| (k.into(), v.into()))
                    .collect(),
            ),
            core::cmp::min(1, len)..=len,
        )
    })
}

proptest! {
    #[test]
    fn test_h256(key: [u8; 32], key2: [u8; 32]) {
        let mut list1: Vec<H256> = vec![key.into() , key2.into()];
        let mut list2 = list1.clone();
        // sort H256
        list1.sort_unstable_by_key(|k| *k);
        // sort by high bits to lower bits
        list2.sort_unstable_by(|k1, k2| {
            for i in (0u8..=255).rev() {
                let b1 = if k1.get_bit(i) { 1 } else { 0 };
                let b2 = if k2.get_bit(i) { 1 } else { 0 };
                let o = b1.cmp(&b2);
                if o != std::cmp::Ordering::Equal {
                    return o;
                }
            }
            std::cmp::Ordering::Equal
        });
        assert_eq!(list1, list2);
    }

    #[test]
    fn test_h256_copy_bits(start in 0u8..254u8, size in 1u8..255u8) {
        let one: H256 = [255u8; 32].into();
        let target = one.copy_bits(start..(start.saturating_add(size)));
        for i in start..start.saturating_add(size) {
            assert_eq!(one.get_bit(i as u8), target.get_bit(i as u8));
        }
        for i in 0..start {
            assert!(!target.get_bit(i as u8));
        }
        if let Some(start_i) = start.checked_add(size).and_then(|i| i.checked_add(1)){
            for i in start_i..=255 {
                assert!(!target.get_bit(i as u8));
            }
        }
    }

    #[test]
    fn test_random_update(key: [u8; 32], value: [u8;32]) {
        test_update(key.into(), value.into());
    }

    #[test]
    fn test_random_update_tree_store(key: [u8;32], value: [u8;32], value2: [u8;32]) {
        test_update_tree_store(key.into(), value.into(), value2.into());
    }

    #[test]
    fn test_random_construct(key: [u8;32], value: [u8;32]) {
        test_construct(key.into(), value.into());
    }

    #[test]
    fn test_random_merkle_proof(key: [u8; 32], value: [u8;32]) {
        test_merkle_proof(key.into(), value.into());
    }

    #[test]
    fn test_smt_single_leaf_small((pairs, _n) in leaves(1, 50)){
        let smt = new_smt(pairs.clone());
        for (k, v) in pairs {
            let proof = smt.merkle_proof(vec![k]).expect("gen proof");
            assert!(proof.verify::<Blake2bHasher>(smt.root(), vec![(k, v)]).expect("verify proof"));
        }
    }

    #[test]
    fn test_smt_single_leaf_large((pairs, _n) in leaves(50, 100)){
        let smt = new_smt(pairs.clone());
        for (k, v) in pairs {
            let proof = smt.merkle_proof(vec![k]).expect("gen proof");
            assert!(proof.verify::<Blake2bHasher>(smt.root(), vec![(k, v)]).expect("verify proof"));
        }
    }

    #[test]
    fn test_smt_multi_leaves_small((pairs, n) in leaves(1, 50)){
        let smt = new_smt(pairs.clone());
        let proof = smt.merkle_proof(pairs.iter().take(n).map(|(k, _v)| *k).collect()).expect("gen proof");
        assert!(proof.verify::<Blake2bHasher>(smt.root(), pairs.into_iter().take(n).collect()).expect("verify proof"));
    }

    #[test]
    fn test_smt_multi_leaves_large((pairs, _n) in leaves(50, 100)){
        let n = 20;
        let smt = new_smt(pairs.clone());
        let proof = smt.merkle_proof(pairs.iter().take(n).map(|(k, _v)| *k).collect()).expect("gen proof");
        assert!(proof.verify::<Blake2bHasher>(smt.root(), pairs.into_iter().take(n).collect()).expect("verify proof"));
    }
}
