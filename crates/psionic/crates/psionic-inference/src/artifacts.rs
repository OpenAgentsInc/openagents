//! Shared artifact checks for model families and execution engines.

use crate::{Error, Result};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{collections::BTreeMap, io::Read, path::Path};

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Artifact {
    pub sha256: String,
    pub bytes: u64,
}

pub fn file_sha256(path: &Path) -> Result<String> {
    let mut file = std::fs::File::open(path)?;
    let mut hash = Sha256::new();
    let mut buffer = [0_u8; 65536];
    loop {
        let n = file.read(&mut buffer)?;
        if n == 0 {
            break;
        }
        hash.update(&buffer[..n]);
    }
    Ok(format!("{:x}", hash.finalize()))
}

pub fn verify(root: &Path, files: &BTreeMap<String, Artifact>) -> Result<()> {
    for (name, expected) in files {
        let path = Path::new(name);
        if path.is_absolute()
            || path
                .components()
                .any(|c| !matches!(c, std::path::Component::Normal(_)))
        {
            return Err(Error(
                "Model file path must stay inside its directory".into(),
            ));
        }
        let path = root.join(path);
        if path.metadata()?.len() != expected.bytes || file_sha256(&path)? != expected.sha256 {
            return Err(Error(format!(
                "Model file does not match its expected checksum: {name}"
            )));
        }
    }
    Ok(())
}
