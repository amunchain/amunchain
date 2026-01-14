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

//! Tide finality gadget (BFT-lite) with signature verification and double-vote detection.

use crate::core::{
    consensus::signing::{vote_signing_bytes_auto, SigningError},
    security::keystore::{Keystore, KeystoreError},
    types::{CanonicalMap, Commit, H256, Signature, ValidatorId, Vote},
};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

/// Tide errors.
#[derive(Debug, Error)]
pub enum TideError {
    /// Replay, stale, or out-of-window message rejected.
    #[error("replay/stale message")]
    Replay,
#[error("unknown validator")]
    UnknownValidator,
    #[error("invalid signature")]
    BadSignature,
    #[error("double vote detected")]
    DoubleVote,
    #[error("insufficient votes for commit")]
    NotEnoughVotes,
    #[error("codec/signing")]
    Signing,
    #[error("keystore")]
    Keystore,
}

impl From<SigningError> for TideError {
    fn from(_: SigningError) -> Self {
        TideError::Signing
    }
}
impl From<KeystoreError> for TideError {
    fn from(_: KeystoreError) -> Self {
        TideError::Keystore
    }
}

/// Slashing hook.
pub trait Slashing: Send + Sync {
    /// Called when a double vote is detected.
    fn on_double_vote(&self, offender: &ValidatorId);
}

/// No-op slashing (default).
#[derive(Clone)]
pub struct NoopSlashing;

impl Slashing for NoopSlashing {
    fn on_double_vote(&self, _offender: &ValidatorId) {}
}

/// Tide configuration.
#[derive(Clone)]
pub struct TideConfig {
    /// Current validator set.
    pub validators: BTreeSet<ValidatorId>,
    /// Maximum allowed clock skew (absolute) between sender timestamp and local time.
    /// Applies only when `sent_ts_ms != 0`.
    pub max_clock_skew_ms: u64,
    /// Maximum allowed TTL for consensus messages.
    /// Applies only when `ttl_ms != 0`.
    pub max_ttl_ms: u32,
    /// If true, reject legacy messages where `epoch == 0`.
    pub require_epoch: bool,
}

impl TideConfig {
    /// Create config with safe defaults.
    pub fn new(validators: BTreeSet<ValidatorId>) -> Self {
        Self {
            validators,
            // 10s skew is usually safe across VPS/VMs while still limiting replay window.
            max_clock_skew_ms: 10_000,
            // 60s TTL cap for gossip consensus messages.
            max_ttl_ms: 60_000,
            require_epoch: cfg!(feature = "production"),
        }
    }
}
/// Stored metadata for replay-window sealed votes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct VoteMeta {
    epoch: u64,
    msg_counter: u64,
    sent_ts_ms: u64,
    ttl_ms: u32,
}

#[derive(Clone, Copy, Debug)]
struct ReplayState {
    epoch: u64,
    last_counter: u64,
    last_sent_ts_ms: u64,
}
type VoteMap = BTreeMap<ValidatorId, (H256, Signature, VoteMeta)>;

/// Tide finalizer state.
pub struct TideFinalizer<S: Slashing> {
    cfg: TideConfig,
    slashing: S,
    // votes[height][round] = { voter -> (block_hash, sig, meta) }
    votes: BTreeMap<u64, BTreeMap<u64, VoteMap>>,
    // Per-validator replay protection state (best-effort).
    replay: BTreeMap<ValidatorId, ReplayState>,
}
impl<S: Slashing> TideFinalizer<S> {
    /// Create a new finalizer.
    pub fn new(cfg: TideConfig, slashing: S) -> Self {
        Self {
            cfg,
            slashing,
            votes: BTreeMap::new(),
            replay: BTreeMap::new(),
        }
    }

    fn now_ms() -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0)
    }

    fn check_freshness(&self, sent_ts_ms: u64, ttl_ms: u32) -> Result<(), TideError> {
        if self.cfg.require_epoch {
            // epoch enforcement is handled at caller; keep this helper focused.
        }
        // Legacy messages skip freshness checks.
        if sent_ts_ms == 0 && ttl_ms == 0 {
            return Ok(());
        }

        // Cap TTL to avoid attackers selecting huge replay windows.
        if ttl_ms != 0 && ttl_ms > self.cfg.max_ttl_ms {
            return Err(TideError::Replay);
        }

        let now = Self::now_ms();
        if now == 0 {
            // If local time is unavailable, be conservative for non-legacy messages.
            return Err(TideError::Replay);
        }

        if sent_ts_ms != 0 {
            let skew = if now >= sent_ts_ms { now - sent_ts_ms } else { sent_ts_ms - now };
            if skew > self.cfg.max_clock_skew_ms {
                return Err(TideError::Replay);
            }
        }

        if sent_ts_ms != 0 && ttl_ms != 0 {
            let expiry = sent_ts_ms.saturating_add(ttl_ms as u64);
            if now > expiry.saturating_add(self.cfg.max_clock_skew_ms) {
                return Err(TideError::Replay);
            }
        }

        Ok(())
    }

    fn check_replay_counter(
        &mut self,
        voter: &ValidatorId,
        epoch: u64,
        msg_counter: u64,
        sent_ts_ms: u64,
    ) -> Result<(), TideError> {
        // Legacy messages do not carry replay protection fields.
        if epoch == 0 && msg_counter == 0 && sent_ts_ms == 0 {
            if self.cfg.require_epoch {
                return Err(TideError::Replay);
            }
            return Ok(());
        }
        if self.cfg.require_epoch && epoch == 0 {
            return Err(TideError::Replay);
        }

        let st = self.replay.get(voter).copied();
        if let Some(prev) = st {
            if prev.epoch == epoch {
                if msg_counter != 0 && msg_counter <= prev.last_counter {
                    return Err(TideError::Replay);
                }
                // Best-effort: also require non-decreasing timestamps if provided.
                if sent_ts_ms != 0 && prev.last_sent_ts_ms != 0 && sent_ts_ms < prev.last_sent_ts_ms {
                    return Err(TideError::Replay);
                }
            }
        }

        // Update replay state (best-effort).
        self.replay.insert(
            voter.clone(),
            ReplayState {
                epoch,
                last_counter: msg_counter,
                last_sent_ts_ms: sent_ts_ms,
            },
        );

        Ok(())
    }
/// Verify vote signature then process.
    pub fn process_vote_verified(&mut self, v: Vote) -> Result<Option<Commit>, TideError> {
        if !self.cfg.validators.contains(&v.voter) {
            return Err(TideError::UnknownValidator);
        }

        self.check_freshness(v.sent_ts_ms, v.ttl_ms)?;
        self.check_replay_counter(&v.voter, v.epoch, v.msg_counter, v.sent_ts_ms)?;
let pk_bytes = v.voter.as_public_key_bytes().ok_or(TideError::BadSignature)?;
        let msg = vote_signing_bytes_auto(
            v.height,
            v.round,
            v.epoch,
            v.msg_counter,
            v.sent_ts_ms,
            v.ttl_ms,
            v.block_hash,
            &v.voter,
        )?;
        Keystore::verify_pubkey_bytes(&pk_bytes, &msg, &v.signature).map_err(|_| TideError::BadSignature)?;

        self.process_vote_inner(v)
    }

    /// Verify commit signatures (supermajority) and accept.
    pub fn process_commit_verified(&mut self, c: Commit) -> Result<(), TideError> {
        self.check_freshness(c.sent_ts_ms, c.ttl_ms)?;
        if self.cfg.require_epoch && c.epoch == 0 {
            return Err(TideError::Replay);
        }
for (vid, _sig) in c.signatures.iter() {
            if !self.cfg.validators.contains(vid) {
                return Err(TideError::UnknownValidator);
            }
        }

        let n = self.cfg.validators.len();
        let threshold = (2 * n) / 3 + 1;
        if c.signatures.len() < threshold {
            return Err(TideError::NotEnoughVotes);
        }

        
        for (vid, sig) in c.signatures.iter() {
            let pk_bytes = vid.as_public_key_bytes().ok_or(TideError::BadSignature)?;
            let bytes = vote_signing_bytes_auto(
                c.height,
                c.round,
                c.epoch,
                c.msg_counter,
                c.sent_ts_ms,
                c.ttl_ms,
                c.block_hash,
                vid,
            )?;
            Keystore::verify_pubkey_bytes(&pk_bytes, &bytes, sig).map_err(|_| TideError::BadSignature)?;
        }

        Ok(())
    }

    fn process_vote_inner(&mut self, v: Vote) -> Result<Option<Commit>, TideError> {
        let height_votes = self.votes.entry(v.height).or_default();
        let round_votes = height_votes.entry(v.round).or_default();

        let meta = VoteMeta { epoch: v.epoch, msg_counter: v.msg_counter, sent_ts_ms: v.sent_ts_ms, ttl_ms: v.ttl_ms };

        if let Some((prev_hash, _prev_sig, prev_meta)) = round_votes.get(&v.voter) {
            if prev_hash != &v.block_hash || prev_meta != &meta {
                self.slashing.on_double_vote(&v.voter);
                return Err(TideError::DoubleVote);
            }
            return Ok(None); // duplicate same vote
        }

        round_votes.insert(v.voter.clone(), (v.block_hash, v.signature.clone(), meta));
        self.try_build_commit(v.height, v.round)
    }

    fn try_build_commit(&self, height: u64, round: u64) -> Result<Option<Commit>, TideError> {
        let Some(hm) = self.votes.get(&height) else { return Ok(None); };
        let Some(rm) = hm.get(&round) else { return Ok(None); };

        let mut counts: BTreeMap<(H256, VoteMeta), usize> = BTreeMap::new();
        for (hash, _sig, meta) in rm.values() {
            *counts.entry((*hash, *meta)).or_insert(0) += 1;
        }

        let n = self.cfg.validators.len();
        let threshold = (2 * n) / 3 + 1;

        for ((hash, meta), c) in counts.iter() {
            if *c >= threshold {
                let mut sigs: CanonicalMap<ValidatorId, Signature> = CanonicalMap::new();
                for (vid, (vh, vsig, vm)) in rm.iter() {
                    if vh == hash && vm == meta {
                        sigs.insert(vid.clone(), vsig.clone());
                    }
                }
                return Ok(Some(Commit {
                    height,
                    round,
                    epoch: meta.epoch,
                    msg_counter: meta.msg_counter,
                    sent_ts_ms: meta.sent_ts_ms,
                    ttl_ms: meta.ttl_ms,
                    block_hash: *hash,
                    signatures: sigs,
                }));
            }
        }

        Ok(None)
    }
}

#[cfg(test)]
impl<S: Slashing> TideFinalizer<S> {
    /// Test-only helper: process a vote without network plumbing.

    pub fn process_vote_inner_for_tests(&mut self, v: Vote) -> Result<Option<Commit>, TideError> {
        self.process_vote_inner(v)
    }
}
