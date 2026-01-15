#![forbid(unsafe_code)]

use std::{
    fs,
    io::{self, Write},
    path::{Path, PathBuf},
};

use libp2p::{identity, PeerId};

#[derive(Debug)]
pub enum IdentityError {
    Io,
    Decode,
}

impl From<io::Error> for IdentityError {
    fn from(_: io::Error) -> Self {
        IdentityError::Io
    }
}

/// Load an existing Ed25519 keypair from `data_dir/p2p_identity.key`,
/// or create a new one and persist it.
///
/// Returns (PeerId, Keypair).
pub fn load_or_create_identity(
    data_dir: impl AsRef<Path>,
) -> Result<(PeerId, identity::Keypair), IdentityError> {
    let dir = data_dir.as_ref();
    fs::create_dir_all(dir)?;

    let path: PathBuf = dir.join("p2p_identity.key");

    if path.exists() {
        let bytes = fs::read(&path)?;
        let kp =
            identity::Keypair::from_protobuf_encoding(&bytes).map_err(|_| IdentityError::Decode)?;
        let pid = PeerId::from(kp.public());
        return Ok((pid, kp));
    }

    let kp = identity::Keypair::generate_ed25519();
    let bytes = kp
        .to_protobuf_encoding()
        .map_err(|_| IdentityError::Decode)?;

    // Atomic-ish write: write to tmp then rename.
    let tmp = dir.join("p2p_identity.key.tmp");
    {
        let mut f = fs::File::create(&tmp)?;
        f.write_all(&bytes)?;
        f.sync_all()?;
    }
    fs::rename(&tmp, &path)?;

    let pid = PeerId::from(kp.public());
    Ok((pid, kp))
}
