use crate::h256::H256;
use crate::traits::Hasher;

/// Merge two hash with node information
/// this function optimized for ZERO_HASH
/// if lhs and rhs both are ZERO_HASH return ZERO_HASH, otherwise hash all info.
pub fn merge<H: Hasher + Default>(height: u8, node_key: &H256, lhs: &H256, rhs: &H256) -> H256 {
    if lhs.is_zero() && rhs.is_zero() {
        return H256::zero();
    }
    let mut hasher = H::default();
    hasher.write_byte(height);
    hasher.write_h256(node_key);
    hasher.write_h256(lhs);
    hasher.write_h256(rhs);
    hasher.finish()
}

/// hash_leaf = hash(key | value)
/// zero value represent delete the key, this function return zero for zero value
pub fn hash_leaf<H: Hasher + Default>(key: &H256, value: &H256) -> H256 {
    if value.is_zero() {
        return H256::zero();
    }
    let mut hasher = H::default();
    hasher.write_h256(key);
    hasher.write_h256(value);
    hasher.finish()
}
