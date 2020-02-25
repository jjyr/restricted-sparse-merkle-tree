use crate::{
    collections,
    tree::{Key, Node},
};

cfg_if::cfg_if! {
    if #[cfg(feature = "std")] {
        pub type Store = collections::HashMap<Key, Node>;
        pub type Entry<'a, K, V> = collections::hash_map::Entry<'a, K, V>;
    } else {
        pub type Store = collections::BTreeMap<Key, Node>;
        pub type Entry<'a, K, V> = collections::btree_map::Entry<'a, K, V>;
    }
}
