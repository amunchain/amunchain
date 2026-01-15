#![forbid(unsafe_code)]
#![warn(missing_docs)]

//! Amunchain node entrypoint (systemd-friendly).
//! Starts P2P and keeps the process alive.

use std::path::Path;
use std::sync::Arc;

use tracing::{info, warn};

fn env(key: &str, default: &str) -> String {
    std::env::var(key).unwrap_or_else(|_| default.to_string())
}

/// Extract node index from a path like `/srv/amunchain/node3/data`.
fn node_index_from_data_dir(data_dir: &str) -> u16 {
    for part in Path::new(data_dir).components() {
        let s = part.as_os_str().to_string_lossy();
        if let Some(rest) = s.strip_prefix("node") {
            if let Ok(n) = rest.parse::<u16>() {
                if (1..=99).contains(&n) {
                    return n;
                }
            }
        }
    }
    1
}

#[tokio::main]
async fn main() {
    let _ = tracing_subscriber::fmt()
        .with_target(false)
        .with_level(true)
        .compact()
        .try_init();

    let data_dir = env("AMUN_DATA_DIR", "./data");
    let node_idx = node_index_from_data_dir(&data_dir);

    // per-node ports: node1=4001, node2=4002, ...
    let p2p_port: u16 = 4000 + node_idx;
    let listen_addr = format!("/ip4/0.0.0.0/tcp/{p2p_port}");

    // API now uses `consensus_topic`
    let consensus_topic = env("AMUN_P2P_TOPIC", "amunchain-consensus");

    // Bootstrap nodes 2..N to node1
    let mut bootstrap: Vec<String> = Vec::new();
    if node_idx != 1 {
        // robust: load node1 peerid from its persisted identity (same VPS)
        let node1_data_dir = "/srv/amunchain/node1/data";
        match amunchain::networking::p2p_identity::load_or_create_identity(node1_data_dir) {
            Ok((peer_id, _kp)) => {
                bootstrap.push(format!("/ip4/127.0.0.1/tcp/4001/p2p/{peer_id}"));
            }
            Err(e) => {
                warn!(?e, "failed to load node1 identity for bootstrap; starting without bootstrap");
            }
        }
    }

    // Metrics: spawn_p2p now needs Arc<Metrics>
    // (Path may be `amunchain::monitoring::metrics::Metrics` OR `amunchain::core::metrics::Metrics` depending on your tree.)
    let metrics: Arc<amunchain::monitoring::metrics::Metrics> = Arc::new(amunchain::monitoring::metrics::Metrics::new().expect("metrics init failed"));

    let cfg = amunchain::networking::p2p::P2pConfig {
        data_dir: data_dir.clone(),
        listen_addr,
        consensus_topic,
        max_msg_per_sec: 200,
        max_peers_per_ip: 4,
        bootstrap,
        allow_peers: vec![
            "12D3KooWLh9S2QyVMgQgHmuzu2tCA6KsL9mpNgnvoiA1SQ9nHnMA".to_string(),
            "12D3KooWLCZAkdjaD7FAT65FvUhwHVC5RBvYRTDZ6QXCbks7ZTaJ".to_string(),
            "12D3KooWS9xDuptBksMQs7hAvKAJQhW5G9wYYVg7yemgGSZkQxWX".to_string(),
            "12D3KooWEdXmay5QGhLnJnuDD9Wt2M3v2ADEjmEHFsN33XkTaTN4".to_string(),
        ],
    };

    info!(node = node_idx, data_dir = %data_dir, "amunchain node starting");

    let (_node, mut ev_rx, p2p_handle) = match amunchain::networking::p2p::spawn_p2p(cfg, metrics) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("P2P start failed: {e}");
            std::process::exit(1);
        }
    };

    // keep alive + log events
    let ev_task = tokio::spawn(async move {
        while let Some(ev) = ev_rx.recv().await {
            info!(?ev, "p2p event");
        }
        warn!("p2p event channel closed");
    });

    // Wait forever (or until crash)
    let _ = p2p_handle.await;
    let _ = ev_task.await;
}
