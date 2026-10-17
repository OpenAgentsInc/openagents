//! Explicit operator authorization for a new attempt after a saved refusal.
use crate::{Store, db::DbError};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// This operation never changes the refused receipt or resends an uncertain one.
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RetryRequest {
    pub computer: String,
    pub operation: String,
    pub resource: String,
    pub action: String,
    pub reason: String,
}

fn refused() -> DbError {
    DbError("The saved request cannot be retried with these settings.".into())
}

impl Store {
    /// Authorize at most eight retries for one definitively refused operation.
    /// The caller must fix the original cause first and hold database authority.
    pub async fn retry_rejected_lifecycle(
        &self,
        owner: &str,
        request: &RetryRequest,
    ) -> Result<String, DbError> {
        if !matches!(request.action.as_str(), "suspend" | "wake" | "delete")
            || request.operation.is_empty()
            || request.operation.len() > 128
            || !request.operation.bytes().all(|b| b.is_ascii_graphic())
            || request.reason.trim().is_empty()
            || request.reason.len() > 512
            || request.reason.chars().any(char::is_control)
        {
            return Err(refused());
        }
        let mut client = self.db.independent().await?;
        let tx = client.transaction().await?;
        crate::usage::lock(&tx).await?;
        let row = tx
            .query_opt(
                &format!(
                    "SELECT {} FROM computers.computers WHERE id=$1 AND owner=$2 FOR UPDATE",
                    crate::store::COLUMNS
                ),
                &[&request.computer, &owner],
            )
            .await?
            .ok_or_else(refused)?;
        let entry = crate::store::entry(&row)?;
        let binding = entry.binding.as_ref().ok_or_else(refused)?;
        if binding.owner != owner
            || entry.record.validate().is_err()
            || !binding
                .artifact
                .as_ref()
                .is_some_and(|a| a.kind == computer_driver::ArtifactKind::SpaceTemplate)
        {
            return Err(refused());
        }
        let saved=tx.query_opt("SELECT resource,action,disposition,provider_operation FROM computers.lifecycle_operations WHERE operation=$1 FOR UPDATE",&[&request.operation]).await?.ok_or_else(refused)?;
        if saved.get::<_, String>(0) != request.resource
            || saved.get::<_, String>(1) != request.action
        {
            return Err(refused());
        }
        if let Some(retry)=tx.query_opt("SELECT next_operation,owner,computer,binding_fingerprint,resource,action,reason FROM computers.lifecycle_retries WHERE operation=$1",&[&request.operation]).await? {
            if retry.get::<_,String>(1)!=owner || retry.get::<_,String>(2)!=entry.id
                || retry.get::<_,String>(3)!=binding.fingerprint || retry.get::<_,String>(4)!=request.resource
                || retry.get::<_,String>(5)!=request.action || retry.get::<_,String>(6)!=request.reason {
                return Err(refused());
            }
            let next=retry.get(0);tx.commit().await?;return Ok(next);
        }
        if entry.phase == "deleted"
            || entry.record.resource() != Some(request.resource.as_str())
            || saved.get::<_, String>(2) != "rejected"
            || saved.get::<_, Option<String>>(3).is_some()
            || request.action == "wake" && entry.desired == crate::store::Desired::Deleted
        {
            return Err(refused());
        }
        if request.action == "wake" {
            crate::storage::allowance::account_open(&tx, owner).await?;
        }
        let parent=tx.query_opt("SELECT root_operation,owner,computer,resource,action FROM computers.lifecycle_retries WHERE next_operation=$1",&[&request.operation]).await?;
        let root = if let Some(parent) = parent {
            if parent.get::<_, String>(1) != owner
                || parent.get::<_, String>(2) != entry.id
                || parent.get::<_, String>(3) != request.resource
                || parent.get::<_, String>(4) != request.action
            {
                return Err(refused());
            }
            parent.get::<_, String>(0)
        } else {
            request.operation.clone()
        };
        let count: i64 = tx
            .query_one(
                "SELECT count(*) FROM computers.lifecycle_retries WHERE root_operation=$1",
                &[&root],
            )
            .await?
            .get(0);
        if count >= 8 {
            return Err(DbError(
                "The request has reached its retry limit. Inspect the computer before continuing."
                    .into(),
            ));
        }
        let mut digest = Sha256::new();
        digest.update(b"openagents.computers.lifecycle-retry.v1\0");
        for part in [&request.operation, &request.resource, &request.action] {
            digest.update(part.as_bytes());
            digest.update([0]);
        }
        let next = format!("lifecycle-retry:{:x}", digest.finalize());
        tx.execute("INSERT INTO computers.lifecycle_retries(operation,next_operation,root_operation,owner,computer,binding_fingerprint,resource,action,reason) VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9)",&[&request.operation,&next,&root,&owner,&entry.id,&binding.fingerprint,&request.resource,&request.action,&request.reason]).await?;
        tx.execute("UPDATE computers.computers SET next_attempt_at=clock_timestamp(),attempts=0,last_error=NULL WHERE id=$1 AND owner=$2",&[&entry.id,&owner]).await?;
        tx.commit().await?;
        Ok(next)
    }
}
