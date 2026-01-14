// Copyright (c) 2026 Amunchain
// Licensed under the Apache License, Version 2.0

//! P2P subsystem interface.
//!
//! This file is intentionally written as a **build-stable stub** to unblock compilation.
//! It preserves the public API expected by the rest of the project (P2pConfig, P2pNode,
//! spawn_p2p, events), while networking internals will be reintroduced safely in a later patch.

#![forbid(unsafe_code)]

use crate::{core::types::ConsensusMsg, monitoring::metrics::Metrics};
use std::sync::Arc;
use thiserror::Error;
use tokio::sync::mpsc;

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
///
/// NOTE: We keep fields that the rest of the code likely depends on, but we do not require
/// libp2p types here to keep compilation stable.
#[derive(Clone, Debug)]
pub struct P2pConfig {
    /// Listen address as string (e.g. "/ip4/0.0.0.0/tcp/4001").
    pub listen_addr: String,
    /// Gossipsub topic for consensus messages.
    pub consensus_topic: String,
    /// Max messages/sec per peer.
    pub max_msg_per_sec: u32,
    /// Maximum peers allowed from the same IP.
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
    /// Inbound consensus messages (peer_id, msg).
    pub fn inbound(&mut self) -> &mut mpsc::Receiver<(Vec<u8>, ConsensusMsg)> {
        &mut self.inbound_rx
    }

    /// Outbound channel for broadcasting consensus messages.
    pub fn outbound(&self) -> mpsc::Sender<ConsensusMsg> {
        self.outbound_tx.clone()
    }
}

/// Spawn the P2P task.
///
/// Current behavior:
/// - Provides channels for inbound/outbound consensus messages.
/// - Spawns a lightweight async task that drains outbound messages (no-op).
///
/// This unblocks compilation while we reintroduce libp2p implementation safely.
pub fn spawn_p2p(
    _cfg: P2pConfig,
    _metrics: Arc<Metrics>,
) -> Result<(P2pNode, EventRx, tokio::task::JoinHandle<()>), P2pError> {
    let (in_tx, in_rx) = mpsc::channel::<(Vec<u8>, ConsensusMsg)>(1024);
    let (out_tx, mut out_rx) = mpsc::channel::<ConsensusMsg>(1024);
    let (ev_tx, ev_rx) = mpsc::channel::<P2pEvent>(128);

    // Spawn a no-op task: drain outbound, keep node alive.
    let join = tokio::spawn(async move {
        // Example: emit a synthetic "connected" event once.
        let _ = ev_tx.send(P2pEvent::PeerConnected(vec![])).await;

        while let Some(_msg) = out_rx.recv().await {
            // In real implementation, this is where we broadcast.
            // For now: no-op.
            let _ = &in_tx;
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
