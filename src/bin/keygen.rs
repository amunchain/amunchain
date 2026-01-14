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

use anyhow::Result;
use ring::signature::{Ed25519KeyPair, KeyPair};
use std::path::PathBuf;

fn main() -> Result<()> {
    let out_dir = std::env::args().nth(1).unwrap_or_else(|| "data".to_string());
    let mut key_path = PathBuf::from(out_dir);
    std::fs::create_dir_all(&key_path)?;
    key_path.push("validator.key");

    let rng = ring::rand::SystemRandom::new();
    let pkcs8 = Ed25519KeyPair::generate_pkcs8(&rng)?;
    std::fs::write(&key_path, pkcs8.as_ref())?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&key_path, std::fs::Permissions::from_mode(0o600));
    }

    let kp = Ed25519KeyPair::from_pkcs8(pkcs8.as_ref())?;
    let pk = kp.public_key().as_ref();
    println!("{}", hex::encode(pk));
    Ok(())
}
