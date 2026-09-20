//! The committed manifests, checked against the committed records.
//!
//! A manifest is a claim about files that live somewhere else: calibration
//! records in this repository, and a package on one machine. The records are
//! here, so this suite checks them on every run, and the fault it exists to
//! catch is a record edited after a release was written — the artifact-shaped
//! version of the stale map that survived two adapter runs.
//!
//! The package is out of git at 133 MB, so the artifact check runs only when
//! the package is on the machine running the test.

use std::path::{Path, PathBuf};

use lev::manifest::{CONTRACT, MANIFEST_SCHEMA, Manifest};

/// Every manifest in `crates/lev/manifests`, by path.
fn committed() -> Vec<PathBuf> {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("manifests");
    let mut paths: Vec<PathBuf> = std::fs::read_dir(&dir)
        .expect("the manifests directory")
        .map(|entry| entry.expect("an entry").path())
        .filter(|path| path.extension().and_then(|extension| extension.to_str()) == Some("json"))
        .collect();
    paths.sort();
    assert!(!paths.is_empty(), "no manifest is committed");
    paths
}

fn load(path: &Path) -> Manifest {
    Manifest::load(path).unwrap_or_else(|fault| panic!("{}: {fault}", path.display()))
}

#[test]
fn every_committed_manifest_loads_and_declares_this_build_of_the_contract() {
    for path in committed() {
        let manifest = load(&path);
        assert_eq!(manifest.schema, MANIFEST_SCHEMA, "{}", path.display());
        assert_eq!(manifest.interface.contract, CONTRACT, "{}", path.display());
        assert!(!manifest.release().starts_with('@'), "{} has no name", path.display());
        assert_eq!(
            manifest.estimator.estimator, "l2",
            "{} names an estimator this door does not run",
            path.display()
        );
    }
}

#[test]
fn every_committed_manifest_names_a_policy_snapshot_it_can_resolve() {
    // A release that named no policy source would be a release nothing can
    // revoke, so the field is required and `Manifest::load` has already
    // refused a document without it by the time this runs. What is left to
    // check is that the path resolves to the snapshot this repository
    // publishes rather than to nothing.
    for path in committed() {
        let manifest = load(&path);
        let policy = manifest.policy();
        assert!(
            policy.source().exists(),
            "{} points at {}, which is not there",
            path.display(),
            policy.source().display()
        );
        lev::policy::read(policy.source())
            .unwrap_or_else(|trouble| panic!("{}: {trouble}", policy.source().display()));
        assert!(
            manifest.policy_snapshot.freshness_window_seconds
                <= lev::policy::DEFAULT_WINDOW_SECONDS,
            "{} accepts a window longer than the published one",
            path.display()
        );
    }
}

#[test]
fn every_eval_ref_matches_the_record_it_names() {
    // The check that would have caught the stale map: a record edited, or
    // refitted, or deleted, after the release that rests on it was written.
    for path in committed() {
        let manifest = load(&path);
        manifest
            .check_eval_refs()
            .unwrap_or_else(|fault| panic!("{}: {fault}", path.display()));
    }
}

#[test]
fn an_adapted_release_names_only_maps_fitted_against_itself() {
    // Until 2026-09-19 every committed calibration map was fitted against the
    // base model with no adapter attached, and this test read "an adapted
    // release names no measurement at all". That was the rule standing in for
    // itself: the thing worth refusing is a map fitted against a *different*
    // door, and with no adapted map in existence the two were the same
    // assertion. `lev-adapted@1` now names three maps fitted against
    // `lev-adapted@1`, so the test says what it meant.
    //
    // `check_eval_refs` does the comparison, because a rule a caller has to
    // remember to apply is a rule that gets skipped.
    for path in committed() {
        let manifest = load(&path);
        manifest
            .check_eval_refs()
            .unwrap_or_else(|fault| panic!("{}: {fault}", path.display()));
        for family in manifest.admitted_families() {
            assert!(
                manifest.eval_ref.iter().any(|reference| reference.family == family
                    && reference.admitted),
                "{} admits {family} with no admitted measurement",
                path.display()
            );
        }
    }
}

#[test]
fn the_base_release_admits_exactly_the_families_its_records_admit() {
    let manifest = load(&Path::new(env!("CARGO_MANIFEST_DIR")).join("manifests/lev-base-v1.json"));
    assert_eq!(manifest.release(), "lev-base@1");
    assert!(manifest.artifact.is_none(), "the operating system ships the base");
    assert_eq!(manifest.admitted_families(), vec!["routing"]);
    for family in ["severity", "urgency"] {
        let refused = manifest.eval_ref(family).expect("the record is still named");
        assert!(!refused.admitted, "{family} admitted");
        assert!(refused.verdict.contains("unverifiable"), "{}", refused.verdict);
    }
}

#[test]
fn the_package_is_checked_when_it_is_on_this_machine() {
    for path in committed() {
        let manifest = load(&path);
        let Some(artifact) = &manifest.artifact else { continue };
        if !artifact.resolved_path().is_dir() {
            eprintln!("{} is not on this machine; the artifact is not checked", artifact.path);
            continue;
        }
        manifest
            .check_artifact()
            .unwrap_or_else(|fault| panic!("{}: {fault}", path.display()));
    }
}
