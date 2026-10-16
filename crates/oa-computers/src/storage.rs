//! Account-bound storage observations. These are provider facts, not totals or prices.

use crate::store::{COLUMNS, PgLease, entry};
use coder_working_computer::Computer;
use coder_working_computer::store::{Result as StoreResult, StoreError};
use computer_driver::{Outcome, StorageObservation};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

mod deleted;
pub use deleted::{DeletedStorageJob, DeletedStorageObserver};

fn evidence(resource: &str, outcome: &Outcome<StorageObservation>) -> Value {
    match outcome {
        Outcome::Done { value } if value.computer == resource && value.validate().is_ok() => {
            json!({"source":"provider_storage","status":"done","storage_observation":value,
                "provider_seconds":null,"customer_charge":null})
        }
        Outcome::Done { .. } => json!({"source":"provider_storage","status":"invalid",
            "storage_observation":null,"provider_seconds":null,"customer_charge":null}),
        Outcome::Failed { .. } | Outcome::Unknown { .. } => {
            // Provider errors can contain paths, URLs, credentials, or workload text.
            json!({"source":"provider_storage","status":"unknown",
                "storage_observation":null,"provider_seconds":null,"customer_charge":null})
        }
    }
}

fn io(_: impl std::fmt::Display) -> StoreError {
    StoreError::Io("The storage usage record is unavailable.")
}

impl PgLease {
    /// Record only while the same worker, revision, and deployment still own
    /// this observation. Repeated identical observations at one revision share a key.
    pub async fn observe_storage(
        &self,
        current: &Computer,
        binding_fingerprint: Option<&str>,
        resource: &str,
        outcome: &Outcome<StorageObservation>,
        observed_ms: u64,
    ) -> StoreResult<()> {
        if current.id != self.id || current.resource() != Some(resource) {
            return Err(StoreError::Corrupt("Storage resource identity mismatch."));
        }
        let mut connection = self.store.db.independent().await.map_err(io)?;
        let tx = connection.transaction().await.map_err(io)?;
        crate::usage::lock(&tx).await.map_err(io)?;
        let revision = i64::try_from(current.revision)
            .map_err(|_| StoreError::Corrupt("Invalid computer revision."))?;
        let row = tx
            .query_opt(
                &format!(
                    "SELECT {COLUMNS} FROM computers.computers
                    WHERE id=$1 AND revision=$2 AND lease_holder=$3 AND lease_until>clock_timestamp()
                    FOR UPDATE"
                ),
                &[&self.id, &revision, &self.holder],
            )
            .await
            .map_err(io)?
            .ok_or(StoreError::Busy)?;
        let stored = entry(&row).map_err(io)?;
        if stored
            .binding
            .as_ref()
            .map(|binding| binding.fingerprint.as_str())
            != binding_fingerprint
            || stored.record != *current
        {
            return Err(StoreError::Busy);
        }
        let value = evidence(resource, outcome);
        let digest = format!(
            "{:x}",
            Sha256::digest(serde_json::to_vec(&(resource, &value)).map_err(io)?)
        );
        crate::usage::record(
            &tx,
            &stored,
            "observed",
            &format!("storage:{}:{digest}", current.revision),
            i64::try_from(observed_ms).map_err(io)?,
            value,
        )
        .await
        .map_err(io)?;
        tx.commit().await.map_err(io)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn failures_never_retain_provider_error_text_or_infer_cleanup() {
        for outcome in [
            Outcome::unknown("private-token"),
            Outcome::failed("private-token"),
        ] {
            let value = evidence("resource", &outcome);
            assert!(!value.to_string().contains("private-token"));
            assert!(value["storage_observation"].is_null());
            assert_eq!(value["status"], "unknown");
        }
    }
}
