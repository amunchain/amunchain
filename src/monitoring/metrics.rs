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

use prometheus::{IntCounter, IntGauge, Registry};
use thiserror::Error;

/// Metrics errors.
#[derive(Debug, Error)]
pub enum MetricsError {
    #[error("prometheus")]
    Prom,
}

/// Metrics container.
#[derive(Clone)]
pub struct Metrics {
    /// Registry.
    pub registry: Registry,

    /// Connected peers gauge.
    pub p2p_peers: IntGauge,
    /// Block height gauge (optional wiring).
    pub block_height: IntGauge,
    /// Total transactions counter (optional wiring).
    pub transactions_total: IntCounter,

    /// Dropped replay messages.
    pub p2p_replay_dropped_total: IntCounter,
    /// Invalid decoded messages.
    pub p2p_invalid_msg_total: IntCounter,
    /// Rate-limited messages.
    pub p2p_rate_limited_total: IntCounter,
    /// Reputation-based throttled messages.
    pub p2p_reputation_throttled_total: IntCounter,
    /// Banned peer events.
    pub p2p_banned_total: IntCounter,
}

impl Metrics {
    /// Create and register metrics.
    pub fn new() -> Result<Self, MetricsError> {
        let registry = Registry::new();

        let p2p_peers = IntGauge::new("amunchain_p2p_peers", "Connected peers")
            .map_err(|_| MetricsError::Prom)?;
        let block_height = IntGauge::new("amunchain_block_height", "Current block height")
            .map_err(|_| MetricsError::Prom)?;
        let transactions_total =
            IntCounter::new("amunchain_transactions_total", "Total tx processed")
                .map_err(|_| MetricsError::Prom)?;

        let p2p_replay_dropped_total = IntCounter::new(
            "amunchain_p2p_replay_dropped_total",
            "Dropped replay messages",
        )
        .map_err(|_| MetricsError::Prom)?;
        let p2p_invalid_msg_total = IntCounter::new(
            "amunchain_p2p_invalid_msg_total",
            "Invalid decoded messages",
        )
        .map_err(|_| MetricsError::Prom)?;
        let p2p_rate_limited_total =
            IntCounter::new("amunchain_p2p_rate_limited_total", "Rate-limited messages")
                .map_err(|_| MetricsError::Prom)?;
        let p2p_reputation_throttled_total = IntCounter::new(
            "amunchain_p2p_reputation_throttled_total",
            "Reputation-based throttled messages",
        )
        .map_err(|_| MetricsError::Prom)?;
        let p2p_banned_total = IntCounter::new("amunchain_p2p_banned_total", "Banned peer events")
            .map_err(|_| MetricsError::Prom)?;

        registry
            .register(Box::new(p2p_peers.clone()))
            .map_err(|_| MetricsError::Prom)?;
        registry
            .register(Box::new(block_height.clone()))
            .map_err(|_| MetricsError::Prom)?;
        registry
            .register(Box::new(transactions_total.clone()))
            .map_err(|_| MetricsError::Prom)?;

        registry
            .register(Box::new(p2p_replay_dropped_total.clone()))
            .map_err(|_| MetricsError::Prom)?;
        registry
            .register(Box::new(p2p_invalid_msg_total.clone()))
            .map_err(|_| MetricsError::Prom)?;
        registry
            .register(Box::new(p2p_rate_limited_total.clone()))
            .map_err(|_| MetricsError::Prom)?;
        registry
            .register(Box::new(p2p_reputation_throttled_total.clone()))
            .map_err(|_| MetricsError::Prom)?;
        registry
            .register(Box::new(p2p_banned_total.clone()))
            .map_err(|_| MetricsError::Prom)?;

        Ok(Self {
            registry,
            p2p_peers,
            block_height,
            transactions_total,
            p2p_replay_dropped_total,
            p2p_invalid_msg_total,
            p2p_rate_limited_total,
            p2p_reputation_throttled_total,
            p2p_banned_total,
        })
    }
}
