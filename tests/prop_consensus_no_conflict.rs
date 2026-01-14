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

use amunchain_layer0::core::consensus::tide::{NoopSlashing, TideConfig, TideFinalizer};
use amunchain_layer0::core::types::{Commit, Signature, ValidatorId, Vote, H256};
use proptest::prelude::*;
use std::collections::BTreeSet;

fn make_validators(n: usize) -> BTreeSet<ValidatorId> {
    let mut s = BTreeSet::new();
    for i in 0..n {
        let mut b = [0u8; 32];
        b[0] = i as u8;
        s.insert(ValidatorId(b.to_vec()));
    }
    s
}

fn dummy_sig() -> Signature {
    Signature(vec![0u8; 64])
}

fn arb_hash() -> impl Strategy<Value = H256> {
    any::<[u8; 32]>().prop_map(H256::from_bytes)
}

proptest! {
    #[test]
    fn prop_no_two_commits_same_height_round(
        height in 1u64..10u64,
        round in 0u64..5u64,
        h1 in arb_hash(),
        h2 in arb_hash(),
    ) {
        prop_assume!(h1 != h2);

        let validators = make_validators(7); // threshold 5
        let cfg = TideConfig::new(validators.clone());
        let mut tide = TideFinalizer::new(cfg, NoopSlashing);

        let mut commits: Vec<Commit> = Vec::new();

        for v in validators.iter().take(5) {
            let vote = Vote { height, round, block_hash: h1, voter: v.clone(), signature: dummy_sig() };
            if let Ok(Some(c)) = tide.process_vote_inner_for_tests(vote) {
                commits.push(c);
            }
        }

        for v in validators.iter().skip(5) {
            let vote = Vote { height, round, block_hash: h2, voter: v.clone(), signature: dummy_sig() };
            if let Ok(Some(c)) = tide.process_vote_inner_for_tests(vote) {
                commits.push(c);
            }
        }

        if commits.len() > 1 {
            let first = &commits[0];
            for c in commits.iter().skip(1) {
                prop_assert_eq!(c.height, first.height);
                prop_assert_eq!(c.round, first.round);
                prop_assert_eq!(c.block_hash, first.block_hash);
            }
        }
    }
}
