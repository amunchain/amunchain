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
#![deny(missing_docs)]

//! Domain-separated signing bytes for consensus messages.

use crate::core::types::{encode_canonical, H256, ValidatorId};
use thiserror::Error;

/// Signing error.
#[derive(Debug, Error)]
pub enum SigningError {
    #[error("codec")]
    Codec,
}

/// Vote signing payload: domain || height || round || block_hash || voter
///
/// This payload is also used for commit verification (commit signatures are
/// expected to be the signatures of the corresponding precommit votes).
pub fn vote_signing_bytes_v1(
    height: u64,
    round: u64,
    block_hash: H256,
    voter: &ValidatorId,
) -> Result<Vec<u8>, SigningError> {
    let mut out = Vec::with_capacity(32 + 8 + 8 + 32 + voter.0.len());
    out.extend_from_slice(b"Amunchain-Tide-Vote-v1");
    out.extend_from_slice(&height.to_be_bytes());
    out.extend_from_slice(&round.to_be_bytes());
    out.extend_from_slice(block_hash.as_bytes());
    let vb = encode_canonical(&voter.0).map_err(|_| SigningError::Codec)?;
    out.extend_from_slice(&vb);
    Ok(out)
}


/// Vote signing payload v2 (replay-window sealed):
/// domain || height || round || epoch || msg_counter || sent_ts_ms || ttl_ms || block_hash || voter
pub fn vote_signing_bytes_v2(
    height: u64,
    round: u64,
    epoch: u64,
    msg_counter: u64,
    sent_ts_ms: u64,
    ttl_ms: u32,
    block_hash: H256,
    voter: &ValidatorId,
) -> Result<Vec<u8>, SigningError> {
    let mut out = Vec::with_capacity(40 + 8 * 5 + 4 + 32 + voter.0.len());
    out.extend_from_slice(b"Amunchain-Tide-Vote-v2");
    out.extend_from_slice(&height.to_be_bytes());
    out.extend_from_slice(&round.to_be_bytes());
    out.extend_from_slice(&epoch.to_be_bytes());
    out.extend_from_slice(&msg_counter.to_be_bytes());
    out.extend_from_slice(&sent_ts_ms.to_be_bytes());
    out.extend_from_slice(&ttl_ms.to_be_bytes());
    out.extend_from_slice(block_hash.as_bytes());
    let vb = encode_canonical(&voter.0).map_err(|_| SigningError::Codec)?;
    out.extend_from_slice(&vb);
    Ok(out)
}

/// Auto-select signing bytes version.
/// - If `epoch/msg_counter/sent_ts_ms/ttl_ms` are all zero => v1 (legacy).
/// - Otherwise => v2.
pub fn vote_signing_bytes_auto(
    height: u64,
    round: u64,
    epoch: u64,
    msg_counter: u64,
    sent_ts_ms: u64,
    ttl_ms: u32,
    block_hash: H256,
    voter: &ValidatorId,
) -> Result<Vec<u8>, SigningError> {
    if epoch == 0 && msg_counter == 0 && sent_ts_ms == 0 && ttl_ms == 0 {
        vote_signing_bytes_v1(height, round, block_hash, voter)
    } else {
        vote_signing_bytes_v2(height, round, epoch, msg_counter, sent_ts_ms, ttl_ms, block_hash, voter)
    }
}

