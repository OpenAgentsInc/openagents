//! Portable signed Snap Station builds, checked by replay before use.
use ed25519_dalek::{Signature, Signer, SigningKey, VerifyingKey};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use snap_rules::{
    Game, Input, Status, level,
    receipt::{self, Receipt},
};

pub const FORMAT: &str = "snap.shared-build.v1";
pub const MAX_BYTES: usize = receipt::MAX_BYTES + 2048;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Shared {
    pub format: String,
    pub job: usize,
    pub author: String,
    pub nonce: String,
    pub gallery: bool,
    pub run: String,
    pub signature: String,
}

pub struct Checked {
    pub game: Game,
    pub construction: Vec<Input>,
}

fn bytes<const N: usize>(s: &str) -> Result<[u8; N], String> {
    if s.len() != N * 2 {
        return Err("The build file is damaged.".into());
    }
    let mut out = [0; N];
    for (i, b) in out.iter_mut().enumerate() {
        *b = u8::from_str_radix(
            s.get(i * 2..i * 2 + 2)
                .ok_or("The build file is damaged.")?,
            16,
        )
        .map_err(|_| "The build file is damaged.")?;
    }
    Ok(out)
}

impl Shared {
    pub fn sign(
        job: usize,
        run: &Receipt,
        secret: &[u8; 32],
        nonce: &[u8; 16],
        gallery: bool,
    ) -> Result<Self, String> {
        let key = SigningKey::from_bytes(secret);
        let mut shared = Self {
            format: FORMAT.into(),
            job,
            author: receipt::hex(&key.verifying_key().to_bytes()),
            nonce: receipt::hex(nonce),
            gallery,
            run: run.encode(),
            signature: String::new(),
        };
        shared.signature = receipt::hex(&key.sign(&shared.message()).to_bytes());
        shared.check()?;
        Ok(shared)
    }
    fn message(&self) -> Vec<u8> {
        format!(
            "{}\n{}\n{}\n{}\n{}\n{}\n",
            self.format,
            self.job,
            self.author,
            self.nonce,
            self.gallery,
            receipt::hex(&Sha256::digest(self.run.as_bytes()))
        )
        .into_bytes()
    }
    pub fn encode(&self) -> String {
        serde_json::to_string(self).expect("serializable build")
    }
    pub fn decode(text: &str) -> Result<Self, String> {
        if text.len() > MAX_BYTES {
            return Err("This build file is too large.".into());
        }
        serde_json::from_str(text).map_err(|_| "Choose a Snap Station build file.".into())
    }
    pub fn code(&self) -> String {
        let digest = Sha256::digest(self.encode().as_bytes());
        let alphabet = b"0123456789ABCDEFGHJKMNPQRSTVWXYZ";
        let value = u64::from_be_bytes(digest[..8].try_into().expect("eight bytes")) >> 4;
        (0..12)
            .rev()
            .map(|i| alphabet[((value >> (i * 5)) & 31) as usize] as char)
            .collect()
    }
    pub fn check(&self) -> Result<Checked, String> {
        if self.format != FORMAT
            || !(1..=level::COUNT).contains(&self.job)
            || self.run.len() > receipt::MAX_BYTES
        {
            return Err("This build needs another version of Snap Station.".into());
        }
        let _ = bytes::<16>(&self.nonce)?;
        let key = VerifyingKey::from_bytes(&bytes::<32>(&self.author)?)
            .map_err(|_| "The build file is damaged.")?;
        let signature = Signature::from_bytes(&bytes::<64>(&self.signature)?);
        key.verify_strict(&self.message(), &signature)
            .map_err(|_| "This build file has changed since it was shared.")?;
        let receipt = Receipt::decode(&self.run).map_err(|_| "This run could not be opened.")?;
        let (game, construction) = receipt
            .replay_build(&level::job(self.job))
            .map_err(|_| "This run did not play back correctly.")?;
        if !matches!(game.status, Status::Won | Status::Failed(_)) {
            return Err("Finish a run before sharing it.".into());
        }
        Ok(Checked { game, construction })
    }
}

/// Accepts grouped, lower-case codes copied from a friend.
pub fn code(text: &str) -> Option<String> {
    let s: String = text
        .chars()
        .filter(|c| *c != '-' && !c.is_whitespace())
        .map(|c| c.to_ascii_uppercase())
        .collect();
    (s.len() == 12
        && s.bytes()
            .all(|b| b"0123456789ABCDEFGHJKMNPQRSTVWXYZ".contains(&b)))
    .then_some(s)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn build() -> Shared {
        let level = level::job(1);
        let plan = snap_rules::bot::solve(&level).unwrap();
        let (_, run) = snap_rules::bot::record_plan(&level, &plan);
        Shared::sign(1, &run, &[7; 32], &[9; 16], true).unwrap()
    }
    #[test]
    fn signed_build_round_trips_and_replays() {
        let b = build();
        assert_eq!(Shared::decode(&b.encode()).unwrap(), b);
        assert_eq!(b.check().unwrap().game.status, Status::Won);
        assert!(!b.check().unwrap().construction.is_empty());
        assert_eq!(code(&b.code().to_lowercase()).unwrap(), b.code());
        assert!(code("../anything").is_none());
    }
    #[test]
    fn signature_binds_run_job_identity_and_gallery_consent() {
        let b = build();
        for n in 0..5 {
            let mut changed = b.clone();
            match n {
                0 => changed.gallery = false,
                1 => changed.job = 2,
                2 => changed.run.push(' '),
                3 => changed.author = receipt::hex(&[0; 32]),
                _ => changed.nonce = receipt::hex(&[0; 16]),
            }
            assert!(changed.check().is_err());
        }
        assert!(Shared::decode(&"x".repeat(MAX_BYTES + 1)).is_err());
    }
}
