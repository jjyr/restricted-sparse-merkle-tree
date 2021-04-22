use crate::traits::Hasher;
use crate::{default_store::DefaultStore, SparseMerkleTree, H256};
use blake2b_rs::{Blake2b, Blake2bBuilder};
use core::ffi::c_void;

#[link(name = "dl-c-impl", kind = "static")]
extern "C" {
    fn smt_state_new(capacity: u32) -> *mut c_void;
    fn smt_state_len(state: *mut c_void) -> u32;

    fn smt_state_insert(state: *mut c_void, key: *const u8, value: *const u8) -> isize;
    fn smt_state_fetch(state: *mut c_void, key: *const u8, value: *mut u8) -> isize;
    fn smt_state_normalize(state: *mut c_void);
    #[allow(dead_code)]
    fn smt_calculate_root(
        buffer: *mut u8,
        state: *const c_void,
        proof: *const u8,
        proof_length: u32,
    ) -> isize;
    fn smt_verify(
        hash: *const u8,
        state: *const c_void,
        proof: *const u8,
        proof_length: u32,
    ) -> isize;
}

pub struct SmtCImpl {
    state_ptr: *mut c_void,
}

fn ffi_smt_result<T>(value: T, code: isize) -> Result<T, isize> {
    if code == 0 {
        Ok(value)
    } else {
        Err(code)
    }
}

fn ffi_assert_slice_len(slice: &[u8], expected_len: usize) -> Result<(), isize> {
    if slice.len() == expected_len {
        Ok(())
    } else {
        Err(-999)
    }
}

impl SmtCImpl {
    pub fn new(capacity: u32) -> SmtCImpl {
        let state_ptr = unsafe { smt_state_new(capacity) };
        SmtCImpl { state_ptr }
    }

    pub fn len(&self) -> u32 {
        unsafe { smt_state_len(self.state_ptr) }
    }

    pub fn insert(&mut self, key: &[u8], value: &[u8]) -> Result<(), isize> {
        ffi_assert_slice_len(key, 32)?;
        ffi_assert_slice_len(value, 32)?;
        let code = unsafe { smt_state_insert(self.state_ptr, key.as_ptr(), value.as_ptr()) };
        ffi_smt_result((), code)
    }

    pub fn fetch(&self, key: &[u8]) -> Result<[u8; 32], isize> {
        ffi_assert_slice_len(key, 32)?;
        let mut value = [0u8; 32];
        let code = unsafe { smt_state_fetch(self.state_ptr, key.as_ptr(), value.as_mut_ptr()) };
        ffi_smt_result(value, code)
    }

    pub fn normalize(&mut self) {
        unsafe {
            smt_state_normalize(self.state_ptr);
        }
    }

    #[allow(dead_code)]
    pub fn calculate_root(&self, proof: &[u8]) -> Result<[u8; 32], isize> {
        let mut hash = [0u8; 32];
        let code = unsafe {
            smt_calculate_root(
                hash.as_mut_ptr(),
                self.state_ptr,
                proof.as_ptr(),
                proof.len() as u32,
            )
        };
        ffi_smt_result(hash, code)
    }

    pub fn verify(&self, root: &[u8], proof: &[u8]) -> Result<(), isize> {
        ffi_assert_slice_len(root, 32)?;
        let code = unsafe {
            smt_verify(
                root.as_ptr(),
                self.state_ptr,
                proof.as_ptr(),
                proof.len() as u32,
            )
        };
        ffi_smt_result((), code)
    }
}

pub struct CkbBlake2bHasher(Blake2b);

impl Default for CkbBlake2bHasher {
    fn default() -> Self {
        let blake2b = Blake2bBuilder::new(32)
            .personal(b"ckb-default-hash")
            .build();
        CkbBlake2bHasher(blake2b)
    }
}

impl Hasher for CkbBlake2bHasher {
    fn write_h256(&mut self, h: &H256) {
        self.0.update(h.as_slice());
    }
    fn finish(self) -> H256 {
        let mut hash = [0u8; 32];
        self.0.finalize(&mut hash);
        hash.into()
    }
}

pub type CkbSMT = SparseMerkleTree<CkbBlake2bHasher, H256, DefaultStore<H256>>;

pub fn new_ckb_smt(pairs: Vec<(H256, H256)>) -> CkbSMT {
    let mut smt = CkbSMT::default();
    for (key, value) in pairs {
        smt.update(key, value).unwrap();
    }
    smt
}
