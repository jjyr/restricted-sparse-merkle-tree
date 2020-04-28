use crate::h256::H256;
use crate::traits::Hasher;

/// Merge two hash
/// this function optimized for ZERO_HASH
/// if one of lhs or rhs is ZERO_HASH, this function just return another one
pub fn merge<H: Hasher + Default>(lhs: &H256, rhs: &H256) -> H256 {
    if lhs.is_zero() {
        return *rhs;
    } else if rhs.is_zero() {
        return *lhs;
    }
    let mut hasher = H::default();
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
