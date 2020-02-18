use crate::{get_bit, set_bit, EXPECTED_PATH_SIZE, H256, ZERO_HASH};

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
            sparse_index: ZERO_HASH,
            buf: Vec::with_capacity(EXPECTED_PATH_SIZE),
            len: 0,
        }
    }
}

impl SparseIndex {
    /// values must push in order by index (1 -> 2 -> 3), otherwise the behavior is undefined
    pub fn push(&mut self, v: H256) {
        assert!(self.len < MAX_LEN, "too many elements");
        if v != ZERO_HASH {
            set_bit(&mut self.sparse_index, self.len);
            self.buf.push(v)
        }
        self.len += 1;
    }

    pub fn len(&self) -> usize {
        self.len
    }

    pub fn set_len(&mut self, len: usize) {
        self.len = len;
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    pub fn buf(&self) -> &Vec<H256> {
        &self.buf
    }

    pub fn sparse_index(&self) -> H256 {
        self.sparse_index
    }

    pub fn pop(&mut self) -> Option<H256> {
        self.len -= 1;
        if get_bit(&self.sparse_index, self.len) {
            return self.buf.pop();
        }
        None
    }

    pub fn into_vec(self) -> Vec<H256> {
        let SparseIndex {
            mut buf,
            sparse_index,
            ..
        } = self;
        buf.push(sparse_index);
        buf
    }
    pub fn from_vec(mut buf: Vec<H256>, len: usize) -> Option<Self> {
        if buf.len() > (MAX_LEN + 1) || len > MAX_LEN {
            return None;
        }
        let sparse_index = match buf.pop() {
            Some(i) => i,
            None => return None,
        };
        Some(SparseIndex {
            buf,
            sparse_index,
            len,
        })
    }
}
