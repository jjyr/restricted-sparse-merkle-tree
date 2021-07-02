# Sparse merkle tree

[![Crates.io](https://img.shields.io/crates/v/restricted-sparse-merkle-tree.svg)](https://crates.io/crates/restricted-sparse-merkle-tree)
[Docs](https://docs.rs/restricted-sparse-merkle-tree)

An optimized sparse merkle tree.

| size | proof size | update | get | merkle proof | verify proof |
| --- | --- | --- | --- | --- | --- |
| 2n + log(n) | log(n) | log(n) | log(n) | log(n) | log(n) |

Features:

* Multi-leaves membership merkle proof
* Customizable hash function
* Rust `no_std` support

This article describes algorithm of this data structure [An optimized compacted sparse merkle tree](https://justjjy.com/An-optimized-compact-sparse-merkle-tree)

**Notice** this library is not stabled yet. The API and the format of the proof may be changed in the future. Make sure you know what you are doing before using this library.

## Known issues

This library do not support **non-membership** proving. We take some aggressive optimizing methods which do not works well with the non-membership proving feature.

Please check this library for [non-membership proving sparse merkle tree](https://github.com/nervosnetwork/sparse-merkle-tree).

## Construction

A sparse merkle tree is a perfectly balanced tree contains `2 ^ N` leaves:

``` txt
# N = 256 sparse merkle tree
height:
255                0
                /     \
254            0        1

.............................

           /   \          /  \
2         0     1        0    1
1        / \   / \      / \   / \
0       0   1 0  1 ... 0   1 0   1 
       0x00..00 0x00..01   ...   0x11..11
```

The above graph demonstrates a sparse merkle tree with `2 ^ 256` leaves, which can mapping every possible `H256` value into leaves. The height of the tree is `256`, from top to bottom, we denote `0` for each left branch and denote `1` for each right branch, so we can get a 256 bits path, which also can represent in `H256`, we use the path as the key of leaves, the most left leaf's key is `0x00..00`, and the next key is `0x00..01`, the most right key is `0x11..11`.

We use a `H256` root and a map `map[(usize, H256)] -> (H256, H256)` to represent the tree, the map's key is node and its height, the map's values are node's children, an empty tree represented in an empty map plus a zero `H256` root.

To update a `key` with `value`, we walk the tree from `root` to `leaf`, push every non-zero sibling into `merkle_path` vector, since the tree height is `N = 256`, the `merkle_path` contains 256 siblings. Then we reconstruct the tree from bottom to top: `map[(height, parent)] = merge(lhs, rhs)`, after do 256 times calculation we got the new `root`.

A sparse merkle tree contains few efficient nodes and others are zeros, we can specialize the `merge` function for zero value. We redefine the `merge` function, only do the actual computing when `lhs` and `rhs` are both non-zero values, otherwise if one of them is zero, we just return another one as the result.

``` rust
fn merge(lhs: H256, rhs: H256) -> H256 {
    if lhs.is_zero() {
        return rhs;
    } else if rhs.is_zero() {
        return lhs;
    }

    // only do actual computing when lhs and rhs both are non-zero
    merge_hash(lhs, rhs)
}
```

This optimized `merge` function still has one issue, `merge(x, zero)` equals to `merge(zero, x)`, which means the merkle `root` is broken since an attacker can easily construct a collision of merkle root.

To fix this, instead of update `key` with an `H256` `value`, we use `hash(key | value)` as the value to merge, so for different keys, no matter what the `value` is, the leaves' hashes are unique. Since all leaves have a unique hash, nodes at each height will either merged by two different hashes or merged by a hash with a zero; for a non-zero parent, either situation we get a unique hash at the parent's height. Until the root, if the tree is empty, we get zero, or if the tree is not empty, the root must be merged from two hashes or a hash with a zero, because of the hash of two children nodes are unique, the root hash is also unique. Thus, an attacker can't construct a collision attack.

## License

MIT
