// Copyright (c) 2026 Amunchain
// Licensed under the Apache License, Version 2.0

#![forbid(unsafe_code)]

use proptest::prelude::*;

use amunchain::core::state::merkle::{merkle_proof_sorted, merkle_root_sorted, verify_proof};

proptest! {
    #[test]
    fn merkle_proof_verifies_for_any_nonempty_set(mut pairs in proptest::collection::vec((any::<u64>(), any::<[u8;32]>()), 1..64)) {
        // Canonical ordering requirement
        pairs.sort_by(|a,b| a.0.cmp(&b.0));

        let root = merkle_root_sorted(&pairs);
        let idx = (pairs[0].0 as usize) % pairs.len();

        let proof = merkle_proof_sorted(&pairs, idx).expect("proof exists for non-empty set");
        prop_assert!(verify_proof(root, &proof));
    }
}
