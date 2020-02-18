#[macro_use]
extern crate criterion;

use criterion::Criterion;
use rand::{thread_rng, Rng};
use sparse_merkle_tree::{verify_proof, SparseMerkleTree, H256};

fn random_h256(rng: &mut impl Rng) -> H256 {
    let mut buf = [0u8; 32];
    rng.fill(&mut buf);
    buf
}

fn random_smt(update_count: usize, rng: &mut impl Rng) -> (SparseMerkleTree, Vec<H256>) {
    let mut smt = SparseMerkleTree::default();
    let mut keys = Vec::with_capacity(update_count);
    for _ in 0..update_count {
        let key = random_h256(rng);
        let value = random_h256(rng);
        smt.update(key, value).unwrap();
        keys.push(key);
    }
    (smt, keys)
}

fn bench(c: &mut Criterion) {
    c.bench_function_over_inputs(
        "SMT update",
        |b, &&size| {
            b.iter(|| {
                let mut rng = thread_rng();
                random_smt(size, &mut rng)
            });
        },
        &[100, 10_000],
    );

    c.bench_function_over_inputs(
        "SMT get",
        |b, &&size| {
            let mut rng = thread_rng();
            let (smt, _keys) = random_smt(size, &mut rng);
            b.iter(|| {
                let key = random_h256(&mut rng);
                smt.get(&key).unwrap();
            });
        },
        &[5_000, 10_000],
    );

    c.bench_function("SMT generate merkle proof", |b| {
        let mut rng = thread_rng();
        let (smt, _keys) = random_smt(10_000, &mut rng);
        b.iter(|| {
            let key = random_h256(&mut rng);
            smt.merkle_proof(&key).unwrap();
        });
    });

    c.bench_function("SMT verify merkle proof", |b| {
        let mut rng = thread_rng();
        let (smt, keys) = random_smt(10_000, &mut rng);
        let value = smt.get(&keys[0]).unwrap();
        let proof = smt.merkle_proof(&keys[0]).unwrap();
        let root = smt.root();
        b.iter(|| {
            let valid = verify_proof(proof.clone(), root, &keys[0], value);
            assert!(valid.expect("verify result"));
        });
    });
}

criterion_group!(
    name = benches;
    config = Criterion::default().sample_size(10);
    targets = bench
);
criterion_main!(benches);
