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

//! libp2p P2P networking with Noise encryption, anti-replay, rate limits, and peer scoring.

use crate::{
    core::types::{decode_canonical_limited, encode_canonical, ConsensusMsg},
    monitoring::metrics::Metrics,
    networking::peer_score::{Decision, PeerScore, ScoreParams},
};
use libp2p::{
    core::upgrade,
    gossipsub::{self, IdentTopic, MessageAuthenticity},
    identify, identity,
    noise, ping,
    swarm::{NetworkBehaviour, Swarm, SwarmEvent},
    tcp, yamux, Multiaddr, PeerId, Transport,
};
use ring::digest;
use std::{
    collections::{BTreeMap, BTreeSet, VecDeque},
    fs,
    net::IpAddr,
    path::{Path, PathBuf},
    sync::Arc,
    time::{Duration, Instant},
};
use thiserror::Error;
use tokio::sync::mpsc;
use tracing::{info, warn};

const MAX_WIRE_MSG_BYTES: usize = 256 * 1024;
const REPLAY_CACHE_MAX: usize = 8192;

/// Compute an effective per-peer message rate limit based on reputation score.
///
/// Design goals:
/// - Deterministic and integer-only (no floats).
/// - Conservative when score is negative (throttle harder).
/// - Reward consistently-good peers with a higher cap.
fn effective_rate_limit(base: u32, score: i32) -> u32 {
    // Clamp score to expected range from PeerScore params.
    let s = score.clamp(-200, 200);

    // Piecewise scaling to keep behavior predictable.
    //  score <= -100  => 0.25x
    //  -100..0        => 0.5x
    //  0..50          => 1.0x
    //  50..150        => 2.0x
    //  >=150          => 4.0x
    let scaled = if s <= -100 {
        base / 4
    } else if s < 0 {
        base / 2
    } else if s < 50 {
        base
    } else if s < 150 {
        base.saturating_mul(2)
    } else {
        base.saturating_mul(4)
    };

    scaled.max(1)
}

#[derive(Debug, Error)]
pub enum P2pError {
    #[error("gossipsub config")]
    Gossip,
    #[error("transport")]
    Transport,
    #[error("bad multiaddr")]
    BadMultiaddr,
    #[error("io")]
    Io,
    #[error("identity decode")]
    Identity,
}

/// Runtime configuration for the P2P subsystem.
#[derive(Clone, Debug)]
pub struct P2pConfig {
    /// Multiaddr to listen on (e.g. /ip4/0.0.0.0/tcp/30333).
    pub listen_addr: Multiaddr,
    /// Gossipsub topic for consensus messages.
    pub consensus_topic: String,
    /// Max messages/sec per peer (best-effort rate limiting).
    pub max_msg_per_sec: u32,
    /// Maximum peers allowed from the same IP (best-effort).
    pub max_peers_per_ip: usize,
    /// Data directory used for persistent identity (p2p_identity.key).
    pub data_dir: String,
    /// Bootstrap peer multiaddrs to dial on startup.
    pub bootstrap: Vec<Multiaddr>,
    /// Optional allowlist of PeerIds (empty => allow all).
    pub allow_peers: Vec<PeerId>,
}

/// Events emitted by the P2P node.
#[derive(Clone, Debug)]
pub enum P2pEvent {
    /// Peer connected.
    PeerConnected(PeerId),
    /// Peer disconnected.
    PeerDisconnected(PeerId),
}

/// Receiver of P2P events.
pub type EventRx = mpsc::Receiver<P2pEvent>;

/// Handle to interact with P2P.
pub struct P2pNode {
    inbound_rx: mpsc::Receiver<(PeerId, ConsensusMsg)>,
    outbound_tx: mpsc::Sender<ConsensusMsg>,
}

impl P2pNode {
    /// Inbound consensus messages (peer_id, msg).
    pub fn inbound(&mut self) -> &mut mpsc::Receiver<(PeerId, ConsensusMsg)> {
        &mut self.inbound_rx
    }

    /// Outbound channel for broadcasting consensus messages.
    pub fn outbound(&self) -> mpsc::Sender<ConsensusMsg> {
        self.outbound_tx.clone()
    }
}

#[derive(Clone)]
struct RateWindow {
    start: Instant,
    count: u32,
}
impl RateWindow {
    fn new(now: Instant) -> Self {
        Self { start: now, count: 0 }
    }
    fn allow(&mut self, now: Instant, limit: u32) -> bool {
        if now.duration_since(self.start) >= Duration::from_secs(1) {
            self.start = now;
            self.count = 0;
        }
        if self.count >= limit {
            return false;
        }
        self.count = self.count.saturating_add(1);
        true
    }

#[derive(Clone, Copy, Debug)]
struct BackoffState {
    until: Instant,
    strikes: u32,
}

impl BackoffState {
    fn new(now: Instant) -> Self {
        Self { until: now, strikes: 0 }
    }

    fn should_drop(&self, now: Instant) -> bool {
        now < self.until
    }

    fn bump(&mut self, peer: &PeerId, now: Instant, weight: u32, startup: Instant) {
        // Exponential backoff with deterministic jitter.
        self.strikes = self.strikes.saturating_add(weight).min(100);

        let exp = (self.strikes / 5).min(8); // 0..8
        let base_ms: u64 = 50u64.saturating_mul(1u64 << exp);
        let base_ms = base_ms.min(5_000);

        // Deterministic jitter: hash(peer_id || strikes || uptime_ms) mod 250ms
        let uptime_ms = now.duration_since(startup).as_millis() as u64;
        let mut buf = Vec::with_capacity(64);
        buf.extend_from_slice(peer.to_bytes().as_slice());
        buf.extend_from_slice(&self.strikes.to_be_bytes());
        buf.extend_from_slice(&uptime_ms.to_be_bytes());
        let h = digest::digest(&digest::SHA256, &buf);
        let mut b = [0u8; 8];
        b.copy_from_slice(&h.as_ref()[..8]);
        let jitter = u64::from_be_bytes(b) % 250;

        self.until = now + Duration::from_millis(base_ms + jitter);
    }
}

}

#[derive(Debug)]
pub enum BehaviourEvent {
    Gossipsub(gossipsub::Event),
    Identify(identify::Event),
    Ping(ping::Event),
}
impl From<gossipsub::Event> for BehaviourEvent {
    fn from(e: gossipsub::Event) -> Self { Self::Gossipsub(e) }
}
impl From<identify::Event> for BehaviourEvent {
    fn from(e: identify::Event) -> Self { Self::Identify(e) }
}
impl From<ping::Event> for BehaviourEvent {
    fn from(e: ping::Event) -> Self { Self::Ping(e) }
}

#[derive(NetworkBehaviour)]
#[behaviour(to_swarm = "BehaviourEvent")]
struct Behaviour {
    gossipsub: gossipsub::Behaviour,
    identify: identify::Behaviour,
    ping: ping::Behaviour,
}

struct ReplayCache {
    q: VecDeque<[u8; 32]>,
    set: BTreeSet<[u8; 32]>,
    cap: usize,
}
impl ReplayCache {
    fn new(cap: usize) -> Self {
        Self { q: VecDeque::new(), set: BTreeSet::new(), cap }
    }
    fn seen_or_insert(&mut self, id: [u8; 32]) -> bool {
        if self.set.contains(&id) { return true; }
        self.set.insert(id);
        self.q.push_back(id);
        while self.q.len() > self.cap {
            if let Some(old) = self.q.pop_front() {
                self.set.remove(&old);
            }
        }
        false
    }
}

/// Load or create persistent libp2p identity in data_dir/p2p_identity.key.
pub fn load_or_create_identity(data_dir: &str) -> Result<identity::Keypair, P2pError> {
    let mut p = PathBuf::from(data_dir);
    p.push("p2p_identity.key");

    if p.exists() {
        let bytes = fs::read(&p).map_err(|_| P2pError::Io)?;
        let kp = identity::Keypair::from_protobuf_encoding(&bytes).map_err(|_| P2pError::Identity)?;
        return Ok(kp);
    }

    let kp = identity::Keypair::generate_ed25519();
    let bytes = kp.to_protobuf_encoding();
    if let Some(dir) = Path::new(&p).parent() {
        fs::create_dir_all(dir).map_err(|_| P2pError::Io)?;
    }
    // Atomic write to avoid partial identity corruption on crash.
    let mut tmp = p.clone();
    tmp.set_extension("tmp");
    {
        use std::io::Write;
        let mut f = fs::OpenOptions::new()
            .create(true)
            .truncate(true)
            .write(true)
            .open(&tmp)
            .map_err(|_| P2pError::Io)?;
        f.write_all(&bytes).map_err(|_| P2pError::Io)?;
        let _ = f.sync_all();
    }

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(&tmp, fs::Permissions::from_mode(0o600));
    }

    fs::rename(&tmp, &p).map_err(|_| P2pError::Io)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(&p, fs::Permissions::from_mode(0o600));
    }
    Ok(kp)
}

/// Spawn the P2P node.
pub fn spawn_p2p(cfg: P2pConfig, metrics: Arc<Metrics>) -> Result<(P2pNode, EventRx, tokio::task::JoinHandle<()>), P2pError> {
    let id_keys = load_or_create_identity(&cfg.data_dir)?;
    let local_peer_id = PeerId::from(id_keys.public());
    info!(peer_id = %local_peer_id, "p2p: starting");

    let noise_cfg = noise::Config::new(&id_keys).map_err(|_| P2pError::Transport)?;
    let tcp_transport = tcp::tokio::Transport::new(tcp::Config::default().nodelay(true));

    let transport = tcp_transport
        .upgrade(upgrade::Version::V1)
        .authenticate(noise_cfg)
        .multiplex(yamux::Config::default())
        .boxed();

    let gs_cfg = gossipsub::ConfigBuilder::default()
        .max_transmit_size(MAX_WIRE_MSG_BYTES)
        .heartbeat_interval(Duration::from_secs(1))
        .validation_mode(gossipsub::ValidationMode::Strict)
        .build()
        .map_err(|_| P2pError::Gossip)?;

    let mut gossipsub = gossipsub::Behaviour::new(
        MessageAuthenticity::Signed(id_keys.clone()),
        gs_cfg,
    )
    .map_err(|_| P2pError::Transport)?;

    let topic = IdentTopic::new(cfg.consensus_topic.clone());
    gossipsub.subscribe(&topic).map_err(|_| P2pError::Transport)?;

    let identify = identify::Behaviour::new(identify::Config::new(
        "amunchain/1.0".to_string(),
        id_keys.public(),
    ));
    let ping = ping::Behaviour::new(ping::Config::new().with_interval(Duration::from_secs(10)));

    let behaviour = Behaviour { gossipsub, identify, ping };
    let mut swarm = Swarm::new(transport, behaviour, local_peer_id);

    Swarm::listen_on(&mut swarm, cfg.listen_addr.clone()).map_err(|_| P2pError::BadMultiaddr)?;

    for a in cfg.bootstrap.iter() {
        let _ = swarm.dial(a.clone());
    }

    let allowlist: BTreeSet<PeerId> = cfg.allow_peers.iter().cloned().collect();

    let (in_tx, in_rx) = mpsc::channel::<(PeerId, ConsensusMsg)>(4096);
    let (out_tx, mut out_rx) = mpsc::channel::<ConsensusMsg>(4096);
    let (ev_tx, ev_rx) = mpsc::channel::<P2pEvent>(1024);

    let mut peers_by_ip: BTreeMap<IpAddr, BTreeSet<PeerId>> = BTreeMap::new();
    let startup = Instant::now();

    let mut peer_rates: BTreeMap<PeerId, RateWindow> = BTreeMap::new();
    let mut peer_backoff: BTreeMap<PeerId, BackoffState> = BTreeMap::new();
    let mut replay = ReplayCache::new(REPLAY_CACHE_MAX);

    let mut scoring = PeerScore::new(ScoreParams::default());

    let max_peers_per_ip = cfg.max_peers_per_ip;
    let max_msg_per_sec = cfg.max_msg_per_sec;

    let join = tokio::spawn(async move {
        loop {
            tokio::select! {
                maybe = out_rx.recv() => {
                    match maybe {
                        None => break,
                        Some(msg) => {
                            let bytes = match encode_canonical(&msg) { Ok(b) => b, Err(_) => continue };
                            if bytes.len() > MAX_WIRE_MSG_BYTES { continue; }
                            let _ = swarm.behaviour_mut().gossipsub.publish(topic.clone(), bytes);
                        }
                    }
                }

                ev = swarm.select_next_some() => {
                    match ev {
                        SwarmEvent::NewListenAddr { address, .. } => {
                            info!(%address, "p2p: listening");
                        }

                        SwarmEvent::ConnectionEstablished { peer_id, endpoint, .. } => {
                            if !allowlist.is_empty() && !allowlist.contains(&peer_id) {
                                warn!(%peer_id, "p2p: peer not in allowlist, disconnecting");
                                swarm.disconnect_peer_id(peer_id);
                                continue;
                            }

                            let now = Instant::now();
                            if scoring.is_banned(&peer_id, now) {
                                metrics.p2p_banned_total.inc();
                                swarm.disconnect_peer_id(peer_id);
                                continue;
                            }

                            let maybe_ip = endpoint.get_remote_address().iter().find_map(|p| {
                                if let libp2p::multiaddr::Protocol::Ip4(ip) = p { Some(IpAddr::V4(ip)) }
                                else if let libp2p::multiaddr::Protocol::Ip6(ip) = p { Some(IpAddr::V6(ip)) }
                                else { None }
                            });

                            if let Some(ip) = maybe_ip {
                                let entry = peers_by_ip.entry(ip).or_default();
                                if entry.len() >= max_peers_per_ip {
                                    warn!(%peer_id, %ip, "p2p: max peers per ip reached, disconnecting");
                                    swarm.disconnect_peer_id(peer_id);
                                } else {
                                    entry.insert(peer_id);
                                    peer_rates.entry(peer_id).or_insert_with(|| RateWindow::new(now));
                                    peer_backoff.entry(peer_id).or_insert_with(|| BackoffState::new(now));
                                    let _ = ev_tx.send(P2pEvent::PeerConnected(peer_id)).await;
                                }
                            } else {
                                peer_rates.entry(peer_id).or_insert_with(|| RateWindow::new(now));
                                let _ = ev_tx.send(P2pEvent::PeerConnected(peer_id)).await;
                            }
                        }

                        SwarmEvent::ConnectionClosed { peer_id, endpoint, .. } => {
                            let maybe_ip = endpoint.get_remote_address().iter().find_map(|p| {
                                if let libp2p::multiaddr::Protocol::Ip4(ip) = p { Some(IpAddr::V4(ip)) }
                                else if let libp2p::multiaddr::Protocol::Ip6(ip) = p { Some(IpAddr::V6(ip)) }
                                else { None }
                            });
                            if let Some(ip) = maybe_ip {
                                if let Some(set) = peers_by_ip.get_mut(&ip) {
                                    set.remove(&peer_id);
                                    if set.is_empty() { peers_by_ip.remove(&ip); }
                                }
                            }
                            peer_rates.remove(&peer_id);
                            peer_backoff.remove(&peer_id);
                            let _ = ev_tx.send(P2pEvent::PeerDisconnected(peer_id)).await;
                        }

                        SwarmEvent::Behaviour(BehaviourEvent::Gossipsub(gossipsub::Event::Message { propagation_source, message, .. })) => {
                            let now = Instant::now();

                            // Fast-path drop if peer is currently in backoff.
                            if let Some(b) = peer_backoff.get(&propagation_source) {
                                if b.should_drop(now) {
                                    metrics.p2p_reputation_throttled_total.inc();
                                    continue;
                                }
                            }

                            // Hard cap before hashing/decoding (defense-in-depth).
                            if message.data.len() > MAX_WIRE_MSG_BYTES {
                                metrics.p2p_invalid_msg_total.inc();
                                peer_backoff.entry(propagation_source).or_insert_with(|| BackoffState::new(now))
                                    .bump(&propagation_source, now, 10, startup);
                                let d = scoring.observe_bad(propagation_source, now, 10);
                                if d == Decision::Ban {
                                    metrics.p2p_banned_total.inc();
                                    swarm.disconnect_peer_id(propagation_source);
                                }
                                continue;
                            }


                            if scoring.is_banned(&propagation_source, now) {
                                metrics.p2p_banned_total.inc();
                                swarm.disconnect_peer_id(propagation_source);
                                continue;
                            }

                            // Dynamic throttling based on peer reputation score.
                            let score = scoring.score(&propagation_source, now);
                            let eff_limit = effective_rate_limit(max_msg_per_sec, score);

                            let allow = peer_rates
                                .entry(propagation_source)
                                .or_insert_with(|| RateWindow::new(now))
                                .allow(now, eff_limit);

                            if !allow {
                                if eff_limit < max_msg_per_sec {
                                    metrics.p2p_reputation_throttled_total.inc();
                                } else {
                                    metrics.p2p_rate_limited_total.inc();
                                }
                                peer_backoff.entry(propagation_source).or_insert_with(|| BackoffState::new(now))
                                    .bump(&propagation_source, now, 2, startup);
                                let d = scoring.observe_bad(propagation_source, now, 5);
                                if d == Decision::Ban {
                                    metrics.p2p_banned_total.inc();
                                    swarm.disconnect_peer_id(propagation_source);
                                }
                                continue;
                            }

                            let h = digest::digest(&digest::SHA256, &message.data);
                            let mut mid = [0u8; 32];
                            mid.copy_from_slice(h.as_ref());
                            if replay.seen_or_insert(mid) {
                                metrics.p2p_replay_dropped_total.inc();
                                peer_backoff.entry(propagation_source).or_insert_with(|| BackoffState::new(now))
                                    .bump(&propagation_source, now, 1, startup);
                                let _ = scoring.observe_bad(propagation_source, now, 1);
                                continue;
                            }

                            let msg = match decode_canonical_limited::<ConsensusMsg>(&message.data, MAX_WIRE_MSG_BYTES) {
                                Ok(m) => m,
                                Err(_) => {
                                    metrics.p2p_invalid_msg_total.inc();
                                    peer_backoff.entry(propagation_source).or_insert_with(|| BackoffState::new(now))
                                        .bump(&propagation_source, now, 5, startup);
                                    let d = scoring.observe_bad(propagation_source, now, 10);
                                    if d == Decision::Ban {
                                        metrics.p2p_banned_total.inc();
                                        swarm.disconnect_peer_id(propagation_source);
                                    }
                                    continue;
                                }
                            };

                            // Production cost-shifting: drop legacy (epoch/counter/timestamp=0) messages on the wire
                            // before they reach the consensus engine, to avoid wasting expensive signature checks under DoS.
                            if cfg!(feature = "production") {
                                let is_legacy = match &msg {
                                    ConsensusMsg::Vote(v) => v.epoch == 0 || v.msg_counter == 0 || v.sent_ts_ms == 0,
                                    ConsensusMsg::Commit(c) => c.epoch == 0 || c.msg_counter == 0 || c.sent_ts_ms == 0,
                                };
                                if is_legacy {
                                    metrics.p2p_invalid_msg_total.inc();
                                    peer_backoff
                                        .entry(propagation_source)
                                        .or_insert_with(|| BackoffState::new(now))
                                        .bump(&propagation_source, now, 2, startup);
                                    let _ = scoring.observe_bad(propagation_source, now, 2);
                                    continue;
                                }
                            }

                            let _ = scoring.observe_good(propagation_source, now, 1);
                            let _ = in_tx.send((propagation_source, msg)).await;
                        }

                        _ => {}
                    }
                }
            }
        }
    });

    Ok((P2pNode { inbound_rx: in_rx, outbound_tx: out_tx }, ev_rx, join))
}
