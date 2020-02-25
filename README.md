# Sparse merkle tree

[![Crates.io](https://img.shields.io/crates/v/sparse-merkle-tree.svg)](https://crates.io/crates/sparse-merkle-tree)
[Docs](https://docs.rs/sparse-merkle-tree)

A construction optimized sparse merkle tree.

| size | proof size | update | get | merkle proof | verify proof |
| --- | --- | --- | --- | --- | --- |
| 2n + log(n) | log(n) | log(n) | log(n) | log(n) | log(n) |

Features:

* Generate / Verify multi-leaves merkle proof
* Customize hash function
* Rust `no_std` support

**Notice** this library is not stable yet; API and proof format may changes. Make sure you know what you are doing before using this library.

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

We use a root `H256` and a map `map[(usize, H256)] -> (H256, H256)` to represent a tree, the key of map is parent node and height, values are children nodes, an empty tree represented in an empty map plus a zero `H256` root.

To update a `key` with `value`, we walk down from `root`, push every non-zero sibling into `merkle_path` vector, since the tree height is `N = 256`, we need store 256 siblings. Then we reconstruct the tree from bottom to top: `map[(height, parent)] = merge(lhs, rhs)`, after do 256 times compute we got the new `root`.

A sparse merkle tree contains few efficient nodes, and lot's of zero nodes, we can specialize the `merge` function for zero value. We redefine the `merge` function, only do the actual computing when `lhs` and `rhs` are both non-zero values, otherwise if one of them is zero, we just return another one as the parent.

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

This optimized `merge` function still has one problem, `merge(x, zero)` equals to `merge(zero, x)`, which means the merkle `root` is broken, we can easily construct a conflicted merkle `root` from different leaves.

To fix this, instead of update `key` with an `H256` `value`, we use `hash(key | value)` as the value to merge, so for different keys, no matter what the `value` is, the leaves' hashes are unique. Since all leaves have a unique hash, nodes at each height will either merged by two different hashes or merged by a hash with a zero; for a non-zero parent, either situation we get a unique hash at the parent's height. Until the root, if the tree is empty, we get zero, or if the tree is not empty, the root must merge from two hashes or a hash with a zero, we already proved the root hash is unique.

## License

MIT
