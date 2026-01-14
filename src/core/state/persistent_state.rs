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

//! Persistent key-value state using sled, with deterministic Merkle roots and inclusion proofs.

use crate::core::state::merkle::{
    merkle_proof_sorted, merkle_root_sorted, verify_proof, Hash32, MerkleProof,
};
use sled::transaction::ConflictableTransactionError;
use thiserror::Error;

/// State errors.
#[derive(Debug, Error)]
pub enum StateError {
    #[error("db open")]
    DbOpen,
    #[error("db io")]
    DbIo,
    #[error("tx conflict")]
    TxConflict,
}

/// State operation.
#[derive(Clone, Debug)]
pub enum KvOp {
    /// Put key/value.
    Put { key: Vec<u8>, value: Vec<u8> },
    /// Delete key.
    Del { key: Vec<u8> },
}

/// Persistent state wrapper.
#[derive(Clone)]
pub struct PersistentState {
    db: sled::Db,
}

impl PersistentState {
    /// Open sled DB at path (directory).
    pub fn open(path: &str) -> Result<Self, StateError> {
        let db = sled::open(path).map_err(|_| StateError::DbOpen)?;
        Ok(Self { db })
    }

    /// Get value.
    pub fn get(&self, key: &[u8]) -> Result<Option<Vec<u8>>, StateError> {
        let v = self.db.get(key).map_err(|_| StateError::DbIo)?;
        Ok(v.map(|iv| iv.to_vec()))
    }

    /// Atomic commit using sled transactions.
    pub fn commit_atomic(&self, ops: Vec<KvOp>) -> Result<(), StateError> {
        let tree = &self.db;
        let res: Result<(), ConflictableTransactionError<StateError>> = (|| {
            tree.transaction(|t| {
                for op in ops.iter() {
                    match op {
                        KvOp::Put { key, value } => {
                            t.insert(key.as_slice(), value.as_slice()).map_err(|_| {
                                ConflictableTransactionError::Abort(StateError::DbIo)
                            })?;
                        }
                        KvOp::Del { key } => {
                            t.remove(key.as_slice()).map_err(|_| {
                                ConflictableTransactionError::Abort(StateError::DbIo)
                            })?;
                        }
                    }
                }
                Ok(())
            })
            .map_err(|e| match e {
                sled::transaction::TransactionError::Abort(se) => {
                    ConflictableTransactionError::Abort(se)
                }
                sled::transaction::TransactionError::Storage(_) => {
                    ConflictableTransactionError::Abort(StateError::DbIo)
                }
            })
        })();

        match res {
            Ok(()) => Ok(()),
            Err(ConflictableTransactionError::Abort(StateError::TxConflict)) => {
                Err(StateError::TxConflict)
            }
            Err(ConflictableTransactionError::Abort(e)) => Err(e),
            Err(ConflictableTransactionError::Conflict) => Err(StateError::TxConflict),
            Err(ConflictableTransactionError::Storage(_)) => Err(StateError::DbIo),
        }
    }

    /// Deterministic Merkle root over all KV pairs in DB.
    pub fn state_root(&self) -> Result<Hash32, StateError> {
        let mut pairs: Vec<(Vec<u8>, Vec<u8>)> = Vec::new();
        for item in self.db.iter() {
            let kv = item.map_err(|_| StateError::DbIo)?;
            pairs.push((kv.0.to_vec(), kv.1.to_vec()));
        }
        pairs.sort_by(|a, b| a.0.cmp(&b.0));
        Ok(merkle_root_sorted(&pairs))
    }

    /// Produce an inclusion proof for a key, if it exists.
    pub fn prove_key(
        &self,
        key: &[u8],
    ) -> Result<Option<(Vec<u8>, Vec<u8>, Hash32, MerkleProof)>, StateError> {
        let v = self.get(key)?;
        let Some(_value) = v else {
            return Ok(None);
        };

        let mut pairs: Vec<(Vec<u8>, Vec<u8>)> = Vec::new();
        for item in self.db.iter() {
            let kv = item.map_err(|_| StateError::DbIo)?;
            pairs.push((kv.0.to_vec(), kv.1.to_vec()));
        }
        pairs.sort_by(|a, b| a.0.cmp(&b.0));

        let root = merkle_root_sorted(&pairs);
        let idx = pairs.binary_search_by(|p| p.0.as_slice().cmp(key)).ok();
        let Some(i) = idx else {
            return Ok(None);
        };

        let proof = merkle_proof_sorted(&pairs, i);
        match proof {
            None => Ok(None),
            Some(p) => Ok(Some((pairs[i].0.clone(), pairs[i].1.clone(), root, p))),
        }
    }

    /// Verify a Merkle proof.
    pub fn verify_proof(root: Hash32, proof: &MerkleProof) -> bool {
        verify_proof(root, proof)
    }
}
