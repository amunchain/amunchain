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


#![no_main]
#![forbid(unsafe_code)]

use arbitrary::Arbitrary;
use libfuzzer_sys::fuzz_target;
use amunchain::core::state::merkle::{merkle_proof_sorted, merkle_root_sorted, verify_proof};

#[derive(Clone, Debug, Arbitrary)]
struct Input {
    pairs: Vec<(Vec<u8>, Vec<u8>)>,
    index: u16,
}

fuzz_target!(|inp: Input| {
    let mut pairs = inp.pairs;
    pairs.sort_by(|a, b| a.0.cmp(&b.0));

    let root = merkle_root_sorted(&pairs);
    if pairs.is_empty() { return; }
    let idx = (inp.index as usize) % pairs.len();

    if let Some(p) = merkle_proof_sorted(&pairs, idx) {
        let _ = verify_proof(root, &p);
    }
});
