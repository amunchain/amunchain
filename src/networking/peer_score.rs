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

use libp2p::PeerId;
use std::{
    collections::BTreeMap,
    time::{Duration, Instant},
};

/// Score parameters.
#[derive(Clone, Debug)]
pub struct ScoreParams {
    /// <= this => ban.
    pub ban_threshold: i32,
    /// >= this => good.
    pub good_threshold: i32,
    /// Max clamp.
    pub max_score: i32,
    /// Min clamp.
    pub min_score: i32,
    /// Decay per minute toward 0.
    pub decay_per_min: i32,
    /// Base ban time.
    pub ban_base_secs: u64,
    /// Max ban time.
    pub ban_max_secs: u64,
}

impl Default for ScoreParams {
    fn default() -> Self {
        Self {
            ban_threshold: -100,
            good_threshold: 50,
            max_score: 200,
            min_score: -200,
            decay_per_min: 2,
            ban_base_secs: 30,
            ban_max_secs: 3600,
        }
    }
}

#[derive(Clone, Debug)]
struct PeerState {
    score: i32,
    last_decay: Instant,
    banned_until: Option<Instant>,
    ban_level: u32,
}

/// Peer scoring.
pub struct PeerScore {
    params: ScoreParams,
    peers: BTreeMap<PeerId, PeerState>,
}

/// Enforcement decision.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Decision {
    /// Accept.
    Allow,
    /// Throttle.
    Throttle,
    /// Ban.
    Ban,
}

impl PeerScore {
    /// Create a new peer scorer with the given parameters.
    pub fn new(params: ScoreParams) -> Self {
        Self { params, peers: BTreeMap::new() }
    }

    /// Return true if the peer is currently banned.
    pub fn is_banned(&mut self, p: &PeerId, now: Instant) -> bool {
        self.decay(p, now);
        match self.peers.get(p).and_then(|s| s.banned_until) {
            None => false,
            Some(t) => now < t,
        }
    }

    /// Record positive behavior and return an enforcement decision.
    pub fn observe_good(&mut self, p: PeerId, now: Instant, delta: i32) -> Decision {
        self.apply(p, now, delta.max(0))
    }

    /// Record negative behavior and return an enforcement decision.
    pub fn observe_bad(&mut self, p: PeerId, now: Instant, delta: i32) -> Decision {
        self.apply(p, now, -delta.abs())
    }

    /// Get the current score for a peer (after applying decay).
    pub fn score(&mut self, p: &PeerId, now: Instant) -> i32 {
        self.decay(p, now);
        self.peers.get(p).map(|s| s.score).unwrap_or(0)
    }

    fn apply(&mut self, p: PeerId, now: Instant, delta: i32) -> Decision {
        let st = self.peers.entry(p).or_insert_with(|| PeerState {
            score: 0,
            last_decay: now,
            banned_until: None,
            ban_level: 0,
        });

        self.decay_inner(st, now);

        if let Some(until) = st.banned_until {
            if now < until {
                return Decision::Ban;
            }
            st.banned_until = None;
        }

        st.score = st.score.saturating_add(delta);
        st.score = st.score.clamp(self.params.min_score, self.params.max_score);

        if st.score <= self.params.ban_threshold {
            st.ban_level = st.ban_level.saturating_add(1);
            let ban_secs = backoff_secs(self.params.ban_base_secs, self.params.ban_max_secs, st.ban_level);
            st.banned_until = Some(now + Duration::from_secs(ban_secs));
            return Decision::Ban;
        }

        if st.score < 0 { Decision::Throttle } else { Decision::Allow }
    }

    fn decay(&mut self, p: &PeerId, now: Instant) {
        if let Some(st) = self.peers.get_mut(p) {
            self.decay_inner(st, now);
        }
    }

    fn decay_inner(&self, st: &mut PeerState, now: Instant) {
        let elapsed = now.duration_since(st.last_decay);
        let mins = (elapsed.as_secs() / 60) as i32;
        if mins <= 0 { return; }
        st.last_decay = st.last_decay + Duration::from_secs((mins as u64) * 60);

        let d = self.params.decay_per_min.saturating_mul(mins);
        if st.score > 0 {
            st.score = (st.score - d).max(0);
        } else if st.score < 0 {
            st.score = (st.score + d).min(0);
        }
    }
}

fn backoff_secs(base: u64, cap: u64, level: u32) -> u64 {
    let pow = level.saturating_sub(1).min(16);
    let mut v = base.saturating_mul(1u64 << pow);
    if v > cap { v = cap; }
    v
}
