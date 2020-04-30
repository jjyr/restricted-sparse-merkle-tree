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

    #[inline]
    pub fn get_bit(&self, i: u8) -> bool {
        let byte_pos = i / BYTE_SIZE;
        let bit_pos = i % BYTE_SIZE;
        let bit = self.0[byte_pos as usize] >> bit_pos & 1;
        bit != 0
    }

    #[inline]
    pub fn set_bit(&mut self, i: u8) {
        let byte_pos = i / BYTE_SIZE;
        let bit_pos = i % BYTE_SIZE;
        self.0[byte_pos as usize] |= 1 << bit_pos as u8;
    }

    #[inline]
    pub fn clear_bit(&mut self, i: u8) {
        let byte_pos = i / BYTE_SIZE;
        let bit_pos = i % BYTE_SIZE;
        self.0[byte_pos as usize] &= !((1 << bit_pos) as u8);
    }

    pub fn as_slice(&self) -> &[u8] {
        &self.0[..]
    }

    pub fn fork_height(&self, key: &H256) -> u8 {
        for h in (0..=core::u8::MAX).rev() {
            if self.get_bit(h) != key.get_bit(h) {
                return h;
            }
        }
        0
    }

    pub fn parent_path(&self, height: u8) -> Self {
        height
            .checked_add(1)
            .map(|i| self.copy_bits(i..))
            .unwrap_or_else(H256::zero)
    }

    /// Copy bits to a new H256
    pub fn copy_bits(&self, range: impl core::ops::RangeBounds<u8>) -> Self {
        const MAX: usize = 256;
        const BYTE: usize = 8;
        use core::ops::Bound;

        let mut target = H256::zero();
        let start = match range.start_bound() {
            Bound::Included(&i) => i as usize,
            Bound::Excluded(&i) => panic!("do not allows excluded start: {}", i),
            Bound::Unbounded => 0,
        };

        let mut end = match range.end_bound() {
            Bound::Included(&i) => i.saturating_add(1) as usize,
            Bound::Excluded(&i) => i as usize,
            Bound::Unbounded => MAX,
        };

        if start >= MAX {
            return target;
        } else if end > MAX {
            end = MAX;
        }

        if end < start {
            panic!("end can't less than start: start {} end {}", start, end);
        }

        let start_byte = {
            let remain = if start % BYTE != 0 { 1 } else { 0 };
            start / BYTE + remain
        };
        let end_byte = end / BYTE;
        // copy bytes
        if start_byte < self.0.len() && start_byte <= end_byte {
            target.0[start_byte..end_byte].copy_from_slice(&self.0[start_byte..end_byte]);
        }

        // copy remain bits
        for i in (start..core::cmp::min(start_byte * BYTE, end))
            .chain(core::cmp::max(end_byte * BYTE, start)..end)
        {
            if self.get_bit(i as u8) {
                target.set_bit(i as u8)
            }
        }
        target
    }
}

impl PartialOrd for H256 {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for H256 {
    fn cmp(&self, other: &Self) -> Ordering {
        // Compare bits from heigher to lower (255..0)
        self.0.iter().rev().cmp(other.0.iter().rev())
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
