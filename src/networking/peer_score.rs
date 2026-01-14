// Copyright (c) 2026 Amunchain
// Licensed under the Apache License, Version 2.0

//! Peer scoring used for networking decisions.
//! This implementation is deterministic, integer-only, and borrow-safe.

#![forbid(unsafe_code)]

use std::{
    collections::BTreeMap,
    time::{Duration, Instant},
};

#[derive(Clone, Debug)]
pub struct ScoreParams {
    pub good_inc: i32,
    pub bad_inc: i32,
    pub decay_per_min: i32,
    pub ban_threshold: i32,
}

impl Default for ScoreParams {
    fn default() -> Self {
        Self {
            good_inc: 1,
            bad_inc: 5,
            decay_per_min: 1,
            ban_threshold: 200,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Decision {
    Allow,
    Throttle,
    Ban,
}

#[derive(Clone, Debug)]
struct PeerState {
    score: i32,
    last: Instant,
}

pub struct PeerScore {
    params: ScoreParams,
    peers: BTreeMap<Vec<u8>, PeerState>,
}

impl PeerScore {
    pub fn new(params: ScoreParams) -> Self {
        Self {
            params,
            peers: BTreeMap::new(),
        }
    }

    pub fn score_of(&self, peer: &[u8]) -> i32 {
        self.peers.get(peer).map(|p| p.score).unwrap_or(0)
    }

    pub fn observe_good(&mut self, peer: Vec<u8>, now: Instant, weight: i32) -> Decision {
        let params = self.params.clone(); // avoid borrow issues
        let st = self.peers.entry(peer).or_insert(PeerState {
            score: 0,
            last: now,
        });
        Self::decay_inner(&params, st, now);

        st.score = st
            .score
            .saturating_add(params.good_inc.saturating_mul(weight.max(1)));
        st.score = st.score.clamp(-1000, 1000);

        Self::decision_from_score(&params, st.score)
    }

    pub fn observe_bad(&mut self, peer: Vec<u8>, now: Instant, weight: i32) -> Decision {
        let params = self.params.clone(); // avoid borrow issues
        let st = self.peers.entry(peer).or_insert(PeerState {
            score: 0,
            last: now,
        });
        Self::decay_inner(&params, st, now);

        st.score = st
            .score
            .saturating_sub(params.bad_inc.saturating_mul(weight.max(1)));
        st.score = st.score.clamp(-1000, 1000);

        Self::decision_from_score(&params, st.score)
    }

    fn decision_from_score(params: &ScoreParams, score: i32) -> Decision {
        if score <= -(params.ban_threshold) {
            Decision::Ban
        } else if score < 0 {
            Decision::Throttle
        } else {
            Decision::Allow
        }
    }

    fn decay_inner(params: &ScoreParams, st: &mut PeerState, now: Instant) {
        let dt = now.saturating_duration_since(st.last);
        let mins = (dt.as_secs() / 60) as i32;
        if mins <= 0 {
            return;
        }
        let d = params.decay_per_min.saturating_mul(mins);
        if st.score > 0 {
            st.score = (st.score - d).max(0);
        } else if st.score < 0 {
            st.score = (st.score + d).min(0);
        }
        st.last = now;
    }
}

// Helper for older Rust: saturating_duration_since is stable on Instant
trait InstantExt {
    fn saturating_duration_since(&self, earlier: Instant) -> Duration;
}

impl InstantExt for Instant {
    fn saturating_duration_since(&self, earlier: Instant) -> Duration {
        self.checked_duration_since(earlier)
            .unwrap_or(Duration::from_secs(0))
    }
}
