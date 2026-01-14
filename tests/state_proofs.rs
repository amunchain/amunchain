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

#[test]
fn test_state_root_and_proof() {
    let dir = tempfile::tempdir().unwrap();
    let st = PersistentState::open(dir.path().to_str().unwrap()).unwrap();

    st.commit_atomic(vec![
        KvOp::Put {
            key: b"a".to_vec(),
            value: b"1".to_vec(),
        },
        KvOp::Put {
            key: b"b".to_vec(),
            value: b"2".to_vec(),
        },
        KvOp::Put {
            key: b"c".to_vec(),
            value: b"3".to_vec(),
        },
    ])
    .unwrap();

    let root = st.state_root().unwrap();
    let got = st.prove_key(b"b").unwrap().unwrap();
    let (_k, _v, root2, proof) = got;
    assert_eq!(root, root2);
    assert!(PersistentState::verify_proof(root, &proof));
}
