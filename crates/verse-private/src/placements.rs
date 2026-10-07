//! The owner-local placements file, `private-assets.json` in Verse's home
//! (`~/.openagents/verse/`): which broker to ask, which Verse profile's key
//! signs, and where each private asset stands. It lives on the owner's
//! computer so that committed code never names a private asset.

use std::io::Read;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::{MAX_PACK_BYTES, valid_digest, valid_name};

/// The placements schema.
pub const SCHEMA: &str = "openagents.verse.private-placements.v1";
/// The file's name in Verse's home.
pub const FILE: &str = "private-assets.json";
/// The private pack cache's directory name in Verse's home.
pub const CACHE: &str = "private-cache";
/// Most placements one file may hold.
pub const MAX_PLACEMENTS: usize = 16;
const MAX_FILE_BYTES: u64 = 64 * 1024;
/// How far from the zone's center a placement may stand, m.
const MAX_REACH: f32 = 400.0;

/// The zones a placement may name.
pub const ZONES: [&str; 1] = ["everglade"];

/// The whole file.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Placements {
    pub schema: String,
    /// The broker's base URL, `https://...`, without a trailing slash.
    pub broker: String,
    /// The Verse profile whose key signs grant requests.
    pub profile: String,
    pub placements: Vec<Placement>,
}

/// One private asset standing in a zone.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Placement {
    /// The registry name.
    pub asset: String,
    /// The pack's SHA-256.
    pub sha256: String,
    /// The pack's length.
    pub bytes: u64,
    /// The zone, such as `everglade`.
    pub zone: String,
    /// Where it stands, x and z, m.
    pub at: [f32; 2],
    /// Its facing, as the controller's yaw, radians.
    pub yaw: f32,
    /// Times its compiled size.
    pub scale: f32,
}

impl Placements {
    /// An empty file for `broker` and `profile`.
    #[must_use]
    pub fn new(broker: &str, profile: &str) -> Self {
        Self {
            schema: SCHEMA.into(),
            broker: broker.trim_end_matches('/').into(),
            profile: profile.into(),
            placements: Vec::new(),
        }
    }

    /// Parses and checks the file's bytes.
    ///
    /// # Errors
    ///
    /// Returns a message naming the first field that fails.
    pub fn parse(bytes: &[u8]) -> Result<Self, String> {
        let file: Self = serde_json::from_slice(bytes).map_err(|e| format!("{FILE}: {e}"))?;
        file.check()?;
        Ok(file)
    }

    /// Checks every field.
    ///
    /// # Errors
    ///
    /// Returns a message naming the first field that fails.
    pub fn check(&self) -> Result<(), String> {
        if self.schema != SCHEMA {
            return Err(format!("{FILE}: schema must be {SCHEMA}"));
        }
        let host = self.broker.strip_prefix("https://").unwrap_or_default();
        if host.is_empty()
            || self.broker.ends_with('/')
            || host.contains(['?', '#', ' ', '@'])
            || host.split('/').count() != 1
        {
            return Err(format!("{FILE}: broker must be an https:// origin"));
        }
        if self.profile.is_empty()
            || self.profile.len() > 32
            || !self
                .profile
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
        {
            return Err(format!("{FILE}: profile must be a Verse profile name"));
        }
        if self.placements.len() > MAX_PLACEMENTS {
            return Err(format!("{FILE}: at most {MAX_PLACEMENTS} placements"));
        }
        for p in &self.placements {
            if !valid_name(&p.asset) || !valid_digest(&p.sha256) {
                return Err(format!(
                    "{FILE}: a placement needs an asset name and digest"
                ));
            }
            if p.bytes == 0 || p.bytes > MAX_PACK_BYTES {
                return Err(format!("{FILE}: {} has an invalid pack length", p.asset));
            }
            if !ZONES.contains(&p.zone.as_str()) {
                return Err(format!("{FILE}: {} names an unknown zone", p.asset));
            }
            let finite = p.at.iter().all(|v| v.is_finite() && v.abs() <= MAX_REACH)
                && p.yaw.is_finite()
                && p.scale.is_finite()
                && (0.25..=4.0).contains(&p.scale);
            if !finite {
                return Err(format!("{FILE}: {} has an invalid place", p.asset));
            }
        }
        Ok(())
    }

    /// The placements in `zone`.
    pub fn in_zone<'a>(&'a self, zone: &'a str) -> impl Iterator<Item = &'a Placement> + 'a {
        self.placements.iter().filter(move |p| p.zone == zone)
    }

    /// Adds `placement`, replacing any earlier placement of the same asset
    /// in the same zone.
    pub fn place(&mut self, placement: Placement) {
        self.placements
            .retain(|p| !(p.asset == placement.asset && p.zone == placement.zone));
        self.placements.push(placement);
    }

    /// The file as stored: pretty JSON with a trailing newline.
    ///
    /// # Errors
    ///
    /// Returns a message when it fails [`Self::check`].
    pub fn to_bytes(&self) -> Result<Vec<u8>, String> {
        self.check()?;
        let mut bytes = serde_json::to_vec_pretty(self).map_err(|e| e.to_string())?;
        bytes.push(b'\n');
        Ok(bytes)
    }
}

/// The placements file in `home`.
#[must_use]
pub fn path(home: &Path) -> PathBuf {
    home.join(FILE)
}

/// Reads the placements file in `home`: `None` when there is none. Refuses
/// a symbolic link, a non-regular file, or a file longer than 64 KiB.
///
/// # Errors
///
/// Returns a message when the file exists but can't be read or fails its
/// checks.
pub fn load(home: &Path) -> Result<Option<Placements>, String> {
    let path = path(home);
    let metadata = match std::fs::symlink_metadata(&path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(_) => return Err(format!("{FILE} could not be read")),
    };
    if !metadata.file_type().is_file() || metadata.len() > MAX_FILE_BYTES {
        return Err(format!("{FILE} must be a regular file of at most 64 KiB"));
    }
    let mut bytes = Vec::new();
    std::fs::File::open(&path)
        .and_then(|file| file.take(MAX_FILE_BYTES + 1).read_to_end(&mut bytes))
        .map_err(|_| format!("{FILE} could not be read"))?;
    Placements::parse(&bytes).map(Some)
}

/// Writes `placements` to `home` with mode 0600, through a temporary file
/// and a rename.
///
/// # Errors
///
/// Returns a message when the file can't be written.
pub fn save(home: &Path, placements: &Placements) -> Result<(), String> {
    let bytes = placements.to_bytes()?;
    std::fs::create_dir_all(home).map_err(|e| format!("{}: {e}", home.display()))?;
    let temp = home.join(format!(".{FILE}.{}.part", std::process::id()));
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let result = (|| {
        use std::io::Write;
        let mut file = options.open(&temp).map_err(|e| e.to_string())?;
        file.write_all(&bytes).map_err(|e| e.to_string())?;
        file.sync_all().map_err(|e| e.to_string())?;
        std::fs::rename(&temp, path(home)).map_err(|e| e.to_string())
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(&temp);
    }
    result.map_err(|e| format!("{FILE} could not be written: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn placement() -> Placement {
        Placement {
            asset: "sample-guest".into(),
            sha256: "cd".repeat(32),
            bytes: 1234,
            zone: "everglade".into(),
            at: [105.5, -31.2],
            yaw: -1.571,
            scale: 1.0,
        }
    }

    #[test]
    fn placements_round_trip_through_a_private_file() {
        let home = tempfile::tempdir().unwrap();
        assert_eq!(load(home.path()).unwrap(), None);
        let mut file = Placements::new("https://broker.example/", "default");
        file.place(placement());
        let mut moved = placement();
        moved.at = [1.0, 2.0];
        file.place(moved.clone());
        assert_eq!(file.placements, vec![moved]);
        save(home.path(), &file).unwrap();
        assert_eq!(load(home.path()).unwrap(), Some(file.clone()));
        assert_eq!(file.in_zone("everglade").count(), 1);
        assert_eq!(file.in_zone("grid").count(), 0);
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = std::fs::metadata(path(home.path()))
                .unwrap()
                .permissions()
                .mode();
            assert_eq!(mode & 0o777, 0o600);
        }
    }

    #[test]
    fn placements_refuse_what_the_loader_cannot_trust() {
        let check = |edit: &dyn Fn(&mut Placements)| {
            let mut file = Placements::new("https://broker.example", "default");
            file.place(placement());
            edit(&mut file);
            file.check()
        };
        assert!(check(&|_| {}).is_ok());
        assert!(check(&|f| f.broker = "http://broker.example".into()).is_err());
        assert!(check(&|f| f.broker = "https://broker.example/path".into()).is_err());
        assert!(check(&|f| f.profile = "../key".into()).is_err());
        assert!(check(&|f| f.placements[0].sha256 = "x".into()).is_err());
        assert!(check(&|f| f.placements[0].zone = "grid".into()).is_err());
        assert!(check(&|f| f.placements[0].at = [f32::NAN, 0.0]).is_err());
        assert!(check(&|f| f.placements[0].scale = 10.0).is_err());
        assert!(check(&|f| f.placements[0].bytes = 0).is_err());
    }

    #[cfg(unix)]
    #[test]
    fn a_linked_placements_file_is_refused() {
        let home = tempfile::tempdir().unwrap();
        let elsewhere = home.path().join("elsewhere.json");
        std::fs::write(&elsewhere, b"{}").unwrap();
        std::os::unix::fs::symlink(&elsewhere, path(home.path())).unwrap();
        assert!(load(home.path()).is_err());
    }
}
