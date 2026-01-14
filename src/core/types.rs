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

//! Deterministic core types and canonical encoding helpers.

use bincode::Options;
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use std::collections::BTreeMap;
use thiserror::Error;

/// Canonical serialization error.
#[derive(Debug, Error)]
pub enum CodecError {
    #[error("serialization")]
    Serialize,
    #[error("deserialization")]
    Deserialize,
    #[error("size limit exceeded")]
    TooLarge,
}

/// Canonical bincode options (deterministic).
fn bincode_opts() -> impl Options {
    // Fixint encoding provides a stable integer representation.
    bincode::DefaultOptions::new()
        .with_fixint_encoding()
        .reject_trailing_bytes()
}

/// Encode with deterministic rules. Requires deterministic container ordering (use BTreeMap/BTreeSet).
pub fn encode_canonical<T: Serialize>(v: &T) -> Result<Vec<u8>, CodecError> {
    bincode_opts()
        .serialize(v)
        .map_err(|_| CodecError::Serialize)
}

/// Decode with a hard size cap.
pub fn decode_canonical_limited<T: DeserializeOwned>(
    bytes: &[u8],
    max: usize,
) -> Result<T, CodecError> {
    // Fast-path cap on the raw wire payload.
    if bytes.len() > max {
        return Err(CodecError::TooLarge);
    }
    // Defensive cap inside the deserializer to prevent memory/CPU bombs via large container lengths.
    // `with_limit` is in bytes; we keep it aligned to the external `max`.
    bincode_opts()
        .with_limit(max as u64)
        .deserialize(bytes)
        .map_err(|_| CodecError::Deserialize)
}

/// 256-bit hash type (32 bytes).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct H256([u8; 32]);

impl H256 {
    /// Construct from raw bytes.
    pub fn from_bytes(b: [u8; 32]) -> Self {
        Self(b)
    }
    /// Return bytes.
    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

/// Ed25519 signature bytes (expected 64).
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Signature(pub Vec<u8>);

/// Validator identity (Ed25519 public key bytes, expected 32).
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct ValidatorId(pub Vec<u8>);

impl ValidatorId {
    /// Interpret as Ed25519 public key bytes if length is 32.
    pub fn as_public_key_bytes(&self) -> Option<[u8; 32]> {
        if self.0.len() != 32 {
            return None;
        }
        let mut out = [0u8; 32];
        out.copy_from_slice(&self.0);
        Some(out)
    }
}

/// Canonical map type alias.
pub type CanonicalMap<K, V> = BTreeMap<K, V>;

/// Consensus vote message.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Vote {
    /// Block height.
    pub height: u64,
    /// Consensus round (height-bound).
    pub round: u64,
    /// Epoch identifier (0 => legacy messages).
    #[serde(default)]
    pub epoch: u64,
    /// Per-sender monotonically increasing message counter (0 => legacy).
    #[serde(default)]
    pub msg_counter: u64,
    /// Sender wall-clock timestamp in milliseconds since UNIX epoch (0 => legacy).
    #[serde(default)]
    pub sent_ts_ms: u64,
    /// Time-to-live for this message in milliseconds (0 => legacy).
    #[serde(default)]
    pub ttl_ms: u32,
    /// Voted block hash.
    pub block_hash: H256,
    /// Voter identity.
    pub voter: ValidatorId,
    /// Signature over domain-separated vote bytes.
    pub signature: Signature,
}

/// Commit message proving finality.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Commit {
    /// Height.
    pub height: u64,
    /// Round.
    pub round: u64,
    /// Epoch identifier (0 => legacy messages).
    #[serde(default)]
    pub epoch: u64,
    /// Per-sender monotonically increasing message counter (0 => legacy).
    #[serde(default)]
    pub msg_counter: u64,
    /// Sender wall-clock timestamp in milliseconds since UNIX epoch (0 => legacy).
    #[serde(default)]
    pub sent_ts_ms: u64,
    /// Time-to-live for this message in milliseconds (0 => legacy).
    #[serde(default)]
    pub ttl_ms: u32,
    /// Finalized block hash.
    pub block_hash: H256,
    /// Signatures by validators (canonical ordering by key).
    pub signatures: CanonicalMap<ValidatorId, Signature>,
}

/// Wire-level consensus messages.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum ConsensusMsg {
    /// Vote broadcast.
    Vote(Vote),
    /// Commit broadcast.
    Commit(Commit),
}

/// Node configuration root.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct NodeConfig {
    /// Node settings.
    pub node: NodeSettings,
    /// HTTP endpoints.
    pub http: HttpConfig,
    /// P2P settings.
    pub p2p: NodeP2pConfig,
    /// Consensus settings.
    pub consensus: ConsensusConfig,
}

/// Node settings.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct NodeSettings {
    /// Human-readable name.
    pub name: String,
    /// Data directory (db + keys).
    pub data_dir: String,
}

/// HTTP config.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct HttpConfig {
    /// Listen address, e.g. 0.0.0.0:9090.
    pub listen_addr: String,
}

/// P2P config embedded in node config.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct NodeP2pConfig {
    /// Listen multiaddr.
    pub listen_addr: String,
    /// Gossipsub topic.
    pub topic: String,
    /// Max messages/sec per peer.
    pub max_msg_per_sec: u32,
    /// Max peers per IP (best-effort).
    pub max_peers_per_ip: usize,

    /// Bootstrap peers to dial at startup.
    #[serde(default)]
    pub bootstrap: Vec<String>,
    /// Allowlist of PeerIds (empty => allow all unless `require_allow_peers` is true).
    #[serde(default)]
    pub allow_peers: Vec<String>,
    /// If true, refuse to start when `allow_peers` is empty (recommended for production).
    #[serde(default)]
    pub require_allow_peers: bool,

    /// Optional path to a signed peer registry file (TOML). If set and `allow_peers` is empty,
    /// the node will load and verify the registry to populate the allowlist.
    #[serde(default)]
    pub peer_registry_path: Option<String>,

    /// Ed25519 public key (hex, 32 bytes) used to verify `peer_registry_path` signatures.
    /// Required when `peer_registry_path` is set.
    #[serde(default)]
    pub peer_registry_pubkey_hex: Option<String>,

    /// Minimum required peer registry format version. (e.g., 1). If 0, accept any supported.
    #[serde(default)]
    pub peer_registry_min_version: u32,

    /// Max age (now - issued_at_ms) allowed for the registry in milliseconds. If 0, no age limit.
    #[serde(default)]
    pub peer_registry_max_age_ms: u64,

    /// Grace period after expires_at_ms in milliseconds. If 0, no grace.
    #[serde(default)]
    pub peer_registry_grace_ms: u64,

    /// Require registry freshness fields (issued_at_ms/expires_at_ms/network) to be present.
    #[serde(default)]
    pub peer_registry_require_fresh: bool,
}

/// Consensus config.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ConsensusConfig {
    /// Validator public keys in hex (32 bytes each).
    pub validators_hex: Vec<String>,
}
