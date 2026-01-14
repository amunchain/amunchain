// Copyright (c) 2026 Amunchain
// Licensed under the Apache-2.0 License.

#![no_main]
#![forbid(unsafe_code)]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    // Best-effort: ensure registry parsing does not panic.
    // Signature verification is covered by unit tests; here we focus on parser robustness.
    if let Ok(s) = std::str::from_utf8(data) {
        let _ = amunchain::networking::peer_registry::parse_peer_registry_toml(s);
    }
});
