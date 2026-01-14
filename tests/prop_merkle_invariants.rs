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

        let kv_pairs: Vec<(Vec<u8>, Vec<u8>)> = pairs.iter().map(|(k,v)| (k.to_be_bytes().to_vec(), v.to_vec())).collect();


        let root = merkle_root_sorted(&kv_pairs);
        let idx = 0usize;

        let proof = merkle_proof_sorted(&kv_pairs, idx).expect("proof exists for non-empty set");
        prop_assert!(verify_proof(root, &proof));
    }
}
