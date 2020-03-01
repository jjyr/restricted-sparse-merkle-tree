use crate::{
    error::Error,
    tree::{BranchNode, LeafNode},
    H256,
};

pub trait Hasher {
    fn write_h256(&mut self, h: &H256);
    fn finish(self) -> H256;
}

pub trait Value {
    fn to_h256(&self) -> H256;
    fn zero() -> Self;
}

impl Value for H256 {
    fn to_h256(&self) -> H256 {
        *self
    }
    fn zero() -> Self {
        H256::zero()
    }
}

pub trait Store<V> {
    fn get_branch(&self, node: &H256) -> Result<Option<BranchNode>, Error>;
    fn get_leaf(&self, leaf_hash: &H256) -> Result<Option<LeafNode<V>>, Error>;
    fn insert_branch(&mut self, node: H256, branch: BranchNode) -> Result<(), Error>;
    fn insert_leaf(&mut self, leaf_hash: H256, leaf: LeafNode<V>) -> Result<(), Error>;
    fn remove_branch(&mut self, node: &H256) -> Result<(), Error>;
    fn remove_leaf(&mut self, leaf_hash: &H256) -> Result<(), Error>;
}
