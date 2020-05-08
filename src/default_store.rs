use crate::{
    collections,
    error::Error,
    traits::Store,
    tree::{BranchNode, LeafNode},
    H256,
};

#[derive(Debug, Clone, Default)]
pub struct DefaultStore<V> {
    branches_map: Map<H256, BranchNode>,
    leaves_map: Map<H256, LeafNode<V>>,
}

impl<V> DefaultStore<V> {
    pub fn branches_map(&self) -> &Map<H256, BranchNode> {
        &self.branches_map
    }
    pub fn leaves_map(&self) -> &Map<H256, LeafNode<V>> {
        &self.leaves_map
    }
}

impl<V: Clone> Store<V> for DefaultStore<V> {
    fn leaf_iter<'a>(&'a self) -> Result<Box<dyn Iterator<Item = &LeafNode<V>> + 'a>, Error> {
        Ok(Box::new(self.leaves_map.values()))
    }
    fn get_branch(&self, node: &H256) -> Result<Option<BranchNode>, Error> {
        Ok(self.branches_map.get(node).map(Clone::clone))
    }
    fn get_leaf(&self, leaf_hash: &H256) -> Result<Option<LeafNode<V>>, Error> {
        Ok(self.leaves_map.get(leaf_hash).map(Clone::clone))
    }
    fn insert_branch(&mut self, node: H256, branch: BranchNode) -> Result<(), Error> {
        self.branches_map.insert(node, branch);
        Ok(())
    }
    fn insert_leaf(&mut self, leaf_hash: H256, leaf: LeafNode<V>) -> Result<(), Error> {
        self.leaves_map.insert(leaf_hash, leaf);
        Ok(())
    }
    fn remove_branch(&mut self, node: &H256) -> Result<(), Error> {
        self.branches_map.remove(node);
        Ok(())
    }
    fn remove_leaf(&mut self, leaf_hash: &H256) -> Result<(), Error> {
        self.leaves_map.remove(leaf_hash);
        Ok(())
    }
}

cfg_if::cfg_if! {
    if #[cfg(feature = "std")] {
        pub type Map<K, V> = collections::HashMap<K, V>;
        pub type Entry<'a, K, V> = collections::hash_map::Entry<'a, K, V>;
    } else {
        pub type Map<K, V> = collections::BTreeMap<K, V>;
        pub type Entry<'a, K, V> = collections::btree_map::Entry<'a, K, V>;
    }
}
