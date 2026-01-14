#![forbid(unsafe_code)]
#![deny(missing_docs)]
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


//! Keystore: Ed25519 signing/verification with rate limiting and an audit trail.
//!
//! ## Production hardening
//! - **Atomic writes** for private key material.
//! - **Key-at-rest encryption** (optional) via `AMUNCHAIN_KEY_PASSPHRASE` (or legacy `NEXUS_KEY_PASSPHRASE`).
//! - **Audit log rotation** (best-effort).
//! - **Best-effort zeroization** of sensitive buffers.
//!
//! ### Key encryption format
//! If `AMUNCHAIN_KEY_PASSPHRASE` is set, `validator.key` is stored as:
//! `MAGIC(9) || SALT(16) || NONCE(12) || CIPHERTEXT+TAG(..)`
//! where the ciphertext is AES-256-GCM over the Ed25519 PKCS#8 bytes.

use ring::{
    aead,
    pbkdf2,
    rand::{SecureRandom, SystemRandom},
    signature::{Ed25519KeyPair, KeyPair, UnparsedPublicKey, ED25519},
};
use std::{
    fs,
    io::Write,
    num::NonZeroU32,
    path::{Path, PathBuf},
    sync::Mutex,
    time::{Duration, Instant},
};
use thiserror::Error;
use zeroize::Zeroize;

use crate::core::types::Signature;


fn env_first(keys: &[&str]) -> Option<String> {
    for &k in keys {
        if let Ok(v) = std::env::var(k) {
            if !v.trim().is_empty() {
                return Some(v);
            }
        }
    }
    None
}

const KEY_FILE_MAGIC: &[u8] = b"AMUNKEY1"; // 8 bytes // 8 bytes
const KEY_SALT_LEN: usize = 16;
const KEY_NONCE_LEN: usize = 12;

const MAX_AUDIT_BYTES: u64 = 32 * 1024 * 1024; // 32 MiB
const AUDIT_ROTATE_KEEP: usize = 3;

// PBKDF2 params: 100k iterations is a reasonable baseline for server-side passphrases.
// Increase if your deployment can afford it.
const PBKDF2_ITERS_DEFAULT: u32 = 100_000;

fn pbkdf2_iters() -> NonZeroU32 {
    // Optional override via env (defense-in-depth; keep bounds sane).
    // Example: AMUNCHAIN_PBKDF2_ITERS=300000 (or legacy NEXUS_PBKDF2_ITERS=300000)
    let iters = env_first(&["AMUNCHAIN_PBKDF2_ITERS", "NEXUS_PBKDF2_ITERS"])
        .and_then(|v| v.parse::<u32>().ok())
        .unwrap_or(PBKDF2_ITERS_DEFAULT);

    // Clamp to a practical range to avoid accidental DoS / too-weak configs.
    let iters = iters.clamp(10_000, 10_000_000);
    NonZeroU32::new(iters).unwrap_or_else(|| NonZeroU32::new(PBKDF2_ITERS_DEFAULT).expect("nonzero"))
}

/// Keystore errors.
#[derive(Debug, Error)]
pub enum KeystoreError {
    #[error("io")]
    Io,
    #[error("invalid key encoding")]
    InvalidKey,
    #[error("missing passphrase (set AMUNCHAIN_KEY_PASSPHRASE)")]
    MissingPassphrase,
    #[error("crypto")]
    Crypto,
    #[error("rate limited")]
    RateLimited,
    #[error("bad signature")]
    BadSignature,
}

/// Signer backend abstraction (HSM compatible).
pub trait SignerBackend: Send + Sync {
    /// Return public key bytes (Ed25519, 32 bytes).
    fn public_key(&self) -> [u8; 32];
    /// Sign message bytes.
    fn sign(&self, msg: &[u8]) -> Result<Signature, KeystoreError>;
}

/// Simple file-backed Ed25519 backend.
pub struct FileEd25519Backend {
    keypair: Ed25519KeyPair,
}

fn rotate_audit_if_needed(path: &Path) {
    let Ok(md) = fs::metadata(path) else { return; };
    if md.len() <= MAX_AUDIT_BYTES {
        return;
    }

    // best-effort rotation (no crash if it fails)
    for i in (1..=AUDIT_ROTATE_KEEP).rev() {
        let dst = PathBuf::from(format!("{}.{}", path.display(), i));
        let src = if i == 1 {
            path.to_path_buf()
        } else {
            PathBuf::from(format!("{}.{}", path.display(), i - 1))
        };
        if src.exists() {
            let _ = fs::rename(&src, &dst);
        }
    }
}

fn set_private_perms_best_effort(path: &Path) {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(path, fs::Permissions::from_mode(0o600));
    }
}

/// Atomic write to disk (best-effort fsync, then rename).
fn atomic_write_private(path: &Path, bytes: &[u8]) -> Result<(), KeystoreError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|_| KeystoreError::Io)?;
    }

    let mut tmp = path.to_path_buf();
    tmp.set_extension("tmp");

    {
        let mut f = fs::OpenOptions::new()
            .create(true)
            .truncate(true)
            .write(true)
            .open(&tmp)
            .map_err(|_| KeystoreError::Io)?;
        f.write_all(bytes).map_err(|_| KeystoreError::Io)?;
        let _ = f.sync_all();
    }

    set_private_perms_best_effort(&tmp);
    fs::rename(&tmp, path).map_err(|_| KeystoreError::Io)?;
    set_private_perms_best_effort(path);
    Ok(())
}

fn derive_aes256gcm_key(passphrase: &[u8], salt: &[u8; KEY_SALT_LEN]) -> Result<[u8; 32], KeystoreError> {
    let mut out = [0u8; 32];
    pbkdf2::derive(
        pbkdf2::PBKDF2_HMAC_SHA256,
        pbkdf2_iters(),
        salt,
        passphrase,
        &mut out,
    );
    Ok(out)
}

fn encrypt_pkcs8(passphrase: &[u8], plaintext: &[u8]) -> Result<Vec<u8>, KeystoreError> {
    let rng = SystemRandom::new();

    let mut salt = [0u8; KEY_SALT_LEN];
    rng.fill(&mut salt).map_err(|_| KeystoreError::Crypto)?;

    let mut nonce_bytes = [0u8; KEY_NONCE_LEN];
    rng.fill(&mut nonce_bytes).map_err(|_| KeystoreError::Crypto)?;
    let nonce = aead::Nonce::assume_unique_for_key(nonce_bytes);

    let mut key = derive_aes256gcm_key(passphrase, &salt)?;
    let unbound = aead::UnboundKey::new(&aead::AES_256_GCM, &key).map_err(|_| KeystoreError::Crypto)?;
    let less_safe = aead::LessSafeKey::new(unbound);

    // ciphertext buffer = plaintext + tag
    let mut in_out = plaintext.to_vec();
    less_safe
        .seal_in_place_append_tag(nonce, aead::Aad::empty(), &mut in_out)
        .map_err(|_| KeystoreError::Crypto)?;

    key.zeroize();

    let mut out = Vec::with_capacity(KEY_FILE_MAGIC.len() + KEY_SALT_LEN + KEY_NONCE_LEN + in_out.len());
    out.extend_from_slice(KEY_FILE_MAGIC);
    out.extend_from_slice(&salt);
    out.extend_from_slice(&nonce_bytes);
    out.extend_from_slice(&in_out);
    Ok(out)
}

fn decrypt_pkcs8(passphrase: &[u8], bytes: &[u8]) -> Result<Vec<u8>, KeystoreError> {
    if bytes.len() < KEY_FILE_MAGIC.len() + KEY_SALT_LEN + KEY_NONCE_LEN + 16 {
        return Err(KeystoreError::InvalidKey);
    }
    if &bytes[..KEY_FILE_MAGIC.len()] != KEY_FILE_MAGIC {
        // Not encrypted, caller should treat as plaintext PKCS#8.
        return Ok(bytes.to_vec());
    }

    let mut salt = [0u8; KEY_SALT_LEN];
    salt.copy_from_slice(&bytes[KEY_FILE_MAGIC.len()..KEY_FILE_MAGIC.len() + KEY_SALT_LEN]);
    let mut nonce_bytes = [0u8; KEY_NONCE_LEN];
    nonce_bytes.copy_from_slice(
        &bytes[KEY_FILE_MAGIC.len() + KEY_SALT_LEN..KEY_FILE_MAGIC.len() + KEY_SALT_LEN + KEY_NONCE_LEN],
    );
    let nonce = aead::Nonce::assume_unique_for_key(nonce_bytes);

    let mut key = derive_aes256gcm_key(passphrase, &salt)?;
    let unbound = aead::UnboundKey::new(&aead::AES_256_GCM, &key).map_err(|_| KeystoreError::Crypto)?;
    let less_safe = aead::LessSafeKey::new(unbound);

    let mut in_out = bytes[KEY_FILE_MAGIC.len() + KEY_SALT_LEN + KEY_NONCE_LEN..].to_vec();
    let plain = less_safe
        .open_in_place(nonce, aead::Aad::empty(), &mut in_out)
        .map_err(|_| KeystoreError::Crypto)?;

    key.zeroize();
    Ok(plain.to_vec())
}

impl FileEd25519Backend {
    /// Load or create an Ed25519 PKCS#8 key file.
    ///
    /// If `AMUNCHAIN_KEY_PASSPHRASE` is set, the key file is encrypted at rest.
    pub fn load_or_create(path: &Path) -> Result<Self, KeystoreError> {
        let pass = env_first(&["AMUNCHAIN_KEY_PASSPHRASE", "NEXUS_KEY_PASSPHRASE"]);

        if path.exists() {
            let bytes = fs::read(path).map_err(|_| KeystoreError::Io)?;
            // If it's encrypted, passphrase is required.
            let pkcs8 = if bytes.starts_with(KEY_FILE_MAGIC) {
                let Some(p) = pass.as_deref() else {
                    return Err(KeystoreError::MissingPassphrase);
                };
                decrypt_pkcs8(p.as_bytes(), &bytes)?
            } else {
                bytes
            };
            let kp = Ed25519KeyPair::from_pkcs8(&pkcs8).map_err(|_| KeystoreError::InvalidKey)?;
            return Ok(Self { keypair: kp });
        }

        let rng = SystemRandom::new();
        let pkcs8 = Ed25519KeyPair::generate_pkcs8(&rng).map_err(|_| KeystoreError::InvalidKey)?;

        // Write key: encrypted if passphrase is present.
        let mut buf = pkcs8.as_ref().to_vec();
        let on_disk = if let Some(p) = pass.as_deref() {
            let enc = encrypt_pkcs8(p.as_bytes(), &buf)?;
            buf.zeroize();
            enc
        } else {
            buf.clone()
        };
        atomic_write_private(path, &on_disk)?;
        buf.zeroize();

        // Parse from plaintext pkcs8 (already in `pkcs8`).
        let kp = Ed25519KeyPair::from_pkcs8(pkcs8.as_ref()).map_err(|_| KeystoreError::InvalidKey)?;
        Ok(Self { keypair: kp })
    }
}

impl SignerBackend for FileEd25519Backend {
    fn public_key(&self) -> [u8; 32] {
        let pk = self.keypair.public_key().as_ref();
        let mut out = [0u8; 32];
        out.copy_from_slice(pk);
        out
    }

    fn sign(&self, msg: &[u8]) -> Result<Signature, KeystoreError> {
        let sig = self.keypair.sign(msg);
        Ok(Signature(sig.as_ref().to_vec()))
    }
}

/// Rate limiter (token bucket style, simple and deterministic).
#[derive(Debug)]
struct RateLimiter {
    window_start: Instant,
    count: u32,
    limit_per_sec: u32,
}

impl RateLimiter {
    fn new(limit_per_sec: u32) -> Self {
        Self { window_start: Instant::now(), count: 0, limit_per_sec }
    }

    fn allow(&mut self) -> bool {
        let now = Instant::now();
        if now.duration_since(self.window_start) >= Duration::from_secs(1) {
            self.window_start = now;
            self.count = 0;
        }
        if self.count >= self.limit_per_sec {
            return false;
        }
        self.count = self.count.saturating_add(1);
        true
    }
}

/// Keystore wrapper.
pub struct Keystore<B: SignerBackend> {
    backend: B,
    limiter: Mutex<RateLimiter>,
    audit_path: PathBuf,
}

impl Keystore<FileEd25519Backend> {
    /// Load or create keystore in `data_dir/validator.key` and write audit to `data_dir/audit.log`.
    pub fn open(data_dir: &str) -> Result<Self, KeystoreError> {
        let mut key_path = PathBuf::from(data_dir);
        key_path.push("validator.key");

        let mut audit_path = PathBuf::from(data_dir);
        audit_path.push("audit.log");

        let backend = FileEd25519Backend::load_or_create(&key_path)?;
        Ok(Self { backend, limiter: Mutex::new(RateLimiter::new(10_000)), audit_path })
    }
}

impl<B: SignerBackend> Keystore<B> {
    /// Public key.
    pub fn public_key(&self) -> [u8; 32] {
        self.backend.public_key()
    }

    /// Sign with rate limiting and an audit trail (best-effort).
    pub fn sign(&self, msg: &[u8]) -> Result<Signature, KeystoreError> {
        let mut guard = self.limiter.lock().map_err(|_| KeystoreError::RateLimited)?;
        if !guard.allow() {
            return Err(KeystoreError::RateLimited);
        }

        let _ = append_audit(&self.audit_path, "sign", msg);
        self.backend.sign(msg)
    }
}

/// Verify signature given raw pubkey bytes.
pub fn verify_pubkey_bytes(pk_bytes: &[u8; 32], msg: &[u8], sig: &Signature) -> Result<(), KeystoreError> {
    // ring requires signature length 64 for Ed25519
    if sig.0.len() != 64 {
        return Err(KeystoreError::BadSignature);
    }
    let pk = UnparsedPublicKey::new(&ED25519, pk_bytes);
    pk.verify(msg, &sig.0).map_err(|_| KeystoreError::BadSignature)
}

fn append_audit(path: &Path, action: &str, msg: &[u8]) -> Result<(), KeystoreError> {
    rotate_audit_if_needed(path);

    // best-effort: store SHA-256(message) only (avoid leaking content)
    let digest = ring::digest::digest(&ring::digest::SHA256, msg);
    let line = format!(
        "{{\"action\":\"{}\",\"msg_sha256\":\"{}\"}}\n",
        action,
        hex::encode(digest.as_ref())
    );

    let mut f = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .map_err(|_| KeystoreError::Io)?;
    set_private_perms_best_effort(path);
    f.write_all(line.as_bytes()).map_err(|_| KeystoreError::Io)?;
    Ok(())
}
