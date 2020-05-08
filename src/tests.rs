use super::*;
use crate::{
    blake2b::Blake2bHasher, default_store::DefaultStore, error::Error, MerkleProof,
    SparseMerkleTree,
};
use proptest::prelude::*;

type SMT = SparseMerkleTree<Blake2bHasher, H256, DefaultStore<H256>>;

#[test]
fn test_default_root() {
    let mut tree = SMT::default();
    assert_eq!(tree.store().branches_map().len(), 0);
    assert_eq!(tree.store().leaves_map().len(), 0);
    assert_eq!(tree.root(), &H256::zero());

    // insert a key-value
    tree.update(H256::zero(), [42u8; 32].into())
        .expect("update");
    assert_ne!(tree.root(), &H256::zero());
    assert_ne!(tree.store().branches_map().len(), 0);
    assert_ne!(tree.store().leaves_map().len(), 0);
    assert_eq!(tree.get(&H256::zero()).expect("get"), [42u8; 32].into());
    // update zero is to delete the key
    tree.update(H256::zero(), H256::zero()).expect("update");
    assert_eq!(tree.root(), &H256::zero());
    assert_eq!(tree.get(&H256::zero()).expect("get"), H256::zero());
}

#[test]
fn test_default_tree() {
    let tree = SMT::default();
    assert_eq!(tree.get(&H256::zero()).expect("get"), H256::zero());
    let proof = tree.merkle_proof(vec![H256::zero()]).expect("merkle proof");
    let root = proof
        .compute_root::<Blake2bHasher>(vec![(H256::zero(), H256::zero())])
        .expect("root");
    assert_eq!(&root, tree.root());
    let proof = tree.merkle_proof(vec![H256::zero()]).expect("merkle proof");
    let root2 = proof
        .compute_root::<Blake2bHasher>(vec![(H256::zero(), [42u8; 32].into())])
        .expect("root");
    assert_ne!(&root2, tree.root());
}

#[test]
fn test_default_merkle_proof() {
    let proof = MerkleProof::new(Default::default(), Default::default());
    let result = proof.compute_root::<Blake2bHasher>(vec![([42u8; 32].into(), [42u8; 32].into())]);
    assert_eq!(
        result.unwrap_err(),
        Error::IncorrectNumberOfLeaves {
            expected: 0,
            actual: 1
        }
    );
    // makes room for leaves
    let proof = MerkleProof::new(vec![Vec::new()], Default::default());
    let root = proof
        .compute_root::<Blake2bHasher>(vec![([42u8; 32].into(), [42u8; 32].into())])
        .expect("compute root");
    assert_ne!(root, H256::zero());
}

#[test]
fn test_merkle_root() {
    fn new_blake2b() -> blake2b_rs::Blake2b {
        blake2b_rs::Blake2bBuilder::new(32).personal(b"SMT").build()
    }

    let mut tree = SMT::default();
    for (i, word) in "The quick brown fox jumps over the lazy dog"
        .split_whitespace()
        .enumerate()
    {
        let key: H256 = {
            let mut buf = [0u8; 32];
            let mut hasher = new_blake2b();
            hasher.update(&(i as u32).to_le_bytes());
            hasher.finalize(&mut buf);
            buf.into()
        };
        let value: H256 = {
            let mut buf = [0u8; 32];
            let mut hasher = new_blake2b();
            hasher.update(&word.as_bytes());
            hasher.finalize(&mut buf);
            buf.into()
        };
        tree.update(key, value).expect("update");
    }

    let expected_root: H256 = [
        82, 221, 165, 5, 244, 130, 169, 59, 37, 71, 129, 215, 69, 57, 74, 189, 188, 99, 84, 60, 14,
        99, 225, 236, 39, 34, 86, 132, 7, 44, 30, 172,
    ]
    .into();
    assert_eq!(tree.store().leaves_map().len(), 9);
    assert_eq!(tree.root(), &expected_root);
}

fn test_construct(key: H256, value: H256) {
    // insert same value to sibling key will construct a different root

    let mut tree = SMT::default();
    tree.update(key, value.clone()).expect("update");

    let mut sibling_key = key;
    if sibling_key.get_bit(0) {
        sibling_key.clear_bit(0);
    } else {
        sibling_key.set_bit(0);
    }
    let mut tree2 = SMT::default();
    tree2.update(sibling_key, value).expect("update");
    assert_ne!(tree.root(), tree2.root());
}

fn test_update(key: H256, value: H256) {
    let mut tree = SMT::default();
    tree.update(key, value).expect("update");
    assert_eq!(tree.get(&key), Ok(value));
}

fn test_update_tree_store(key: H256, value: H256, value2: H256) {
    const EXPECTED_BRANHCES_LEN: usize = 1;
    const EXPECTED_LEAVES_LEN: usize = 1;

    let mut tree = SMT::default();
    tree.update(key, value).expect("update");
    assert_eq!(tree.store().branches_map().len(), EXPECTED_BRANHCES_LEN);
    assert_eq!(tree.store().leaves_map().len(), EXPECTED_LEAVES_LEN);
    tree.update(key, value2).expect("update");
    assert_eq!(tree.store().branches_map().len(), EXPECTED_BRANHCES_LEN);
    assert_eq!(tree.store().leaves_map().len(), EXPECTED_LEAVES_LEN);
    assert_eq!(tree.get(&key), Ok(value2));
}

fn test_merkle_proof(key: H256, value: H256) {
    const EXPECTED_PROOF_SIZE: usize = 16;

    let mut tree = SMT::default();
    tree.update(key, value).expect("update");
    if !tree.is_empty() {
        let proof = tree.merkle_proof(vec![key]).expect("proof");
        let compiled_proof = proof
            .clone()
            .compile(vec![(key, value)])
            .expect("compile proof");
        assert!(proof.proof().len() < EXPECTED_PROOF_SIZE);
        assert!(proof
            .verify::<Blake2bHasher>(tree.root(), vec![(key, value)])
            .expect("verify"));
        assert!(compiled_proof
            .verify::<Blake2bHasher>(tree.root(), vec![(key, value)])
            .expect("compiled verify"));
    }
}

fn new_smt(pairs: Vec<(H256, H256)>) -> SMT {
    let mut smt = SMT::default();
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
            let compiled_proof = proof.clone().compile(vec![(k, v)]).expect("compile proof");
            assert!(proof.verify::<Blake2bHasher>(smt.root(), vec![(k, v)]).expect("verify proof"));
            assert!(compiled_proof.verify::<Blake2bHasher>(smt.root(), vec![(k, v)]).expect("verify compiled proof"));
        }
    }

    #[test]
    fn test_smt_single_leaf_large((pairs, _n) in leaves(50, 100)){
        let smt = new_smt(pairs.clone());
        for (k, v) in pairs {
            let proof = smt.merkle_proof(vec![k]).expect("gen proof");
            let compiled_proof = proof.clone().compile(vec![(k, v)]).expect("compile proof");
            assert!(proof.verify::<Blake2bHasher>(smt.root(), vec![(k, v)]).expect("verify proof"));
            assert!(compiled_proof.verify::<Blake2bHasher>(smt.root(), vec![(k, v)]).expect("verify compiled proof"));
        }
    }

    #[test]
    fn test_smt_multi_leaves_small((pairs, n) in leaves(1, 50)){
        let smt = new_smt(pairs.clone());
        let proof = smt.merkle_proof(pairs.iter().take(n).map(|(k, _v)| *k).collect()).expect("gen proof");
        let data: Vec<(H256, H256)> = pairs.into_iter().take(n).collect();
        let compiled_proof = proof.clone().compile(data.clone()).expect("compile proof");
        assert!(proof.verify::<Blake2bHasher>(smt.root(), data.clone()).expect("verify proof"));
        assert!(compiled_proof.verify::<Blake2bHasher>(smt.root(), data).expect("verify compiled proof"));
    }

    #[test]
    fn test_smt_multi_leaves_large((pairs, _n) in leaves(50, 100)){
        let n = 20;
        let smt = new_smt(pairs.clone());
        let proof = smt.merkle_proof(pairs.iter().take(n).map(|(k, _v)| *k).collect()).expect("gen proof");
        let data: Vec<(H256, H256)> = pairs.into_iter().take(n).collect();
        let compiled_proof = proof.clone().compile(data.clone()).expect("compile proof");
        assert!(proof.verify::<Blake2bHasher>(smt.root(), data.clone()).expect("verify proof"));
        assert!(compiled_proof.verify::<Blake2bHasher>(smt.root(), data).expect("verify compiled proof"));
    }

    #[test]
    fn test_smt_non_exists_leaves((pairs, _n) in leaves(1, 20), (pairs2, _n2) in leaves(1, 5)){
        let smt = new_smt(pairs);
        let non_exists_keys: Vec<_> = pairs2.into_iter().map(|(k, _v)|k).collect();
        let proof = smt.merkle_proof(non_exists_keys.clone()).expect("gen proof");
        let data: Vec<(H256, H256)> = non_exists_keys.into_iter().map(|k|(k, H256::zero())).collect();
        let compiled_proof = proof.clone().compile(data.clone()).expect("compile proof");
        assert!(proof.verify::<Blake2bHasher>(smt.root(), data.clone()).expect("verify proof"));
        assert!(compiled_proof.verify::<Blake2bHasher>(smt.root(), data).expect("verify compiled proof"));
    }

    #[test]
    fn test_smt_non_exists_leaves_mix((pairs, _n) in leaves(1, 20), (pairs2, _n2) in leaves(1, 5)){
        let smt = new_smt(pairs.clone());
        let exists_keys: Vec<_> = pairs.into_iter().map(|(k, _v)|k).collect();
        let non_exists_keys: Vec<_> = pairs2.into_iter().map(|(k, _v)|k).collect();
        let exists_keys_len = std::cmp::max(exists_keys.len() / 2, 1);
        let non_exists_keys_len = std::cmp::max(non_exists_keys.len() / 2, 1);
        let mut keys: Vec<_> = exists_keys.into_iter().take(exists_keys_len).chain(non_exists_keys.into_iter().take(non_exists_keys_len)).collect();
        keys.dedup();
        let proof = smt.merkle_proof(keys.clone()).expect("gen proof");
        let data: Vec<(H256, H256)> = keys.into_iter().map(|k|(k, smt.get(&k).expect("get"))).collect();
        let compiled_proof = proof.clone().compile(data.clone()).expect("compile proof");
        assert!(proof.verify::<Blake2bHasher>(smt.root(), data.clone()).expect("verify proof"));
        assert!(compiled_proof.verify::<Blake2bHasher>(smt.root(), data).expect("verify compiled proof"));
    }

    #[test]
    fn test_update_smt_tree_store((pairs, n) in leaves(1, 20)) {
        let smt = new_smt(pairs.clone());
        for (k, v) in pairs.into_iter().take(n) {
            assert_eq!(smt.get(&k), Ok(v));
        }
    }

    #[test]
    fn test_clear_smt_tree((pairs, _) in leaves(1, 20)) {
        let mut smt = new_smt(pairs.clone());
        smt.clear().expect("clear");
        assert_eq!(smt.root(), &H256::zero());
    }
}
