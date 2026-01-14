#![forbid(unsafe_code)]

fn main() {
    // Minimal binary entrypoint to keep the crate building cleanly.
    // P2P + HTTP runtime wiring will be restored in a dedicated pass.
    println!("amunchain: build OK (library + minimal binary).");
}
