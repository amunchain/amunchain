// Copyright (c) 2026 Amunchain
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//     http://www.apache.org/licenses/LICENSE-2.0
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

//! Deterministic Merkle tree for state proofs.
//!
//! leaf = H( "Amunchain-State-Leaf-v1" || H(key) || H(value) )
//! node = H( "Amunchain-State-Node-v1" || left || right )

use ring::digest;

/// Hash32 type.
pub type Hash32 = [u8; 32];

const LEAF_DOMAIN: &[u8] = b"Amunchain-State-Leaf-v1";
const NODE_DOMAIN: &[u8] = b"Amunchain-State-Node-v1";

/// Side of sibling in proof.
#[derive(Clone, Debug)]
pub enum Side {
    /// Sibling is left.
    Left,
    /// Sibling is right.
    Right,
}

/// One proof item.
#[derive(Clone, Debug)]
pub struct ProofItem {
    /// Whether sibling is left or right of current hash.
    pub side: Side,
    /// Sibling hash.
    pub sibling: Hash32,
}

/// Merkle inclusion proof.
#[derive(Clone, Debug)]
pub struct MerkleProof {
    /// Leaf hash.
    pub leaf: Hash32,
    /// Path items from leaf to root.
    pub path: Vec<ProofItem>,
}

fn h(data: &[u8]) -> Hash32 {
    let d = digest::digest(&digest::SHA256, data);
    let mut out = [0u8; 32];
    out.copy_from_slice(d.as_ref());
    out
}

fn hash_leaf(key: &[u8], value: &[u8]) -> Hash32 {
    let hk = h(key);
    let hv = h(value);
    let mut buf = Vec::with_capacity(LEAF_DOMAIN.len() + 32 + 32);
    buf.extend_from_slice(LEAF_DOMAIN);
    buf.extend_from_slice(&hk);
    buf.extend_from_slice(&hv);
    h(&buf)
}

fn hash_node(left: Hash32, right: Hash32) -> Hash32 {
    let mut buf = Vec::with_capacity(NODE_DOMAIN.len() + 32 + 32);
    buf.extend_from_slice(NODE_DOMAIN);
    buf.extend_from_slice(&left);
    buf.extend_from_slice(&right);
    h(&buf)
}

/// Build Merkle root from sorted (key,value) pairs.
/// If empty, returns ZERO hash.
pub fn merkle_root_sorted(pairs: &[(Vec<u8>, Vec<u8>)]) -> Hash32 {
    if pairs.is_empty() {
        return [0u8; 32];
    }
    let mut level: Vec<Hash32> = pairs.iter().map(|(k, v)| hash_leaf(k, v)).collect();
    while level.len() > 1 {
        let mut next: Vec<Hash32> = Vec::with_capacity((level.len() + 1) / 2);
        let mut i = 0usize;
        while i < level.len() {
            let left = level[i];
            let right = if i + 1 < level.len() {
                level[i + 1]
            } else {
                level[i]
            };
            next.push(hash_node(left, right));
            i += 2;
        }
        level = next;
    }
    level[0]
}

/// Generate inclusion proof for a pair at `index` (pairs must be sorted).
pub fn merkle_proof_sorted(pairs: &[(Vec<u8>, Vec<u8>)], index: usize) -> Option<MerkleProof> {
    if pairs.is_empty() || index >= pairs.len() {
        return None;
    }

    let mut level: Vec<Hash32> = pairs.iter().map(|(k, v)| hash_leaf(k, v)).collect();
    let mut idx = index;
    let leaf = level[idx];
    let mut path: Vec<ProofItem> = Vec::new();

    while level.len() > 1 {
        let is_right = (idx % 2) == 1;
        let sib_idx = if is_right { idx - 1 } else { idx + 1 };

        let sibling = if sib_idx < level.len() {
            level[sib_idx]
        } else {
            level[idx]
        };

        path.push(ProofItem {
            side: if is_right { Side::Left } else { Side::Right },
            sibling,
        });

        let mut next: Vec<Hash32> = Vec::with_capacity((level.len() + 1) / 2);
        let mut i = 0usize;
        while i < level.len() {
            let left = level[i];
            let right = if i + 1 < level.len() {
                level[i + 1]
            } else {
                level[i]
            };
            next.push(hash_node(left, right));
            i += 2;
        }

        level = next;
        idx /= 2;
    }

    Some(MerkleProof { leaf, path })
}

/// Verify proof against root.
pub fn verify_proof(root: Hash32, proof: &MerkleProof) -> bool {
    let mut cur = proof.leaf;
    for item in proof.path.iter() {
        cur = match item.side {
            Side::Left => hash_node(item.sibling, cur),
            Side::Right => hash_node(cur, item.sibling),
        };
    }
    cur == root
}
