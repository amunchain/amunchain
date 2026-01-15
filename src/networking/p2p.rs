
#[allow(missing_docs)]
// Copyright (c) 2026 Amunchain
// Licensed under the Apache License, Version 2.0

// P2P subsystem (libp2p): persistent identity + gossipsub consensus topic.

// This replaces the previous build-stub with a minimal but real networking loop.
// - Outbound: ConsensusMsg -> gossipsub publish (bincode)
// - Inbound: gossipsub message -> ConsensusMsg -> inbound channel
// - Allowlist: if allow_peers non-empty, disconnect peers not in allowlist
// - Metrics: peer count gauge + banned counter + invalid msg counter


use crate::{core::types::ConsensusMsg, monitoring::metrics::Metrics};
use std::{
    collections::HashSet,
    path::Path,
    sync::Arc,
    time::Duration,
};

use thiserror::Error;
use tokio::sync::mpsc;
use tracing::{info, warn};

use libp2p::{
    core::upgrade,
    gossipsub::{self, IdentTopic, MessageAuthenticity},
    identify,     noise, ping,
    swarm::{NetworkBehaviour, Swarm, SwarmEvent},
    tcp, yamux, Multiaddr, PeerId, Transport,
};

use libp2p::futures::StreamExt;
use libp2p::swarm::Config as SwarmConfig;

/// Events emitted by the P2P node.
#[derive(Clone, Debug)]
pub enum P2pEvent {
    /// Peer connected.
    PeerConnected(Vec<u8>),
    /// Peer disconnected.
    PeerDisconnected(Vec<u8>),
}

/// Receiver of P2P events.
pub type EventRx = mpsc::Receiver<P2pEvent>;

#[derive(Debug, Error)]
pub enum P2pError {
    #[error("io")]
    Io,
    #[error("config")]
    Config,
}

/// Runtime configuration for the P2P subsystem.
#[derive(Clone, Debug)]
pub struct P2pConfig {
    /// Listen address as string (e.g. "/ip4/0.0.0.0/tcp/4001").
    pub listen_addr: String,
    /// Gossipsub topic for consensus messages.
    pub consensus_topic: String,
    /// Max messages/sec per peer. (best-effort placeholder)
    pub max_msg_per_sec: u32,
    /// Maximum peers allowed from the same IP. (best-effort placeholder)
    pub max_peers_per_ip: usize,
    /// Data directory used for persistent identity.
    pub data_dir: String,
    /// Bootstrap peers.
    pub bootstrap: Vec<String>,
    /// Optional allowlist of peer ids (empty => allow all).
    pub allow_peers: Vec<String>,
}

/// Handle to interact with P2P.
pub struct P2pNode {
    inbound_rx: mpsc::Receiver<(Vec<u8>, ConsensusMsg)>,
    outbound_tx: mpsc::Sender<ConsensusMsg>,
}

impl P2pNode {
    /// Inbound consensus messages (peer_id_bytes, msg).
    pub fn inbound(&mut self) -> &mut mpsc::Receiver<(Vec<u8>, ConsensusMsg)> {
        &mut self.inbound_rx
    }

    /// Outbound channel for broadcasting consensus messages.
    pub fn outbound(&self) -> mpsc::Sender<ConsensusMsg> {
        self.outbound_tx.clone()
    }
}

#[derive(Debug)]
enum BehaviourEvent {
    Gossipsub(gossipsub::Event),
    Identify(()),
    Ping(()),
}
impl From<gossipsub::Event> for BehaviourEvent {
    fn from(e: gossipsub::Event) -> Self {
        Self::Gossipsub(e)
    }
}
impl From<identify::Event> for BehaviourEvent {
    fn from(_e: identify::Event) -> Self {
        Self::Identify(())
    }
}
impl From<ping::Event> for BehaviourEvent {
    fn from(_e: ping::Event) -> Self {
        Self::Ping(())
    }
}

#[derive(NetworkBehaviour)]
#[behaviour(to_swarm = "BehaviourEvent")]
struct Behaviour {
    gossipsub: gossipsub::Behaviour,
    identify: identify::Behaviour,
    ping: ping::Behaviour,
}

fn ensure_dir(path: &str) -> Result<(), P2pError> {
    let p = Path::new(path);
    if !p.exists() {
        std::fs::create_dir_all(p).map_err(|_| P2pError::Io)?;
    }
    Ok(())
}

/// Spawn the P2P task (real libp2p).
pub fn spawn_p2p(
    cfg: P2pConfig,
    metrics: Arc<Metrics>,
) -> Result<(P2pNode, EventRx, tokio::task::JoinHandle<()>), P2pError> {
    ensure_dir(&cfg.data_dir)?;

    // Persistent identity lives in networking::p2p_identity (already in your project).
    let (local_peer_id, id_keys) = crate::networking::p2p_identity::load_or_create_identity(&cfg.data_dir)
        .map_err(|_| P2pError::Io)?;

    // Build allowlist set.
    let mut allow_set: HashSet<PeerId> = HashSet::new();
    for s in cfg.allow_peers.iter().map(|x| x.trim()).filter(|x| !x.is_empty()) {
        match s.parse::<PeerId>() {
            Ok(pid) => {
                allow_set.insert(pid);
            }
            Err(_) => {
                warn!(peer = %s, "invalid allow_peers entry; ignoring");
            }
        }
    }

    // Channels
    let (in_tx, in_rx) = mpsc::channel::<(Vec<u8>, ConsensusMsg)>(1024);
    let (out_tx, mut out_rx) = mpsc::channel::<ConsensusMsg>(1024);
    let (ev_tx, ev_rx) = mpsc::channel::<P2pEvent>(128);

    let listen_addr = cfg.listen_addr.clone();
    let topic_name = cfg.consensus_topic.clone();
    let bootstrap = cfg.bootstrap.clone();

    // Spawn swarm loop
    let join = tokio::spawn(async move {
        // --- Transport (TCP + Noise + Yamux) ---
        let noise_keys = match noise::Config::new(&id_keys) {
            Ok(v) => v,
            Err(_) => {
                warn!("failed to build noise config");
                return;
            }
        };

        let transport = tcp::tokio::Transport::new(tcp::Config::default().nodelay(true))
            .upgrade(upgrade::Version::V1)
            .authenticate(noise_keys)
            .multiplex(yamux::Config::default())
            .boxed();

        // --- Gossipsub ---
        let gcfg = gossipsub::ConfigBuilder::default()
            .validation_mode(gossipsub::ValidationMode::Permissive)
            .heartbeat_interval(Duration::from_secs(1))
            .build()
            .unwrap_or_else(|_| gossipsub::Config::default());

        let mut gossipsub = match gossipsub::Behaviour::new(
            MessageAuthenticity::Signed(id_keys.clone()),
            gcfg,
        ) {
            Ok(v) => v,
            Err(_) => {
                warn!("failed to create gossipsub behaviour");
                return;
            }
        };

        let topic = IdentTopic::new(topic_name.clone());
        if let Err(e) = gossipsub.subscribe(&topic) {
            warn!(err = ?e, "failed to subscribe topic");
        }

        // Identify + Ping
        let identify = identify::Behaviour::new(identify::Config::new(
            "amunchain/1.0.0".to_string(),
            id_keys.public(),
        ));

        let ping = ping::Behaviour::new(
            ping::Config::new()
                .with_interval(Duration::from_secs(10))
                .with_timeout(Duration::from_secs(20)),
        );

        let behaviour = Behaviour { gossipsub, identify, ping };

        let mut swarm = Swarm::new(transport, behaviour, local_peer_id, SwarmConfig::with_tokio_executor());

        // Listen
        let listen: Multiaddr = match listen_addr.parse() {
            Ok(v) => v,
            Err(_) => {
                warn!(addr = %listen_addr, "bad listen_addr");
                return;
            }
        };

        if let Err(e) = swarm.listen_on(listen) {
            warn!(err = ?e, "listen_on failed");
            return;
        }

        // Bootstrap
        for b in bootstrap.iter().map(|x| x.trim()).filter(|x| !x.is_empty()) {
            match b.parse::<Multiaddr>() {
                Ok(ma) => {
                    if let Err(e) = swarm.dial(ma.clone()) {
                        warn!(boot = %b, err = ?e, "dial bootstrap failed");
                    } else {
                        info!(boot = %b, "dialing bootstrap");
                    }
                }
                Err(_) => warn!(boot = %b, "bad bootstrap addr; skipping"),
            }
        }

        info!(%local_peer_id, topic = %topic_name, "p2p loop started");

        // Ensure gauge starts at 0
        metrics.p2p_peers.set(0);

        loop {
            tokio::select! {
                maybe_msg = out_rx.recv() => {
                    match maybe_msg {
                        Some(msg) => {
                            match bincode::serialize(&msg) {
                                Ok(bytes) => {
                                    if let Err(e) = swarm.behaviour_mut().gossipsub.publish(topic.clone(), bytes) {
                                        warn!(err=?e, "gossipsub publish failed");
                                    }
                                }
                                Err(_) => {
                                    warn!("failed to serialize ConsensusMsg");
                                    metrics.p2p_invalid_msg_total.inc();
                                }
                            }
                        }
                        None => {
                            warn!("outbound channel closed; stopping p2p task");
                            break;
                        }
                    }
                }

                ev = swarm.select_next_some() => {
                    match ev {
                        SwarmEvent::NewListenAddr { address, .. } => {
                            info!(addr=%address, "listening");
                        }

                        SwarmEvent::ConnectionEstablished { peer_id, .. } => {
                            if !allow_set.is_empty() && !allow_set.contains(&peer_id) {
                                warn!(%peer_id, "peer not in allowlist; disconnecting");
                                metrics.p2p_banned_total.inc();
                                let _ = swarm.disconnect_peer_id(peer_id);
                                continue;
                            }
                            metrics.p2p_peers.inc();
                            let _ = ev_tx.send(P2pEvent::PeerConnected(peer_id.to_bytes())).await;
                            info!(%peer_id, "peer connected");
                        }

                        SwarmEvent::ConnectionClosed { peer_id, .. } => {
                            metrics.p2p_peers.dec();
                            let _ = ev_tx.send(P2pEvent::PeerDisconnected(peer_id.to_bytes())).await;
                            info!(%peer_id, "peer disconnected");
                        }

                        SwarmEvent::Behaviour(BehaviourEvent::Gossipsub(gossipsub::Event::Message { propagation_source, message, .. })) => {
                            if !allow_set.is_empty() && !allow_set.contains(&propagation_source) {
                                warn!(%propagation_source, "message from non-allowlisted peer; dropping");
                                metrics.p2p_banned_total.inc();
                                continue;
                            }
                            match bincode::deserialize::<ConsensusMsg>(&message.data) {
                                Ok(msg) => {
                                    let _ = in_tx.send((propagation_source.to_bytes(), msg)).await;
                                }
                                Err(_) => {
                                    warn!(%propagation_source, "invalid consensus msg decode");
                                    metrics.p2p_invalid_msg_total.inc();
                                }
                            }
                        }

                        SwarmEvent::Behaviour(BehaviourEvent::Identify(_)) => {}
                        SwarmEvent::Behaviour(BehaviourEvent::Ping(_)) => {}

                        _ => {}
                    }
                }
            }
        }
    });

    Ok((
        P2pNode {
            inbound_rx: in_rx,
            outbound_tx: out_tx,
        },
        ev_rx,
        join,
    ))
}
