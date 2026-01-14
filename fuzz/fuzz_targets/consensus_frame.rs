#![no_main]
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    // Best-effort: ensure decoding/parsing paths do not panic.
    // This target is intentionally conservative and should be extended with real protocol framing.
    let _ = std::str::from_utf8(data);
});
