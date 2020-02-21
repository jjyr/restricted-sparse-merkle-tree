use core::cmp::Ordering;
#[derive(Eq, PartialEq, Debug, Default, Hash, Clone, Copy)]
pub struct H256([u8; 32]);

const ZERO: H256 = H256([0u8; 32]);
const BYTE_SIZE: u8 = 8;

impl H256 {
    pub const fn zero() -> Self {
        ZERO
    }

    pub fn is_zero(&self) -> bool {
        self == &ZERO
    }

    pub fn get_bit(&self, i: u8) -> bool {
        let byte_pos = i / BYTE_SIZE;
        let bit_pos = i % BYTE_SIZE;
        let bit = self.0[byte_pos as usize] >> bit_pos & 1;
        bit != 0
    }

    pub fn set_bit(&mut self, i: u8) {
        let byte_pos = i / BYTE_SIZE;
        let bit_pos = i % BYTE_SIZE;
        self.0[byte_pos as usize] |= 1 << bit_pos as u8;
    }

    pub fn clear_bit(&mut self, i: u8) {
        let byte_pos = i / BYTE_SIZE;
        let bit_pos = i % BYTE_SIZE;
        self.0[byte_pos as usize] &= !((1 << bit_pos) as u8);
    }

    pub fn as_slice(&self) -> &[u8] {
        &self.0[..]
    }

    // TODO optimize
    pub fn copy_bits(&self, range: impl core::ops::RangeBounds<usize>) -> Self {
        const MAX: usize = 256;
        use core::ops::Bound;

        let mut target = H256::zero();
        let start = match range.start_bound() {
            Bound::Included(&i) => i,
            Bound::Excluded(&i) => panic!("do not allows excluded start: {}", i),
            Bound::Unbounded => 0,
        };

        let end = match range.end_bound() {
            Bound::Included(&i) => i.saturating_add(1),
            Bound::Excluded(&i) => i,
            Bound::Unbounded => core::cmp::max(MAX, start),
        };

        if end < start {
            panic!("end can't less than start: start {} end {}", start, end);
        }

        for i in start..end {
            if self.get_bit(i as u8) {
                target.set_bit(i as u8)
            }
        }
        target
    }
}

impl PartialOrd for H256 {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        // H256 is little endian
        for i in (0..self.0.len()).rev() {
            let o = self.0[i].partial_cmp(&other.0[i]);
            if o != Some(Ordering::Equal) {
                return o;
            }
        }
        Some(Ordering::Equal)
    }
}

impl Ord for H256 {
    fn cmp(&self, other: &Self) -> Ordering {
        self.partial_cmp(other).expect("partial cmp")
    }
}

impl From<[u8; 32]> for H256 {
    fn from(v: [u8; 32]) -> H256 {
        H256(v)
    }
}

impl Into<[u8; 32]> for H256 {
    fn into(self: H256) -> [u8; 32] {
        self.0
    }
}
