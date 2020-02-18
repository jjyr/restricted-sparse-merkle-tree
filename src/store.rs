use crate::{Node, H256};
use std::collections::HashMap;

pub type Store = HashMap<(usize, H256), Node>;
