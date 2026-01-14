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

//! Hydro consensus (block production selection) - minimal skeleton.
//!
//! This module is provided as a safe starting point, focusing on the critical
//! fixes requested:
//! - Absolute time window checks
//! - Canonical VRF transcript bytes
//! - PoW difficulty check (hash < target)
//!
//! Full block production, VRF verification, and fork-choice are intentionally
//! kept minimal here; integrate with your block format in later phases.

use crate::core::types::H256;
use thiserror::Error;

/// Hydro errors.
#[derive(Debug, Error)]
pub enum HydroError {
    #[error("time outside slot window")]
    TimeWindow,
    #[error("invalid difficulty")]
    Difficulty,
}

/// Hydro configuration.
#[derive(Clone, Debug)]
pub struct HydroConfig {
    /// Genesis absolute time (ms since unix epoch).
    pub genesis_time_ms: u64,
    /// Slot duration in ms.
    pub slot_ms: u64,
    /// Allowed clock skew in ms.
    pub skew_ms: u64,
    /// Epoch randomness (32 bytes).
    pub epoch_randomness: [u8; 32],
}

impl HydroConfig {
    /// Build canonical VRF transcript.
    pub fn build_vrf_transcript(&self, slot: u64, parent_hash: H256) -> Vec<u8> {
        let mut transcript = Vec::with_capacity(4 + 8 + 32 + 32);
        transcript.extend_from_slice(b"Amunchain-Hydro-VRF-v1");
        transcript.extend_from_slice(&slot.to_be_bytes());
        transcript.extend_from_slice(parent_hash.as_bytes());
        transcript.extend_from_slice(&self.epoch_randomness);
        transcript
    }

    /// Absolute time window check for slot.
    pub fn check_time_window_abs(&self, current_abs_ms: u64, slot_start_abs_ms: u64) -> Result<u64, HydroError> {
        let slot_end_abs_ms = slot_start_abs_ms.saturating_add(self.slot_ms);
        let min_ok = slot_start_abs_ms.saturating_sub(self.skew_ms);
        let max_ok = slot_end_abs_ms.saturating_add(self.skew_ms);

        if current_abs_ms < min_ok || current_abs_ms > max_ok {
            return Err(HydroError::TimeWindow);
        }
        // relative for header, if needed
        Ok(current_abs_ms.saturating_sub(self.genesis_time_ms))
    }

    /// PoW difficulty: hash < target.
    pub fn verify_difficulty(&self, hash: &H256, target: [u8; 32]) -> Result<(), HydroError> {
        if hash.as_bytes() < &target {
            Ok(())
        } else {
            Err(HydroError::Difficulty)
        }
    }
}
