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

use anyhow::Result;
use axum::{extract::State, http::{header::AUTHORIZATION, HeaderMap, StatusCode}, response::IntoResponse, routing::get, Router};
use prometheus::TextEncoder;
use std::{
    collections::BTreeSet,
    net::SocketAddr,
    sync::{
        atomic::{AtomicBool, AtomicUsize, Ordering},
        Arc,
    },
};
use tokio::sync::Mutex;
use tracing::{info, Level};
use tracing_subscriber::FmtSubscriber;

use amunchain::{
    core::{
        consensus::driver::ConsensusDriver,
        state::persistent_state::PersistentState,
        types::{NodeConfig, ValidatorId},
    },
    monitoring::metrics::Metrics,
    networking::{
        p2p::{load_or_create_identity, spawn_p2p, P2pConfig, P2pEvent},
        peer_registry::{load_and_verify_peer_registry_now, PeerRegistryPolicy},
    },
};

fn init_logging() {
    let subscriber = FmtSubscriber::builder()
        .with_max_level(Level::INFO)
        .json()
        .finish();
    let _ = tracing::subscriber::set_global_default(subscriber);
}

fn read_config(path: &str) -> Result<NodeConfig> {
    let raw = std::fs::read_to_string(path)?;
    let cfg: NodeConfig = toml::from_str(&raw)?;
    Ok(cfg)
}

fn parse_hex_32(s: &str) -> Option<[u8; 32]> {
    let s = s.strip_prefix("0x").unwrap_or(s);
    let bytes = hex::decode(s).ok()?;
    if bytes.len() != 32 {
        return None;
    }
    let mut out = [0u8; 32];
    out.copy_from_slice(&bytes);
    Some(out)
}

#[derive(Clone)]
struct AppState {
    ready: Arc<AtomicBool>,
    metrics: Arc<Metrics>,
    metrics_token: Option<String>,
}

#[tokio::main]
async fn main() -> Result<()> {
    init_logging();

    // CLI:
    //  - Default: amunchain [configs/node.toml]
    //  - Utility: amunchain --print-peer-id <data_dir>
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.first().map(|s| s.as_str()) == Some("--print-peer-id") {
        let data_dir = args
            .get(1)
            .cloned()
            .unwrap_or_else(|| "data".to_string());
        std::fs::create_dir_all(&data_dir)?;
        let kp = load_or_create_identity(&data_dir)?;
        let pid = libp2p::PeerId::from(kp.public());
        println!("{pid}");
        return Ok(());
    }

    let cfg_path = args
        .get(0)
        .cloned()
        .unwrap_or_else(|| "configs/node.toml".to_string());
    let cfg = read_config(&cfg_path)?;

    // Resolve P2P allowlist:
    // 1) Use explicit `allow_peers` if provided.
    // 2) Else, if a signed registry is configured, load+verify and use it.
    // 3) Else, leave empty (dev/test only).
    let mut resolved_allow_peers: Vec<libp2p::PeerId> = cfg
        .p2p
        .allow_peers
        .iter()
        .filter_map(|s| s.parse().ok())
        .collect();

    if resolved_allow_peers.is_empty() {
        if let (Some(path), Some(pk_hex)) = (
            cfg.p2p.peer_registry_path.as_deref(),
            cfg.p2p.peer_registry_pubkey_hex.as_deref(),
        ) {
            let mut pol = PeerRegistryPolicy::default_with_now(0);
            pol.expected_network = Some(cfg.p2p.topic.as_str());
            pol.min_version = cfg.p2p.peer_registry_min_version;
            pol.max_age_ms = cfg.p2p.peer_registry_max_age_ms;
            pol.grace_ms = cfg.p2p.peer_registry_grace_ms;
            // In production builds we require freshness fields by default.
            pol.require_freshness_fields = if cfg!(feature = "production") {
                true
            } else {
                cfg.p2p.peer_registry_require_fresh
            };
            match load_and_verify_peer_registry_now(path, pk_hex, &pol) {
                Ok(list) => resolved_allow_peers = list,
                Err(e) => {
                    anyhow::bail!(
                        "Failed to load/verify peer registry (p2p.peer_registry_path): {e}.\n\
                        Either fix the registry signature/public key, or set [p2p].allow_peers explicitly."
                    );
                }
            }
        }
    }

    
    // ---- Security lints (production hardening) ----
    // In production builds we refuse insecure defaults that expand the attack surface.
    // `require_allow_peers` enables the same lint in non-production builds.
    if cfg!(feature = "production") || cfg.p2p.require_allow_peers {
        if resolved_allow_peers.is_empty() {
            anyhow::bail!(
                "Refusing to start: no P2P allowlist configured.\n\
                 Set [p2p].allow_peers, or configure [p2p].peer_registry_path + peer_registry_pubkey_hex."
            );
        }
    }

info!(node = %cfg.node.name, "Starting Amunchain Layer 0 node");

    // Ensure data dir exists.
    std::fs::create_dir_all(&cfg.node.data_dir)?;

    let _state = PersistentState::open(&cfg.node.data_dir)?;
    let metrics = Arc::new(Metrics::new()?);

    // Validator set from config
    let mut vals: BTreeSet<ValidatorId> = BTreeSet::new();
    for h in cfg.consensus.validators_hex.iter() {
        let pk = parse_hex_32(h).ok_or_else(|| anyhow::anyhow!("bad validator hex in config"))?;
        vals.insert(ValidatorId(pk.to_vec()));
    }
    let driver = Arc::new(Mutex::new(ConsensusDriver::new(vals)?));

    let p2p_cfg = P2pConfig {
        listen_addr: cfg.p2p.listen_addr.parse()?,
        consensus_topic: cfg.p2p.topic.clone(),
        max_msg_per_sec: cfg.p2p.max_msg_per_sec,
        max_peers_per_ip: cfg.p2p.max_peers_per_ip,
        data_dir: cfg.node.data_dir.clone(),
        bootstrap: cfg
            .p2p
            .bootstrap
            .iter()
            .filter_map(|s| s.parse().ok())
            .collect(),
        allow_peers: resolved_allow_peers.clone(),
    };

    let (mut p2p_node, mut p2p_events, p2p_join) = spawn_p2p(p2p_cfg, metrics.clone())?;

    let ready = Arc::new(AtomicBool::new(false));
    let peer_count = Arc::new(AtomicUsize::new(0));

    // Spawn a simple bridge that forwards inbound consensus messages to the driver.
    let driver2 = driver.clone();
    let ready2 = ready.clone();
    let peer_count2 = peer_count.clone();
    tokio::spawn(async move {
        while let Some(ev) = p2p_events.recv().await {
            match ev {
                P2pEvent::PeerConnected(_) => {
                    peer_count2.fetch_add(1, Ordering::Relaxed);
                }
                P2pEvent::PeerDisconnected(_) => {
                    peer_count2.fetch_sub(1, Ordering::Relaxed);
                }
            }
        }
        ready2.store(false, Ordering::Relaxed);
        let _ = driver2.lock().await;
    });

    // Mark ready once the p2p task is up.
    ready.store(true, Ordering::Relaxed);

    // HTTP server (metrics + basic health)
    let metrics_token = std::env::var("AMUNCHAIN_METRICS_TOKEN")
        .ok()
        .and_then(|v| {
            let t = v.trim().to_string();
            if t.is_empty() { None } else { Some(t) }
        });

    let state = AppState {
        ready: ready.clone(),
        metrics: metrics.clone(),
        metrics_token,
    };

    let app = Router::new()
        .route("/healthz", get(healthz))
        .route("/readyz", get(readyz))
        .route("/metrics", get(metrics))
        .with_state(state);

    let addr: SocketAddr = cfg.http.listen_addr.parse()?;
    info!(%addr, "http: listening");

    let server = axum::Server::bind(&addr).serve(app.into_make_service());

    tokio::select! {
        _ = server => {},
        _ = p2p_join => {},
        _ = tokio::signal::ctrl_c() => {
            info!("shutdown: ctrl-c");
        }
    }

    // Best-effort stop p2p.
    let _ = p2p_node.shutdown().await;

    Ok(())
}

async fn healthz() -> impl IntoResponse {
    (StatusCode::OK, "ok")
}

async fn readyz(State(state): State<AppState>) -> impl IntoResponse {
    if state.ready.load(Ordering::Relaxed) {
        (StatusCode::OK, "ready")
    } else {
        (StatusCode::SERVICE_UNAVAILABLE, "not ready")
    }
}

async fn metrics(State(state): State<AppState>, headers: HeaderMap) -> impl IntoResponse {
    if let Some(expected) = &state.metrics_token {
        match headers.get(AUTHORIZATION).and_then(|h| h.to_str().ok()) {
            Some(v) if v == format!("Bearer {}", expected) => {}
            _ => return (StatusCode::UNAUTHORIZED, "unauthorized").into_response(),
        }
    }

    let families = state.metrics.registry.gather();
    let mut buffer = vec![];
    let encoder = TextEncoder::new();
    if encoder.encode(&families, &mut buffer).is_err() {
        return (StatusCode::INTERNAL_SERVER_ERROR, "encode error").into_response();
    }
    (StatusCode::OK, String::from_utf8_lossy(&buffer).to_string()).into_response()
}