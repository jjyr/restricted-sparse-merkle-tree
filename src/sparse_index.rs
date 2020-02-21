use crate::{tree::EXPECTED_PATH_SIZE, vec::Vec, H256};

pub const MAX_LEN: usize = 256;

pub struct SparseIndex {
    // a index mark the non-zero index
    sparse_index: H256,
    // a vector stores non-zero elements
    buf: Vec<H256>,
    len: usize,
}

impl Default for SparseIndex {
    fn default() -> Self {
        SparseIndex {
            sparse_index: H256::zero(),
            buf: Vec::with_capacity(EXPECTED_PATH_SIZE),
            len: 0,
        }
    }
}

impl SparseIndex {
    /// values must push in order by index (1 -> 2 -> 3), otherwise the behavior is undefined
    pub fn push(&mut self, v: H256) {
        assert!(self.len < MAX_LEN, "too many elements");
        if !v.is_zero() {
            self.sparse_index.set_bit(self.len as u8);
            self.buf.push(v)
        }
        self.len += 1;
    }

    pub fn set_len(&mut self, len: usize) {
        self.len = len;
    }

    pub fn pop(&mut self) -> Option<H256> {
        self.len -= 1;
        if self.sparse_index.get_bit(self.len as u8) {
            return self.buf.pop();
        }
        None
    }
}
