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

//! Amunchain Layer 0 - production-oriented blockchain framework skeleton.
//!
//! This repository provides:
//! - Deterministic types & canonical encoding
//! - A BFT-lite finality gadget (Tide) with signature verification
//! - Encrypted P2P transport (libp2p Noise + Yamux) with anti-replay and peer scoring
//! - Persistent key-value state with deterministic Merkle roots and proofs
//! - Monitoring via Prometheus metrics and structured JSON logging

/// Core protocol primitives (types, consensus, state, security).
pub mod core;
/// Observability (metrics, structured logging helpers).
pub mod monitoring;
/// P2P networking stack (libp2p transport, scoring, anti-abuse).
pub mod networking;
