#![forbid(unsafe_code)]

use amunchain::{
    core::security::keystore::Keystore,
    networking::peer_registry::{load_and_verify_peer_registry_now, PeerRegistryPolicy},
};
use std::fs;

#[test]
fn peer_registry_loads_and_verifies() {
    // Create a temporary keystore to act as the registry signer.
    let dir = tempfile::tempdir().expect("tempdir");
    let ks = Keystore::open(dir.path().to_str().unwrap()).expect("keystore open");
    let pk = ks.public_key();

    let peer = "12D3KooWPYkNZrwQo5yESaXbBQ64f3GyFaUPFynPUoE7PfJ4xL4u";
    let network = "amunchain/consensus/v2";

    let issued_at_ms: u64 = 1768336425892;
    let expires_at_ms: u64 = 1768336485892;

    // Canonical bytes as specified in networking::peer_registry.
    let msg = format!(
        "v1\nnetwork={}\nissued_at_ms={}\nexpires_at_ms={}\npeers\n{}\n",
        network, issued_at_ms, expires_at_ms, peer
    );
    let sig = ks.sign(msg.as_bytes()).expect("sign");

    let toml = format!(
        "version = 1\nnetwork = \"{}\"\nissued_at_ms = {}\nexpires_at_ms = {}\npeers = [\"{}\"]\n\nsignature_hex = \"{}\"\n",
        network,
        issued_at_ms,
        expires_at_ms,
        peer,
        hex::encode(sig.0)
    );

    let path = dir.path().join("peer_registry.toml");
    fs::write(&path, toml).expect("write");

    let mut pol = PeerRegistryPolicy::default_with_now(issued_at_ms + 1);
    pol.expected_network = Some(network);
    pol.require_freshness_fields = true;

    let allow = load_and_verify_peer_registry_now(path.to_str().unwrap(), &hex::encode(pk.0), &pol)
        .expect("load and verify");
    assert_eq!(allow, vec![peer.to_string()]);
}
