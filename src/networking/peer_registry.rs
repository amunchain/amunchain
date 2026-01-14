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

//! Signed peer registry for loading validator allowlists.
//!
//! ## Format (TOML)
//! The registry is an **out-of-band** artifact (object storage, config management, etc.) verified
//! locally before use.
//!
//! ```text
//! version = 1
//! network = "amunchain/consensus/v2"
//! issued_at_ms = 1730000000000
//! expires_at_ms = 1730003600000
//! peers = ["12D3KooW...", "..."]
//! signature_hex = "..."   # Ed25519 signature over canonical bytes (see below)
//! ```
//!
//! ## Canonical bytes
//! Canonical payload is an unambiguous, newline-delimited format:
//!
//! ```text
//! v1
//! network=<network>
//! issued_at_ms=<u64>
//! expires_at_ms=<u64>
//! peers
//! <peer1>
//! <peer2>
//! ...
//! ```
//!
//! A pinned Ed25519 public key verifies `signature_hex`.
//!
//! ## Security properties
//! - **Integrity/authenticity:** verified signature with pinned public key.
//! - **Freshness:** enforced with `issued_at_ms`, `expires_at_ms`, and node policy.
//! - **Rollback safety:** optional minimum version policy (and operationally, monotonically increasing
//!   `issued_at_ms` via config management).

use crate::core::security::keystore::{verify_pubkey_bytes, verify_sig_bytes};
use libp2p::PeerId;
use serde::Deserialize;
use std::collections::BTreeSet;
use std::fs;
use std::time::{SystemTime, UNIX_EPOCH};
use thiserror::Error;

/// Peer registry verification errors.
#[derive(Debug, Error)]
pub enum PeerRegistryError {
    /// Cannot read registry file.
    #[error("read registry")]
    Read,
    /// Cannot parse TOML.
    #[error("parse registry")]
    Parse,
    /// Registry public key is invalid.
    #[error("bad registry public key")]
    BadPubkey,
    /// Registry signature is invalid.
    #[error("bad registry signature")]
    BadSignature,
    /// Registry contains invalid peer id.
    #[error("invalid peer id")]
    InvalidPeer,
    /// Registry version is unsupported.
    #[error("unsupported registry version")]
    UnsupportedVersion,
    /// Registry not valid yet.
    #[error("registry not valid yet")]
    NotYetValid,
    /// Registry is expired.
    #[error("registry expired")]
    Expired,
    /// Registry issued time is too old per policy.
    #[error("registry too old per policy")]
    TooOld,
    /// Registry network does not match node topic.
    #[error("registry network mismatch")]
    NetworkMismatch,
    /// Missing required field.
    #[error("missing required field")]
    MissingField,
}

/// Registry verification policy (node-side).
#[derive(Clone, Debug)]
pub struct PeerRegistryPolicy<'a> {
    /// Current time in ms since UNIX epoch.
    pub now_ms: u64,
    /// Max accepted age for a registry (now - issued_at_ms) in ms. If 0, no age limit.
    pub max_age_ms: u64,
    /// Acceptable grace window in ms after `expires_at_ms`. If 0, no grace.
    pub grace_ms: u64,
    /// Minimum required registry format version (e.g., 1). If 0, accept any supported.
    pub min_version: u32,
    /// If set, require `network` to match this value.
    pub expected_network: Option<&'a str>,
    /// If true, require freshness fields (issued/expires) to be present and non-zero.
    pub require_freshness_fields: bool,
}

impl<'a> PeerRegistryPolicy<'a> {
    /// Create a policy with sane defaults.
    pub fn default_with_now(now_ms: u64) -> Self {
        Self {
            now_ms,
            max_age_ms: 0,
            grace_ms: 0,
            min_version: 0,
            expected_network: None,
            require_freshness_fields: true,
        }
    }
}

#[derive(Debug, Deserialize)]
struct PeerRegistryFile {
    /// Registry format version.
    version: u32,
    /// Network/topic binding (prevents cross-network rollback).
    #[serde(default)]
    network: Option<String>,
    /// Issued-at time in ms since UNIX epoch.
    #[serde(default)]
    issued_at_ms: Option<u64>,
    /// Expiration time in ms since UNIX epoch.
    #[serde(default)]
    expires_at_ms: Option<u64>,
    /// List of peer IDs (base58). Duplicates are allowed but will be deduplicated.
    #[serde(default)]
    peers: Vec<String>,
    /// Signature over canonical bytes (hex; Ed25519 64 bytes).
    signature_hex: String,
}

fn parse_hex_32(s: &str) -> Result<[u8; 32], PeerRegistryError> {
    let bytes = hex::decode(s.trim()).map_err(|_| PeerRegistryError::BadPubkey)?;
    if bytes.len() != 32 {
        return Err(PeerRegistryError::BadPubkey);
    }
    let mut out = [0u8; 32];
    out.copy_from_slice(&bytes);
    Ok(out)
}

fn parse_sig_64(s: &str) -> Result<[u8; 64], PeerRegistryError> {
    let bytes = hex::decode(s.trim()).map_err(|_| PeerRegistryError::BadSignature)?;
    if bytes.len() != 64 {
        return Err(PeerRegistryError::BadSignature);
    }
    let mut out = [0u8; 64];
    out.copy_from_slice(&bytes);
    Ok(out)
}

fn canonical_bytes(reg: &PeerRegistryFile, peers: &BTreeSet<PeerId>) -> Result<Vec<u8>, PeerRegistryError> {
    // Require basic fields for v1.
    if reg.version != 1 {
        return Err(PeerRegistryError::UnsupportedVersion);
    }
    let net = reg.network.as_deref().ok_or(PeerRegistryError::MissingField)?;
    let issued = reg.issued_at_ms.ok_or(PeerRegistryError::MissingField)?;
    let expires = reg.expires_at_ms.ok_or(PeerRegistryError::MissingField)?;

    let mut out = Vec::new();
    out.extend_from_slice(b"v1\n");
    out.extend_from_slice(format!("network={}\n", net).as_bytes());
    out.extend_from_slice(format!("issued_at_ms={}\n", issued).as_bytes());
    out.extend_from_slice(format!("expires_at_ms={}\n", expires).as_bytes());
    out.extend_from_slice(b"peers\n");
    for p in peers.iter() {
        out.extend_from_slice(p.to_base58().as_bytes());
        out.push(b'\n');
    }
    Ok(out)
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

/// Load and verify a signed peer registry, returning a deduplicated allowlist.
///
/// Node policy can enforce:
/// - freshness (issued_at / expires_at)
/// - age limits
/// - grace windows
/// - topic binding
/// Parse a peer registry TOML document (syntax + schema only).
///
/// This does **not** verify signatures. It is intended for tooling and fuzzing.
pub fn parse_peer_registry_toml(raw: &str) -> Result<(), PeerRegistryError> {
    let _reg: PeerRegistryFile = toml::from_str(raw).map_err(|_| PeerRegistryError::Parse)?;
    Ok(())
}

pub fn load_and_verify_peer_registry(
    path: &str,
    pubkey_hex: &str,
    policy: &PeerRegistryPolicy<'_>,
) -> Result<Vec<String>, PeerRegistryError> {
    // Public key must be a valid 32-byte Ed25519 pubkey.
    let pk = parse_hex_32(pubkey_hex)?;
    if !verify_pubkey_bytes(&pk) {
        return Err(PeerRegistryError::BadPubkey);
    }

    let raw = fs::read_to_string(path).map_err(|_| PeerRegistryError::Read)?;
    let reg: PeerRegistryFile = toml::from_str(&raw).map_err(|_| PeerRegistryError::Parse)?;

    // Version gate.
    if reg.version != 1 {
        return Err(PeerRegistryError::UnsupportedVersion);
    }
    if policy.min_version != 0 && reg.version < policy.min_version {
        return Err(PeerRegistryError::UnsupportedVersion);
    }

    // Freshness fields.
    let issued = reg.issued_at_ms.unwrap_or(0);
    let expires = reg.expires_at_ms.unwrap_or(0);
    if policy.require_freshness_fields {
        if issued == 0 || expires == 0 {
            return Err(PeerRegistryError::MissingField);
        }
    }
    if issued != 0 && policy.now_ms < issued {
        return Err(PeerRegistryError::NotYetValid);
    }
    if expires != 0 {
        let deadline = expires.saturating_add(policy.grace_ms);
        if policy.now_ms > deadline {
            return Err(PeerRegistryError::Expired);
        }
    }
    if policy.max_age_ms != 0 && issued != 0 {
        let age = policy.now_ms.saturating_sub(issued);
        if age > policy.max_age_ms {
            return Err(PeerRegistryError::TooOld);
        }
    }

    // Network/topic binding.
    if let Some(expected) = policy.expected_network {
        match reg.network.as_deref() {
            Some(n) if n == expected => {}
            _ => return Err(PeerRegistryError::NetworkMismatch),
        }
    }

    // Parse and dedupe peers.
    let mut peers = BTreeSet::new();
    for s in reg.peers.iter() {
        let p = PeerId::from_bytes(&bs58::decode(s).into_vec().map_err(|_| PeerRegistryError::InvalidPeer)?)
            .map_err(|_| PeerRegistryError::InvalidPeer)?;
        peers.insert(p);
    }

    let sig = parse_sig_64(&reg.signature_hex)?;
    let msg = canonical_bytes(&reg, &peers)?;
    if !verify_sig_bytes(&pk, &msg, &sig) {
        return Err(PeerRegistryError::BadSignature);
    }

    Ok(peers.into_iter().map(|p| p.to_base58()).collect())
}

/// Convenience helper using system time for `now_ms`.
pub fn load_and_verify_peer_registry_now(
    path: &str,
    pubkey_hex: &str,
    policy: &PeerRegistryPolicy<'_>,
) -> Result<Vec<String>, PeerRegistryError> {
    let mut p = policy.clone();
    if p.now_ms == 0 {
        p.now_ms = now_ms();
    }
    load_and_verify_peer_registry(path, pubkey_hex, &p)
}
