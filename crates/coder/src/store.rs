//! The offline cache: package bytes kept by digest so a lock resolves
//! without the tree it was read from.
//!
//! A [`crate::package::Lock`] pins every reference to a digest of bytes
//! on disk. Those bytes are the run's world — the program, its question
//! sets, sources, policies, and capability manifests — and they usually
//! live under a resolution root the caller owns. When that root is gone
//! — another machine, a cleaned checkout, an offline host — the lock
//! still says exactly which bytes a run needs; what it cannot do is
//! produce them.
//!
//! A [`Store`] closes that gap. [`Store::retain`] copies every byte a
//! lock pins into a directory keyed by digest, verifying each pin
//! against its on-disk bytes before it is kept — a lock that does not
//! verify against its own tree is not cached, because caching it would
//! only preserve the disagreement. [`Store::rebuild`] writes those bytes
//! back to the paths the lock recorded, every digest checked on the way
//! out, so a rebuilt tree resolves to the same lock digest it had
//! online. Rebuild is all-or-nothing: it inventories the store first
//! and refuses without writing when any pinned digest is absent, so an
//! incomplete cache can never leave a half-materialized tree looking
//! like a resolved one.
//!
//! The store verifies on read, never trusts names: a file at a digest's
//! path whose bytes do not produce that digest is tampering, surfaced
//! as [`Refusal::Tampered`] rather than returned as content. And a
//! store is a cache, not authority — it supplies bytes for pins a lock
//! already names; it cannot add a reference, widen a grant, or stand
//! in for the trust markings the lock itself carries.

use std::collections::BTreeMap;
use std::fmt;
use std::path::{Path, PathBuf};

use crate::package::{Lock, Pin, digest};

/// A directory of package bodies, each named by the digest of its own
/// bytes.
pub struct Store {
    root: PathBuf,
}

/// Every pin a lock holds, walked as `(component, pin)` pairs —
/// dependencies included, each under `packages/<slug>/...` — the same
/// components [`Lock::pins`] flattens to, keeping the `found` path the
/// flat map drops.
fn pins(lock: &Lock) -> Vec<(String, &Pin)> {
    let mut out = Vec::new();
    walk(lock, "", &mut out);
    out
}

fn walk<'a>(lock: &'a Lock, prefix: &str, out: &mut Vec<(String, &'a Pin)>) {
    out.push((format!("{prefix}programs/{}", lock.slug), &lock.program));
    for (dir, list) in [
        ("questions", &lock.questions),
        ("sources", &lock.sources),
        ("policies", &lock.policies),
        ("capabilities", &lock.capabilities),
    ] {
        for (name, pin) in list {
            out.push((format!("{prefix}{dir}/{name}"), pin));
        }
    }
    for (slug, locked) in &lock.dependencies {
        out.push((format!("{prefix}packages/{slug}"), &locked.record));
        walk(&locked.lock, &format!("{prefix}packages/{slug}/"), out);
    }
}

impl Store {
    /// The store under `root`. Opening creates nothing; the first
    /// [`Store::put`] makes the directory.
    #[must_use]
    pub fn open(root: PathBuf) -> Self {
        Self { root }
    }

    /// Where a digest's bytes live inside the store.
    fn path(&self, digest: &str) -> PathBuf {
        self.root.join(digest)
    }

    /// Keep `bytes` under the digest of themselves; idempotent — the
    /// same bytes put twice are one entry. Returns the digest they are
    /// filed under, which is the only name the store answers to.
    ///
    /// # Errors
    ///
    /// Returns [`Refusal::Io`] when the store cannot be written.
    pub fn put(&self, bytes: &str) -> Result<String, Refusal> {
        let digest = digest(bytes);
        std::fs::create_dir_all(&self.root).map_err(|error| Refusal::Io {
            at: self.root.display().to_string(),
            reason: error.to_string(),
        })?;
        std::fs::write(self.path(&digest), bytes).map_err(|error| Refusal::Io {
            at: self.path(&digest).display().to_string(),
            reason: error.to_string(),
        })?;
        Ok(digest)
    }

    /// The bytes filed under `digest`, verified on the way out: a store
    /// file whose contents no longer produce its name is tampering, not
    /// content.
    ///
    /// # Errors
    ///
    /// Returns [`Refusal::Tampered`] when the stored bytes fail their
    /// own digest, and [`Refusal::Io`] when the file cannot be read.
    pub fn get(&self, digest: &str) -> Result<Option<String>, Refusal> {
        let path = self.path(digest);
        match std::fs::read_to_string(&path) {
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(error) => Err(Refusal::Io {
                at: path.display().to_string(),
                reason: error.to_string(),
            }),
            Ok(bytes) if crate::package::digest(&bytes) == digest => Ok(Some(bytes)),
            Ok(bytes) => Err(Refusal::Tampered {
                at: path.display().to_string(),
                stated: digest.to_string(),
                found: crate::package::digest(&bytes),
            }),
        }
    }

    /// Whether `digest`'s bytes are held — presence, not a promise:
    /// [`Store::get`] still verifies before returning them.
    #[must_use]
    pub fn has(&self, digest: &str) -> bool {
        self.path(digest).is_file()
    }

    /// Copy every byte `lock` pins into the store, verified first:
    /// each pin's digest is recomputed against its bytes under `root`,
    /// and one that does not hold refuses the whole retain — a lock
    /// that disagrees with its own tree is not a record worth caching.
    /// Returns how many pins were stored.
    ///
    /// # Errors
    ///
    /// Returns the first [`Refusal`] a pin produces: [`Refusal::Missing`]
    /// when the recorded path is gone, [`Refusal::Tampered`] when the
    /// bytes do not match the pin, [`Refusal::Io`] when a read or write
    /// fails.
    pub fn retain(&self, lock: &Lock, root: &Path) -> Result<usize, Refusal> {
        let pins = pins(lock);
        // Verify everything before keeping anything: a retain that
        // stops halfway would hold bytes for a lock that never held.
        let mut bodies = BTreeMap::new();
        for (component, pin) in &pins {
            let at = root.join(&pin.found);
            let bytes = std::fs::read_to_string(&at).map_err(|error| match error.kind() {
                std::io::ErrorKind::NotFound => Refusal::Missing {
                    component: component.clone(),
                    at: at.display().to_string(),
                },
                _ => Refusal::Io {
                    at: at.display().to_string(),
                    reason: error.to_string(),
                },
            })?;
            let found = digest(&bytes);
            if found != pin.digest {
                return Err(Refusal::Tampered {
                    at: at.display().to_string(),
                    stated: pin.digest.clone(),
                    found,
                });
            }
            bodies.insert(pin.digest.clone(), bytes);
        }
        for bytes in bodies.values() {
            self.put(bytes)?;
        }
        Ok(pins.len())
    }

    /// Write every byte `lock` pins back to its recorded path under
    /// `root`, from the store alone. All-or-nothing: the inventory is
    /// checked before the first file is written, so an incomplete cache
    /// refuses cleanly instead of leaving a partial tree.
    ///
    /// Rollback is rebuild of an older lock — the store answers every
    /// digest the lock pins, so a cache that holds both sides makes
    /// either direction complete, and a pruned one refuses rather than
    /// approximating the earlier world.
    ///
    /// # Errors
    ///
    /// Returns [`Refusal::Offline`] naming the first component whose
    /// digest the store does not hold, [`Refusal::Tampered`] when stored
    /// bytes fail their own digest, and [`Refusal::Io`] when a write
    /// fails.
    pub fn rebuild(&self, lock: &Lock, root: &Path) -> Result<(), Refusal> {
        let pins = pins(lock);
        let mut bodies = BTreeMap::new();
        for (component, pin) in &pins {
            match self.get(&pin.digest)? {
                Some(bytes) => {
                    bodies.insert(component.clone(), (pin.found.clone(), bytes));
                }
                None => {
                    return Err(Refusal::Offline {
                        component: component.clone(),
                        digest: pin.digest.clone(),
                    });
                }
            }
        }
        for (found, bytes) in bodies.values() {
            let at = root.join(found);
            if let Some(parent) = at.parent() {
                std::fs::create_dir_all(parent).map_err(|error| Refusal::Io {
                    at: parent.display().to_string(),
                    reason: error.to_string(),
                })?;
            }
            std::fs::write(&at, bytes).map_err(|error| Refusal::Io {
                at: at.display().to_string(),
                reason: error.to_string(),
            })?;
        }
        Ok(())
    }
}

/// Why the store refused.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Refusal {
    /// The lock names bytes that are not where they were recorded —
    /// a stale or replaced tree, surfaced rather than read anyway.
    Missing { component: String, at: String },
    /// The lock pins bytes the store does not hold. The component and
    /// the digest name exactly what is missing; nothing is substituted.
    Offline { component: String, digest: String },
    /// Bytes failed their own digest — on disk during a retain, or in
    /// the store during a read. Tampering is named, never returned.
    Tampered {
        at: String,
        stated: String,
        found: String,
    },
    /// A read or write the host could not perform.
    Io { at: String, reason: String },
}

impl fmt::Display for Refusal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Missing { component, at } => {
                write!(f, "{component}: nothing at {at} to retain")
            }
            Self::Offline { component, digest } => {
                write!(
                    f,
                    "{component}: digest {digest} is not in the store, and offline \
                     resolution cannot fetch it"
                )
            }
            Self::Tampered { at, stated, found } => {
                write!(f, "{at}: bytes digest to {found}, not the stated {stated}")
            }
            Self::Io { at, reason } => write!(f, "{at}: {reason}"),
        }
    }
}

impl std::error::Error for Refusal {}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::*;
    use crate::package::{Locked, Trust};

    const TOP_PROGRAM: &str = r#"{"v":1,"slug":"top","steps":[]}"#;
    const ASK_SET: &str =
        r#"{"v":1,"id":"suite.ask.v1","questions":{"q":{"type":"noul","instructions":"i"}}}"#;
    const LEAF_PROGRAM: &str = r#"{"v":1,"slug":"leaf","steps":[]}"#;

    /// The leaf package record, pinning its own program. Written to
    /// `packages/leaf.json`; its bytes change with the program's, so it
    /// is built here rather than kept as a literal.
    fn leaf_package() -> String {
        serde_json::to_string(&serde_json::json!({
            "v": 1, "slug": "leaf", "version": "1.0.0", "publisher": "suite",
            "program": {"name": "leaf", "digest": digest(LEAF_PROGRAM)}
        }))
        .unwrap()
    }

    /// The package record that resolves to `lock()` under `tree()`.
    fn top_package() -> crate::package::Package {
        serde_json::from_value(serde_json::json!({
            "v": 1, "slug": "top", "version": "1.0.0", "publisher": "suite",
            "provenance": "local",
            "program": {"name": "top", "digest": digest(TOP_PROGRAM)},
            "questions": [{"name": "suite.ask.v1", "digest": digest(ASK_SET)}],
            "requires": [{"package": "leaf"}]
        }))
        .unwrap()
    }

    /// A lock pinning two components and one dependency, every `found`
    /// a path under the tree `tree()` writes.
    fn lock() -> Lock {
        let pin = |found: &str, bytes: &str| Pin {
            digest: digest(bytes),
            found: found.to_string(),
        };
        Lock {
            v: 1,
            slug: "top".to_string(),
            publisher: "suite".to_string(),
            provenance: "local".to_string(),
            program: pin("programs/top.json", TOP_PROGRAM),
            questions: BTreeMap::from([(
                "suite.ask.v1".to_string(),
                pin("questions/ask.json", ASK_SET),
            )]),
            sources: BTreeMap::new(),
            policies: BTreeMap::new(),
            capabilities: BTreeMap::new(),
            dependencies: BTreeMap::from([(
                "leaf".to_string(),
                Locked {
                    trust: Trust::Untrusted,
                    record: pin("packages/leaf.json", &leaf_package()),
                    lock: Box::new(Lock {
                        v: 1,
                        slug: "leaf".to_string(),
                        publisher: "suite".to_string(),
                        provenance: String::new(),
                        program: pin("programs/leaf.json", LEAF_PROGRAM),
                        questions: BTreeMap::new(),
                        sources: BTreeMap::new(),
                        policies: BTreeMap::new(),
                        capabilities: BTreeMap::new(),
                        dependencies: BTreeMap::new(),
                    }),
                },
            )]),
        }
    }

    /// The tree `lock()` was resolved against: the top program and its
    /// question set, the leaf package record, and the leaf's program.
    fn tree(root: &Path) {
        for (found, bytes) in [
            ("programs/top.json", TOP_PROGRAM),
            ("questions/ask.json", ASK_SET),
            ("programs/leaf.json", LEAF_PROGRAM),
            ("packages/leaf.json", leaf_package().as_str()),
        ] {
            let at = root.join(found);
            std::fs::create_dir_all(at.parent().unwrap()).unwrap();
            std::fs::write(at, bytes).unwrap();
        }
    }

    #[test]
    fn put_gets_the_same_bytes_by_digest() {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::open(dir.path().to_path_buf());
        let filed = store.put("a body").unwrap();
        assert!(store.has(&filed));
        assert_eq!(store.get(&filed).unwrap().as_deref(), Some("a body"));
        assert_eq!(
            store.get(&digest("a body")).unwrap().as_deref(),
            Some("a body")
        );
    }

    #[test]
    fn a_missing_digest_is_none_not_an_error() {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::open(dir.path().to_path_buf());
        assert_eq!(store.get(&digest("never stored")).unwrap(), None);
    }

    #[test]
    fn tampered_store_bytes_are_refused_not_returned() {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::open(dir.path().to_path_buf());
        let filed = store.put("honest bytes").unwrap();
        std::fs::write(dir.path().join(&filed), "replaced bytes").unwrap();
        match store.get(&filed) {
            Err(Refusal::Tampered { stated, found, .. }) => {
                assert_eq!(stated, filed);
                assert_eq!(found, digest("replaced bytes").as_str());
            }
            other => panic!("tampering is refused, not {other:?}"),
        }
    }

    #[test]
    fn retain_then_rebuild_restores_the_tree_offline() {
        let source = tempfile::tempdir().unwrap();
        tree(source.path());
        let store_dir = tempfile::tempdir().unwrap();
        let store = Store::open(store_dir.path().to_path_buf());
        assert_eq!(store.retain(&lock(), source.path()).unwrap(), 4);

        let rebuilt = tempfile::tempdir().unwrap();
        store.rebuild(&lock(), rebuilt.path()).unwrap();
        assert_eq!(
            std::fs::read_to_string(rebuilt.path().join("programs/leaf.json")).unwrap(),
            LEAF_PROGRAM
        );
        // The rebuilt tree resolves to the same lock it was retained from.
        let relocked = crate::package::Package::resolve(rebuilt.path(), &top_package());
        assert_eq!(relocked.unwrap().digest(), lock().digest());
    }

    #[test]
    fn rebuild_with_an_incomplete_cache_refuses_and_writes_nothing() {
        let source = tempfile::tempdir().unwrap();
        tree(source.path());
        let store_dir = tempfile::tempdir().unwrap();
        let store = Store::open(store_dir.path().to_path_buf());
        store.retain(&lock(), source.path()).unwrap();
        // Prune one digest: the cache no longer holds the leaf program.
        std::fs::remove_file(store_dir.path().join(digest(LEAF_PROGRAM))).unwrap();

        let rebuilt = tempfile::tempdir().unwrap();
        match store.rebuild(&lock(), rebuilt.path()) {
            Err(Refusal::Offline { component, .. }) => {
                assert_eq!(component, "packages/leaf/programs/leaf");
            }
            other => panic!("an incomplete cache refuses, not {other:?}"),
        }
        assert_eq!(
            std::fs::read_dir(rebuilt.path()).unwrap().count(),
            0,
            "a refused rebuild leaves no partial tree"
        );
    }

    #[test]
    fn retain_refuses_a_pin_whose_recorded_path_is_gone() {
        let source = tempfile::tempdir().unwrap();
        tree(source.path());
        std::fs::remove_file(source.path().join("programs/leaf.json")).unwrap();
        let store = Store::open(tempfile::tempdir().unwrap().path().to_path_buf());
        match store.retain(&lock(), source.path()) {
            Err(Refusal::Missing { component, .. }) => {
                assert_eq!(component, "packages/leaf/programs/leaf");
            }
            other => panic!("a gone file refuses, not {other:?}"),
        }
    }

    #[test]
    fn retain_refuses_bytes_that_disagree_with_the_lock() {
        let source = tempfile::tempdir().unwrap();
        tree(source.path());
        // A stale or replaced file: the pin says one digest, disk says another.
        std::fs::write(source.path().join("questions/ask.json"), "changed").unwrap();
        let store = Store::open(tempfile::tempdir().unwrap().path().to_path_buf());
        match store.retain(&lock(), source.path()) {
            Err(Refusal::Tampered { stated, found, .. }) => {
                assert_eq!(stated, lock().questions["suite.ask.v1"].digest);
                assert_eq!(found, digest("changed"));
            }
            other => panic!("disagreeing bytes refuse, not {other:?}"),
        }
    }
}
