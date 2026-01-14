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

use amunchain_layer0::core::state::persistent_state::{KvOp, PersistentState};
use proptest::prelude::*;
use std::collections::BTreeMap;

proptest! {
    #[test]
    fn prop_state_root_deterministic(
        kvs in prop::collection::vec((prop::collection::vec(any::<u8>(), 1..16),
                                      prop::collection::vec(any::<u8>(), 0..32)), 1..50)
    ) {
        let dir1 = tempfile::tempdir().unwrap();
        let dir2 = tempfile::tempdir().unwrap();
        let st1 = PersistentState::open(dir1.path().to_str().unwrap()).unwrap();
        let st2 = PersistentState::open(dir2.path().to_str().unwrap()).unwrap();

        let mut map: BTreeMap<Vec<u8>, Vec<u8>> = BTreeMap::new();
        for (k, v) in kvs.into_iter() { map.insert(k, v); }

        let mut ops = Vec::new();
        for (k, v) in map.iter() {
            ops.push(KvOp::Put { key: k.clone(), value: v.clone() });
        }

        st1.commit_atomic(ops.clone()).unwrap();
        st2.commit_atomic(ops).unwrap();

        let r1 = st1.state_root().unwrap();
        let r2 = st2.state_root().unwrap();

        prop_assert_eq!(r1, r2);
    }
}
