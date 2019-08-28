use blake2b_rs::{Blake2b, Blake2bBuilder};

const BLAKE2B_KEY: &[u8] = &[];
const BLAKE2B_LEN: usize = 32;
const PERSONALIZATION: &[u8] = b"sparsemerkletree";

pub fn new_blake2b() -> Blake2b {
    Blake2bBuilder::new(BLAKE2B_LEN)
        .personal(PERSONALIZATION)
        .key(BLAKE2B_KEY)
        .build()
}
