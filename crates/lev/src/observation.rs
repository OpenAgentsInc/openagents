//! Raw measurement references, separate from calibration admission grants.
//!
//! A store records the identity the door actually published. In particular,
//! a historical package path is not rewritten into a release identity. These
//! references prove which retained rows a report used, not which artifact
//! bytes a historical runtime loaded, and never grant probability admission.

use std::collections::BTreeSet;
use std::path::Path;

use gym::row::{DoorIdentity, Row};
use gym::store::Store;
use serde::{Deserialize, Serialize};

/// A digest-pinned selection of observed rows in a Gym result store.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ObservationRef {
    /// Store path relative to the manifest.
    pub record: String,
    /// SHA-256 of the complete store, including other doors' rows.
    pub sha256: String,
    /// Suite id and content digest carried by the selected rows.
    pub suite: String,
    /// The suite's content digest.
    pub suite_digest: String,
    /// Partition to select. Use separate references for separate partitions.
    pub partition_id: String,
    /// Run label, retained exactly as written by the harness.
    pub door: String,
    /// Published identity, retained without substituting a release name.
    pub door_identity: DoorIdentity,
    /// Expected number of selected rows, including refusals.
    pub rows: usize,
}

/// Why an observational reference does not match its retained evidence.
#[derive(Debug, thiserror::Error)]
#[error("observationRef {record}: {reason}")]
pub struct Fault {
    /// Store path declared by the reference.
    pub record: String,
    /// The failed check.
    pub reason: String,
}

impl ObservationRef {
    /// Checks the file digest, receipt chain, row validity, and selection.
    ///
    /// This does not fit a calibration map or admit any question family.
    ///
    /// # Errors
    ///
    /// Returns an error for missing, changed, malformed, or mismatched evidence.
    pub fn check(&self, beside: &Path) -> Result<(), Fault> {
        let fail = |reason: String| Fault {
            record: self.record.clone(),
            reason,
        };
        if self.rows == 0
            || [
                &self.record,
                &self.suite,
                &self.suite_digest,
                &self.partition_id,
                &self.door,
            ]
            .iter()
            .any(|field| field.trim().is_empty())
        {
            return Err(fail("a selection must name a nonempty set of rows".into()));
        }
        let path = beside.join(&self.record);
        let digest = crate::manifest::digest_of(&path).map_err(|error| fail(error.to_string()))?;
        if digest != self.sha256 {
            return Err(fail(format!(
                "SHA-256 differs: expected {}, found {digest}",
                self.sha256
            )));
        }
        let values = Store::at(path)
            .verified_rows()
            .map_err(|error| fail(error.to_string()))?;
        let mut items = BTreeSet::new();
        for value in values {
            let row: Row =
                serde_json::from_value(value).map_err(|error| fail(error.to_string()))?;
            row.check().map_err(|error| fail(error.to_string()))?;
            if row.door != self.door || row.suite != self.suite || row.split != self.partition_id {
                continue;
            }
            if row.suite_digest != self.suite_digest || row.door_identity != self.door_identity {
                return Err(fail(format!(
                    "{} has different suite or door provenance",
                    row.item_id
                )));
            }
            if !items.insert(row.item_id) {
                return Err(fail(
                    "the selection contains repeated items; select a single trial store".into(),
                ));
            }
        }
        if items.len() != self.rows {
            return Err(fail(format!(
                "expected {} rows, found {}",
                self.rows,
                items.len()
            )));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn evidence(dir: &Path) -> ObservationRef {
        let mut row = Row::new("outside", "suite-digest", "item-1", "band");
        row.split = "development".into();
        row.family = "routing".into();
        row.door_identity = DoorIdentity::published("lev", "base", "package-id");
        let identity = row.door_identity.clone();
        let path = dir.join("rows.jsonl");
        Store::at(&path)
            .append(&row.refused(gym::row::RefusalCode::Other("fixture refusal".into())))
            .unwrap();
        ObservationRef {
            record: "rows.jsonl".into(),
            sha256: crate::manifest::digest_of(&path).unwrap(),
            suite: "outside".into(),
            suite_digest: "suite-digest".into(),
            partition_id: "development".into(),
            door: "band".into(),
            door_identity: identity,
            rows: 1,
        }
    }

    #[test]
    fn changed_bytes_counts_and_identities_are_rejected() {
        let dir = tempfile::tempdir().unwrap();
        let reference = evidence(dir.path());
        reference.check(dir.path()).unwrap();
        let mut changed = reference.clone();
        changed.door_identity.adapter = "lev-adapted@2".into();
        assert!(
            changed.check(dir.path()).is_err(),
            "do not relabel a published identity"
        );
        changed = reference.clone();
        changed.rows += 1;
        assert!(changed.check(dir.path()).is_err());
        let path = dir.path().join("rows.jsonl");
        let text = std::fs::read_to_string(&path).unwrap();
        std::fs::write(&path, text.replace("item-1", "item-2")).unwrap();
        changed = reference.clone();
        changed.sha256 = crate::manifest::digest_of(&path).unwrap();
        assert!(
            changed.check(dir.path()).is_err(),
            "a new digest cannot repair a broken receipt chain"
        );
        std::fs::write(dir.path().join("rows.jsonl"), "{}\n").unwrap();
        assert!(reference.check(dir.path()).is_err());
    }

    #[test]
    fn observation_cannot_grant_admission_or_change_calibration_references() {
        let dir = tempfile::tempdir().unwrap();
        let reference = evidence(dir.path());
        let mut manifest = crate::manifest::Manifest::load(
            Path::new(env!("CARGO_MANIFEST_DIR")).join("manifests/lev-adapted-v2.json"),
        )
        .unwrap();
        let calibration = manifest.eval_ref.clone();
        manifest.source = dir.path().into();
        manifest.observation_ref = vec![reference];
        manifest.check_observation_refs().unwrap();
        assert_eq!(manifest.eval_ref, calibration);
        assert!(!manifest.admits("routing"));
        assert!(manifest.admitted_families().is_empty());
    }
}
