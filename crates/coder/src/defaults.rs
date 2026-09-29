//! Coder's defaults on this computer: the extensions the newest
//! `openagents:coder-defaults` release admits, as a session admits them.
//!
//! A release reaches a runtime as one directory
//! ([`directory`]: `CODER_DEFAULTS`, else `~/.openagents/coder-defaults`)
//! that `openagents ext defaults sync` writes after reading the relay under
//! the ledger's checks (`xp_ledger::defaults`): `lock.json`, the defaults
//! lock naming the release and each admitted extension with the admission
//! that admits it and when that lapses; `programs/<slug>.json`, each
//! admitted extension's program; and `skills/<name>.md`, its skills. A
//! session reads the lock ([`read`]), widens the operator's program grant
//! by the admitted programs whose files are here, appends the skills to
//! its instructions, and records the lock's digest in every program run's
//! run-state record and in the trace. An admission that lapsed before the
//! session started admits nothing; the sync refreshes the lock, and a
//! stale lock names its `expires_at`, so a reader can tell.
//!
//! Nothing here fetches or verifies signatures: that is the sync's job,
//! and the lock is what it wrote. A session without the directory runs
//! as it always did, with nothing admitted by default.

use std::path::{Path, PathBuf};

use knowledge::xp::defaults::{Defaults, parse_lock};

/// The directory the defaults are kept in, when the environment names one
/// (`off` names none).
pub const DIR_ENV: &str = "CODER_DEFAULTS";

/// The lock file's name.
pub const LOCK_FILE: &str = "lock.json";

/// The programs directory's name.
pub const PROGRAMS_DIR: &str = "programs";

/// The skills directory's name.
pub const SKILLS_DIR: &str = "skills";

/// Where this computer keeps the defaults: `CODER_DEFAULTS`, else
/// `~/.openagents/coder-defaults`; `None` when the variable says `off`
/// or there is no home.
#[must_use]
pub fn directory() -> Option<PathBuf> {
    resolve(
        std::env::var(DIR_ENV).ok().as_deref(),
        std::env::var_os("HOME").as_deref(),
    )
}

fn resolve(env: Option<&str>, home: Option<&std::ffi::OsStr>) -> Option<PathBuf> {
    match env.map(str::trim) {
        Some("off" | "0" | "no" | "false") => None,
        Some(dir) if !dir.is_empty() => Some(PathBuf::from(dir)),
        _ => home.filter(|home| !home.is_empty()).map(|home| {
            PathBuf::from(home)
                .join(".openagents")
                .join("coder-defaults")
        }),
    }
}

/// The programs directory under the defaults, when the defaults exist.
#[must_use]
pub fn programs_dir() -> Option<PathBuf> {
    directory()
        .map(|dir| dir.join(PROGRAMS_DIR))
        .filter(|dir| dir.is_dir())
}

/// One skill an admitted extension supplies.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Skill {
    /// The file name without `.md`.
    pub name: String,
    pub text: String,
    /// The file's `sha256:` digest.
    pub digest: String,
}

/// The defaults a session admits.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Admitted {
    /// The lock as the sync wrote it.
    pub defaults: Defaults,
    /// The lock file's `sha256:` digest, which a run records.
    pub digest: String,
    /// The admitted programs whose files are here, by slug.
    pub programs: Vec<String>,
    /// Admitted programs the lock names whose files aren't here; they
    /// aren't granted.
    pub missing: Vec<String>,
    /// The admitted extensions' skills.
    pub skills: Vec<Skill>,
    /// Admissions that lapsed before now, by subject release ID; the lock
    /// is stale and the sync should run again.
    pub lapsed: Vec<String>,
}

impl Admitted {
    /// The skills as one guidance block, or `None` without any.
    #[must_use]
    pub fn guidance(&self) -> Option<String> {
        if self.skills.is_empty() {
            return None;
        }
        let mut text = String::from("## Default skills\n\n");
        for skill in &self.skills {
            text.push_str(&format!(
                "### {} ({})\n\n{}\n\n",
                skill.name,
                skill.digest,
                skill.text.trim_end()
            ));
        }
        Some(text)
    }

    /// One line for the trace.
    #[must_use]
    pub fn line(&self) -> String {
        let mut line = format!(
            "defaults: release {} (version {}) lock {} admits {}",
            short(&self.defaults.release.id),
            self.defaults.version,
            self.digest,
            if self.programs.is_empty() {
                "no program".to_string()
            } else {
                self.programs.join(", ")
            }
        );
        if !self.missing.is_empty() {
            line.push_str(&format!("; not here: {}", self.missing.join(", ")));
        }
        if !self.lapsed.is_empty() {
            line.push_str(&format!(
                "; lapsed since the sync: {}",
                self.lapsed
                    .iter()
                    .map(|id| short(id))
                    .collect::<Vec<_>>()
                    .join(", ")
            ));
        }
        line
    }
}

fn short(id: &str) -> &str {
    &id[..id.len().min(12)]
}

/// The program slug an admitted definition names: the last path segment
/// of `<pubkey>:<package>/<program>`.
fn slug_of(definition: &str) -> &str {
    definition.rsplit('/').next().unwrap_or(definition)
}

/// Reads the defaults in `dir` as of `now`: `None` when there is no lock.
///
/// # Errors
///
/// Returns a sentence when the lock is present and can't be read or
/// isn't a lock.
pub fn read(dir: &Path, now: u64) -> Result<Option<Admitted>, String> {
    let lock_path = dir.join(LOCK_FILE);
    let bytes = match std::fs::read(&lock_path) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(format!("{}: {error}", lock_path.display())),
    };
    let defaults =
        parse_lock(&bytes).map_err(|error| format!("{}: {error}", lock_path.display()))?;
    let digest = nostr::contracts::digest_bytes(&bytes);
    let mut programs = Vec::new();
    let mut missing = Vec::new();
    let mut lapsed = Vec::new();
    for admitted in &defaults.admitted {
        if admitted.expires_at <= now {
            lapsed.push(admitted.subject.id.clone());
            continue;
        }
        let slug = slug_of(&admitted.definition).to_string();
        if dir
            .join(PROGRAMS_DIR)
            .join(format!("{slug}.json"))
            .is_file()
        {
            if !programs.contains(&slug) {
                programs.push(slug);
            }
        } else {
            missing.push(slug);
        }
    }
    let mut skills = Vec::new();
    if let Ok(entries) = std::fs::read_dir(dir.join(SKILLS_DIR)) {
        let mut paths: Vec<PathBuf> = entries
            .filter_map(Result::ok)
            .map(|entry| entry.path())
            .filter(|path| path.extension().is_some_and(|ext| ext == "md") && path.is_file())
            .collect();
        paths.sort();
        for path in paths {
            let text = std::fs::read_to_string(&path)
                .map_err(|error| format!("{}: {error}", path.display()))?;
            skills.push(Skill {
                name: path
                    .file_stem()
                    .map(|name| name.to_string_lossy().into_owned())
                    .unwrap_or_default(),
                digest: nostr::contracts::digest_bytes(text.as_bytes()),
                text,
            });
        }
    }
    Ok(Some(Admitted {
        defaults,
        digest,
        programs,
        missing,
        skills,
        lapsed,
    }))
}

/// [`read`] of [`directory`] as of now; a directory without a lock, or no
/// directory, is `None`.
///
/// # Errors
///
/// As [`read`].
pub fn current() -> Result<Option<Admitted>, String> {
    let Some(dir) = directory() else {
        return Ok(None);
    };
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    read(&dir, now)
}

#[cfg(test)]
mod tests {
    use super::*;
    use knowledge::xp::defaults::{Admitted as Row, EventPointerRecord, lock_document};

    fn lock(expires_at: u64) -> Vec<u8> {
        lock_document(&Defaults {
            release: EventPointerRecord {
                id: "aa".repeat(32),
                pubkey: "bb".repeat(32),
                kind: 3184,
            },
            version: "1".into(),
            manifest: format!("sha256:{}", "cc".repeat(32)),
            admitted: vec![
                Row {
                    subject: EventPointerRecord {
                        id: "dd".repeat(32),
                        pubkey: "ee".repeat(32),
                        kind: 3184,
                    },
                    definition: format!("{}:project-map/project-map", "ee".repeat(32)),
                    admission: format!("sha256:{}", "ff".repeat(32)),
                    expires_at,
                },
                Row {
                    subject: EventPointerRecord {
                        id: "11".repeat(32),
                        pubkey: "ee".repeat(32),
                        kind: 3184,
                    },
                    definition: format!("{}:code-finder/code-finder", "ee".repeat(32)),
                    admission: format!("sha256:{}", "22".repeat(32)),
                    expires_at,
                },
            ],
            lapsed: Vec::new(),
        })
    }

    #[test]
    fn the_lock_grants_the_programs_that_are_here_and_names_the_rest() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(read(dir.path(), 1_000).unwrap(), None);
        std::fs::write(dir.path().join(LOCK_FILE), lock(2_000)).unwrap();
        std::fs::create_dir_all(dir.path().join(PROGRAMS_DIR)).unwrap();
        std::fs::write(
            dir.path().join(PROGRAMS_DIR).join("project-map.json"),
            b"{}",
        )
        .unwrap();
        std::fs::create_dir_all(dir.path().join(SKILLS_DIR)).unwrap();
        std::fs::write(
            dir.path().join(SKILLS_DIR).join("map-first.md"),
            "Map the project before answering.\n",
        )
        .unwrap();

        let admitted = read(dir.path(), 1_000).unwrap().unwrap();
        assert_eq!(admitted.programs, vec!["project-map".to_string()]);
        assert_eq!(admitted.missing, vec!["code-finder".to_string()]);
        assert!(admitted.lapsed.is_empty());
        assert_eq!(admitted.skills.len(), 1);
        assert_eq!(admitted.skills[0].name, "map-first");
        let guidance = admitted.guidance().unwrap();
        assert!(guidance.starts_with("## Default skills"));
        assert!(guidance.contains("Map the project before answering."));
        assert!(admitted.line().contains("admits project-map"));
        assert!(admitted.line().contains("not here: code-finder"));
        assert_eq!(
            admitted.digest,
            nostr::contracts::digest_bytes(&lock(2_000))
        );

        // A lapsed admission grants nothing and is named.
        let later = read(dir.path(), 2_000).unwrap().unwrap();
        assert!(later.programs.is_empty());
        assert_eq!(later.lapsed.len(), 2);
        assert!(later.line().contains("lapsed since the sync"));

        // A file that isn't a lock is an error, not an empty grant.
        std::fs::write(dir.path().join(LOCK_FILE), b"{\"v\":\"other\"}").unwrap();
        assert!(read(dir.path(), 1_000).is_err());
    }

    #[test]
    fn the_directory_follows_the_environment_and_off_means_none() {
        let home = std::ffi::OsStr::new("/home/t");
        assert_eq!(
            resolve(None, Some(home)),
            Some(PathBuf::from("/home/t/.openagents/coder-defaults"))
        );
        assert_eq!(
            resolve(Some("/elsewhere"), Some(home)),
            Some(PathBuf::from("/elsewhere"))
        );
        assert_eq!(resolve(Some("off"), Some(home)), None);
        assert_eq!(resolve(Some(""), None), None);
    }
}
