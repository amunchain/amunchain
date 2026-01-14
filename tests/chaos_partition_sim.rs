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

use amunchain_layer0::core::consensus::driver::ConsensusDriver;
use amunchain_layer0::core::types::{ConsensusMsg, Signature, ValidatorId, Vote, H256};
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

#[test]
fn chaos_partition_does_not_panic() {
    let validators = make_validators(7);
    let mut driver_a = ConsensusDriver::new(validators.clone()).unwrap();
    let mut driver_b = ConsensusDriver::new(validators.clone()).unwrap();

    let group1: Vec<ValidatorId> = validators.iter().take(4).cloned().collect();
    let group2: Vec<ValidatorId> = validators.iter().skip(4).cloned().collect();

    let height = 1u64;
    let round = 0u64;
    let h1 = H256::from_bytes([1u8; 32]);
    let h2 = H256::from_bytes([2u8; 32]);

    for v in group1 {
        let vote = Vote {
            height,
            round,
            block_hash: h1,
            voter: v,
            signature: dummy_sig(),
        };
        driver_a.on_msg(ConsensusMsg::Vote(vote));
    }
    for v in group2 {
        let vote = Vote {
            height,
            round,
            block_hash: h2,
            voter: v,
            signature: dummy_sig(),
        };
        driver_b.on_msg(ConsensusMsg::Vote(vote));
    }
}
