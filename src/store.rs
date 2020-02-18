use crate::{collections, tree::Node, H256};

cfg_if::cfg_if! {
    if #[cfg(feature = "std")] {
        pub type Store = collections::HashMap<(usize, H256), Node>;
    } else {
        pub type Store = collections::BTreeMap<(usize, H256), Node>;
    }
}
