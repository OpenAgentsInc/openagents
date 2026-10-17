//! Private storage allowances survive unknown outcomes and provider deletion.
//! An operator resolves them with a complete inventory, including recoverable
//! versions and unfinished uploads. This book does not settle customer charges.

use crate::{Store, db::DbError, store::Entry, usage::QUOTA_ERROR};
use coder_working_computer::{Computer, Fact, Phase};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;
use tokio_postgres::{GenericClient, Transaction};

pub(crate) async fn account_open(tx: &Transaction<'_>, owner: &str) -> Result<(), DbError> {
    let closed: bool = tx
        .query_one(
            "SELECT EXISTS(SELECT 1 FROM computers.account_cleanup WHERE owner=$1)",
            &[&owner],
        )
        .await?
        .get(0);
    if closed {
        return Err(DbError(QUOTA_ERROR.into()));
    }
    Ok(())
}

pub(crate) async fn reserve(
    tx: &Transaction<'_>,
    entry: &Entry,
    computer: &Computer,
) -> Result<(), DbError> {
    let boot = if matches!(
        computer.phase,
        Phase::New | Phase::Creating | Phase::Stopped
    ) {
        computer.boots.len() + 1
    } else {
        computer.boots.last().map_or(1, |boot| boot.number as usize)
    };
    let ordinal = i32::try_from(computer.creates.len().max(1))
        .map_err(|_| DbError("The computer has too many creation attempts.".into()))?;
    let boot =
        i32::try_from(boot).map_err(|_| DbError("The computer has too many boots.".into()))?;
    let bounds = tx
        .query_one(
            "SELECT max(storage_bytes_per_boot),max(checkpoints_per_boot)
             FROM computers.usage_limits
             WHERE scope='service' OR (scope='account' AND subject=$1)",
            &[&entry.owner],
        )
        .await?;
    let bytes: Option<i64> = bounds.get(0);
    let checkpoints: Option<i64> = bounds.get(1);
    let fingerprint = entry.binding.as_ref().map(|b| b.fingerprint.as_str());
    let create = matches!(computer.phase, Phase::Creating)
        .then(|| computer.creates.last())
        .flatten();
    let operation = create.map(|c| {
        entry
            .binding
            .as_ref()
            .map_or_else(|| c.operation.clone(), |b| b.operation(&c.operation))
    });
    // A pending resume can return a different resource. Associate it only
    // when the matching boot exists, not with the preceding boot's resource.
    let resource = computer
        .boots
        .iter()
        .find(|b| b.number as i32 == boot)
        .map(|b| b.resource.as_str())
        .or_else(|| create.and_then(|c| c.resource.as_deref()));
    let changed = tx.execute(
        "INSERT INTO computers.storage_reservations AS held
             (computer,owner,boot,create_ordinal,binding_fingerprint,operation,resource,maximum_bytes,maximum_checkpoints)
         VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9)
         ON CONFLICT(computer,boot,create_ordinal) DO UPDATE SET
             operation=COALESCE(held.operation,EXCLUDED.operation),
             resource=COALESCE(held.resource,EXCLUDED.resource)
         WHERE held.owner=EXCLUDED.owner AND held.state='held'
           AND held.binding_fingerprint IS NOT DISTINCT FROM EXCLUDED.binding_fingerprint
           AND (held.operation IS NULL OR EXCLUDED.operation IS NULL OR held.operation=EXCLUDED.operation)
           AND (held.resource IS NULL OR EXCLUDED.resource IS NULL OR held.resource=EXCLUDED.resource)",
        &[&entry.id,&entry.owner,&boot,&ordinal,&fingerprint,&operation,&resource,&bytes,&checkpoints],
    ).await?;
    if changed != 1 {
        return Err(DbError("The stored computer details changed.".into()));
    }
    Ok(())
}

pub(crate) async fn admit(tx: &Transaction<'_>, owner: &str) -> Result<(), DbError> {
    for (scope, subject) in [("account", owner), ("service", "")] {
        let policy = tx
            .query_opt(
                "SELECT private_storage_bytes,private_checkpoint_count FROM computers.usage_limits
             WHERE scope=$1 AND subject=$2",
                &[&scope, &subject],
            )
            .await?;
        let Some(policy) = policy else { continue };
        let bytes: Option<i64> = policy.get(0);
        let checkpoints: Option<i64> = policy.get(1);
        if bytes.is_none() && checkpoints.is_none() {
            continue;
        }
        // SUM(bigint) stays numeric through the comparison, so a large total
        // refuses admission without overflowing into a smaller allowance.
        let denied = tx.query_one(
            "SELECT
                 ($3::bigint IS NOT NULL AND
                   (count(*) FILTER(WHERE maximum_bytes IS NULL)>0 OR COALESCE(sum(maximum_bytes),0)>$3)),
                 ($4::bigint IS NOT NULL AND
                   (count(*) FILTER(WHERE maximum_checkpoints IS NULL)>0 OR COALESCE(sum(maximum_checkpoints),0)>$4))
             FROM computers.storage_reservations WHERE state='held' AND ($1='service' OR owner=$2)",
            &[&scope,&owner,&bytes,&checkpoints],
        ).await?;
        if denied.get::<_, bool>(0) || denied.get::<_, bool>(1) {
            return Err(DbError(QUOTA_ERROR.into()));
        }
    }
    Ok(())
}

pub(crate) async fn summary(
    client: &(impl GenericClient + Sync),
    owner: &str,
) -> Result<Value, DbError> {
    Ok(client.query_one(
        "SELECT jsonb_build_object(
          'held_bytes',CASE WHEN count(*) FILTER(WHERE state='held' AND maximum_bytes IS NULL)>0
             OR COALESCE(sum(maximum_bytes) FILTER(WHERE state='held'),0)>9223372036854775807 THEN NULL
             ELSE COALESCE(sum(maximum_bytes) FILTER(WHERE state='held'),0)::bigint END,
          'held_checkpoints',CASE WHEN count(*) FILTER(WHERE state='held' AND maximum_checkpoints IS NULL)>0
             OR COALESCE(sum(maximum_checkpoints) FILTER(WHERE state='held'),0)>9223372036854775807 THEN NULL
             ELSE COALESCE(sum(maximum_checkpoints) FILTER(WHERE state='held'),0)::bigint END,
          'held_allowances',count(*) FILTER(WHERE state='held'),
          'unknown_allowances',count(*) FILTER(WHERE state='held' AND (maximum_bytes IS NULL OR maximum_checkpoints IS NULL)),
          'released_allowances',count(*) FILTER(WHERE state='released'))
         FROM computers.storage_reservations WHERE owner=$1", &[&owner],
    ).await?.get(0))
}

/// An authenticated operator's complete inventory of one deleted computer.
/// Zero counts include recoverable versions, local copies, and in-flight work.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CleanupProof {
    pub schema: String,
    pub owner: String,
    pub computer: String,
    pub resources: Vec<String>,
    pub create_operations: Vec<String>,
    pub observed_ms: u64,
    pub live_resources: u64,
    pub retained_objects: u64,
    pub unfinished_uploads: u64,
    pub unresolved_operations: u64,
    pub inventory_receipt_sha256: String,
}

fn identifiers(values: &[String]) -> Result<BTreeSet<String>, DbError> {
    let set: BTreeSet<_> = values.iter().cloned().collect();
    if values.len() > 4096
        || set.len() != values.len()
        || values
            .iter()
            .any(|v| v.is_empty() || v.len() > 2048 || v.chars().any(char::is_control))
    {
        return Err(DbError(
            "The cleanup inventory has invalid identifiers.".into(),
        ));
    }
    Ok(set)
}

impl Store {
    /// Resolve held capacity after a complete operator inventory. A deleted VM,
    /// partial provider list, or expired timeout alone cannot release it.
    pub async fn resolve_storage_cleanup(
        &self,
        owner: &str,
        proof: &CleanupProof,
    ) -> Result<bool, DbError> {
        if proof.schema != "openagents.computers.storage-cleanup.v1"
            || proof.owner != owner
            || proof.live_resources != 0
            || proof.retained_objects != 0
            || proof.unfinished_uploads != 0
            || proof.unresolved_operations != 0
            || proof.inventory_receipt_sha256.len() != 64
            || !proof
                .inventory_receipt_sha256
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        {
            return Err(DbError(
                "Complete the private storage inventory before releasing capacity.".into(),
            ));
        }
        let resources = identifiers(&proof.resources)?;
        let operations = identifiers(&proof.create_operations)?;
        let mut normalized = proof.clone();
        normalized.resources = resources.iter().cloned().collect();
        normalized.create_operations = operations.iter().cloned().collect();
        let value = serde_json::to_value(&normalized)
            .map_err(|_| DbError("The cleanup inventory is invalid.".into()))?;
        let digest = format!(
            "{:x}",
            Sha256::digest(
                serde_json::to_vec(&value)
                    .map_err(|_| DbError("The cleanup inventory is invalid.".into()))?
            )
        );
        let mut client = self.db.independent().await?;
        let tx = client.transaction().await?;
        crate::usage::lock(&tx).await?;
        if let Some(saved) = tx
            .query_opt(
                "SELECT owner,proof_sha256 FROM computers.storage_resolutions WHERE computer=$1",
                &[&proof.computer],
            )
            .await?
        {
            if saved.get::<_, String>(0) != owner || saved.get::<_, String>(1) != digest {
                return Err(DbError(
                    "The cleanup inventory differs from the saved resolution.".into(),
                ));
            }
            tx.commit().await?;
            return Ok(false);
        }
        let row = tx
            .query_opt(
                &format!(
                    "SELECT {} FROM computers.computers WHERE id=$1 AND owner=$2 FOR UPDATE",
                    crate::store::COLUMNS
                ),
                &[&proof.computer, &owner],
            )
            .await?
            .ok_or_else(|| DbError("The computer could not be found.".into()))?;
        let entry = crate::store::entry(&row)?;
        if !matches!(entry.record.phase, Phase::Deleted)
            || !entry.record.deletion.as_ref().is_some_and(Fact::is_done)
            || entry.record.validate().is_err()
            || proof.observed_ms < entry.record.updated_ms
            || proof.observed_ms > crate::reconcile::now_ms().saturating_add(30_000)
        {
            return Err(DbError(
                "Delete the computer and collect a new storage inventory first.".into(),
            ));
        }
        let expected_resources: BTreeSet<_> = entry
            .record
            .boots
            .iter()
            .map(|b| b.resource.clone())
            .chain(
                entry
                    .record
                    .creates
                    .iter()
                    .filter_map(|c| c.resource.clone()),
            )
            .collect();
        let expected_operations: BTreeSet<_> = entry
            .record
            .creates
            .iter()
            .map(|c| {
                entry
                    .binding
                    .as_ref()
                    .map_or_else(|| c.operation.clone(), |b| b.operation(&c.operation))
            })
            .collect();
        if resources != expected_resources || operations != expected_operations {
            return Err(DbError(
                "The cleanup inventory belongs to different allocations.".into(),
            ));
        }
        tx.execute("INSERT INTO computers.storage_resolutions(computer,owner,proof_sha256,proof) VALUES($1,$2,$3,$4)",&[&proof.computer,&owner,&digest,&value]).await?;
        tx.execute("UPDATE computers.storage_reservations SET state='released',released_at=clock_timestamp() WHERE computer=$1 AND owner=$2 AND state='held'",&[&proof.computer,&owner]).await?;
        crate::usage::record(&tx,&entry,"resolution","storage-cleanup",crate::reconcile::now_ms() as i64,
            json!({"source":"operator_storage_inventory","proof_sha256":digest,
                "inventory_receipt_sha256":proof.inventory_receipt_sha256,"provider_seconds":null,"customer_charge":null})).await?;
        tx.commit().await?;
        Ok(true)
    }
}
