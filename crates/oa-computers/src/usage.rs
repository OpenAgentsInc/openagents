//! Durable usage evidence and operator allocation allowances.
//!
//! A boot allowance reserves a declared amount before a provider call. It is separate
//! from measured provider seconds and purchased compute: this module never
//! quotes a price, debits a customer, or estimates missing meter readings.
//! Provider outages or overruns can exceed the declared runtime allowance.

use crate::{Store, db::DbError, store::Entry};
use coder_working_computer::{Computer, Phase};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::time::Duration;
use tokio_postgres::Transaction;

pub const QUOTA_ERROR: &str = "The computer limit has been reached.";

/// Limits set by an operator, never accepted from an allocation request.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Limits {
    pub computer_count: Option<i64>,
    pub running_vcpus: Option<i64>,
    pub running_memory_mib: Option<i64>,
    /// Sum of unresolved maximums and settled usage, in CPU milliseconds.
    /// This is an operator allowance, not a customer price or charge.
    pub boot_cpu_ms: Option<i64>,
    pub turn_seconds: Option<i32>,
    /// Time to keep an observed stopped computer, capped by the service setting.
    pub stopped_retention_seconds: Option<i64>,
}

/// All accounting mutations use the same lock, including service-wide limits.
/// The restricted rollout favors a simple conservation boundary over throughput.
pub(crate) async fn lock(tx: &Transaction<'_>) -> Result<(), DbError> {
    tx.batch_execute("SET LOCAL lock_timeout='5s'; SET LOCAL statement_timeout='10s'; SET LOCAL idle_in_transaction_session_timeout='15s'").await?;
    tx.query_one("SELECT pg_advisory_xact_lock(11815)", &[])
        .await?;
    Ok(())
}

/// Check current reservations, including unknown shapes and provider outcomes.
/// Accepted stop/delete requests do not release running or resource capacity.
pub(crate) async fn admit(tx: &Transaction<'_>, owner: &str) -> Result<(), DbError> {
    for (scope, subject) in [("account", owner), ("service", "")] {
        let policy = tx
            .query_opt(
                "SELECT computer_count,running_vcpus,running_memory_mib,boot_cpu_ms
             FROM computers.usage_limits WHERE scope=$1 AND subject=$2",
                &[&scope, &subject],
            )
            .await?;
        let default_count = (scope == "account").then_some(crate::store::MAX_ACCOUNT_COMPUTERS);
        let limits: [Option<i64>; 4] = match policy {
            Some(row) => [
                row.get::<_, Option<i64>>(0).or(default_count),
                row.get(1),
                row.get(2),
                row.get(3),
            ],
            None => [default_count, None, None, None],
        };
        let row = tx.query_one(
            "SELECT count(*),
                COALESCE(sum((binding->'shape'->>'vcpus')::bigint) FILTER(WHERE desired='running' OR phase NOT IN ('stopped','deleted')),0)::bigint,
                COALESCE(sum((binding->'shape'->>'memory_mib')::bigint) FILTER(WHERE desired='running' OR phase NOT IN ('stopped','deleted')),0)::bigint,
                count(*) FILTER(WHERE (desired='running' OR phase NOT IN ('stopped','deleted')) AND
                    (binding->'shape'->>'vcpus' IS NULL OR binding->'shape'->>'memory_mib' IS NULL))
             FROM computers.computers WHERE NOT done AND ($1='service' OR owner=$2)", &[&scope,&owner],
        ).await?;
        for (index, limit) in limits[..3].iter().enumerate() {
            if limit.is_some_and(|limit| row.get::<_, i64>(index) > limit)
                || index > 0 && limit.is_some() && row.get::<_, i64>(3) > 0
            {
                return Err(DbError(QUOTA_ERROR.into()));
            }
        }
        if let Some(limit) = limits[3] {
            let row = tx.query_one(
                "SELECT COALESCE(sum(CASE WHEN state='settled' THEN used_cpu_ms ELSE maximum_cpu_ms END),0)::bigint,
                        count(*) FILTER(WHERE maximum_cpu_ms IS NULL)
                 FROM computers.usage_reservations WHERE $1='service' OR owner=$2", &[&scope,&owner],
            ).await?;
            if row.get::<_, i64>(0) > limit || row.get::<_, i64>(1) > 0 {
                return Err(DbError(QUOTA_ERROR.into()));
            }
        }
    }
    Ok(())
}

/// Reserve once for this boot before create or resume can reach the provider.
pub(crate) async fn reserve(tx: &Transaction<'_>, e: &Entry, c: &Computer) -> Result<(), DbError> {
    let boot = if matches!(c.phase, Phase::New | Phase::Creating | Phase::Stopped) {
        c.boots.len() + 1
    } else {
        c.boots.last().map_or(1, |boot| boot.number as usize)
    };
    let maximum = e
        .binding
        .as_ref()
        .and_then(|b| b.shape.as_ref())
        .and_then(|shape| c.bounds.absolute_ms.checked_mul(u64::from(shape.vcpus)))
        .and_then(|n| i64::try_from(n).ok());
    tx.execute(
        "INSERT INTO computers.usage_reservations(computer,owner,boot,maximum_cpu_ms)
         VALUES($1,$2,$3,$4) ON CONFLICT DO NOTHING",
        &[&e.id, &e.owner, &(boot as i32), &maximum],
    )
    .await?;
    admit(tx, &e.owner).await
}

pub(crate) async fn record(
    tx: &Transaction<'_>,
    e: &Entry,
    kind: &str,
    key: &str,
    observed_ms: i64,
    mut evidence: Value,
) -> Result<(), DbError> {
    let deployment = e.binding.as_ref().map(|b| b.deployment.as_str());
    let shape = e
        .binding
        .as_ref()
        .and_then(|b| b.shape.as_ref())
        .map(serde_json::to_value)
        .transpose()
        .map_err(|e| DbError(e.to_string()))?;
    let boot = e.record.boots.last().map(|b| b.number as i32);
    let operation = e.record.creates.last().map(|a| a.operation.as_str());
    evidence["lifecycle_ms"] = json!(e.record.updated_ms);
    evidence["deployment_fingerprint"] = json!(e.binding.as_ref().map(|b| &b.fingerprint));
    evidence["provider_operation"] =
        json!(operation.map(|operation| e.binding.as_ref().map_or_else(
            || operation.to_owned(),
            |binding| binding.operation(operation)
        )));
    evidence["storage"] = json!({"resource_deleted":matches!(e.record.phase,Phase::Deleted),
        "bytes":null,"checkpoint_records":e.record.checkpoints.len()});
    evidence["service_costs"] = json!({"shared_base_snapshots":null,"ready_pools":null,
        "worker_idle":null,"nat":null,"transfer":null,"storage_operations":null});
    tx.execute(
        "INSERT INTO computers.usage_events(computer,owner,revision,intent_generation,kind,event_key,
            deployment,driver,size,shape,resource,boot,phase,desired,observed_ms,evidence,operation,turn_generation)
         VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14,$15,$16,$17,$18)
         ON CONFLICT(computer,event_key) DO NOTHING",
        &[&e.id,&e.owner,&(e.record.revision as i64),&e.intent_generation,&kind,&key,&deployment,
          &e.driver,&e.size,&shape,&e.record.resource(),&boot,&e.phase,&e.desired.as_str(),&observed_ms,&evidence,
          &operation,&(e.record.turn.dispatched as i64)],
    ).await?;
    if matches!(
        e.record.phase,
        Phase::Stopped | Phase::Deleted | Phase::Unknown { .. } | Phase::Failed { .. }
    ) {
        tx.execute("UPDATE computers.usage_reservations SET state='unknown' WHERE computer=$1 AND state='held'", &[&e.id]).await?;
    }
    Ok(())
}

pub(crate) fn lifecycle_evidence() -> Value {
    json!({"source":"lifecycle","provider_seconds":null,"customer_charge":null})
}

pub(crate) async fn check_turn(
    tx: &Transaction<'_>,
    owner: &str,
    seconds: i32,
) -> Result<(), DbError> {
    let limit: i32 = tx
        .query_one(
            "SELECT LEAST(3600,COALESCE(min(turn_seconds),3600)) FROM computers.usage_limits
         WHERE (scope='account' AND subject=$1) OR scope='service'",
            &[&owner],
        )
        .await?
        .get(0);
    if seconds < 1 || seconds > limit {
        return Err(DbError(QUOTA_ERROR.into()));
    }
    Ok(())
}

pub(crate) async fn record_intent(
    tx: &Transaction<'_>,
    owner: &str,
    id: &str,
) -> Result<(), DbError> {
    let e = crate::store::entry(
        &tx.query_one(
            &format!(
                "SELECT {} FROM computers.computers WHERE id=$1 AND owner=$2",
                crate::store::COLUMNS
            ),
            &[&id, &owner],
        )
        .await?,
    )?;
    if e.desired == crate::store::Desired::Running {
        reserve(tx, &e, &e.record).await?;
    }
    record(
        tx,
        &e,
        "accepted",
        &format!("intent:{}", e.intent_generation),
        crate::reconcile::now_ms() as i64,
        lifecycle_evidence(),
    )
    .await
}

pub(crate) fn meter_evidence(outcome: &computer_driver::Outcome<computer_driver::Meter>) -> Value {
    use computer_driver::Outcome;
    match outcome {
        Outcome::Done { value } => json!({"source":"provider_meter","status":"done",
            "running":value.running,"provider_seconds":value.seconds,
            "measurement":if value.seconds.is_some() {"reported"} else {"unknown"},
            "evidence_sha256":format!("{:x}",Sha256::digest(value.evidence.as_bytes())),
            "evidence_bytes":value.evidence.len(),"counter_scope":"provider_defined","customer_charge":null}),
        Outcome::Failed { .. } | Outcome::Unknown { .. } => json!({"source":"provider_meter",
            "status":if matches!(outcome,Outcome::Failed { .. }) {"failed"} else {"unknown"},
            "provider_seconds":null,"measurement":"unknown","customer_charge":null}),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use computer_driver::{Meter, Outcome};

    #[test]
    fn absent_seconds_and_provider_text_never_become_a_zero_measurement_or_usage_text() {
        let mut meter = Meter {
            running: false,
            evidence: "private provider text".into(),
            seconds: None,
        };
        let absent = meter_evidence(&Outcome::done(meter.clone()));
        assert!(absent["provider_seconds"].is_null());
        assert_eq!(absent["measurement"], "unknown");
        assert!(!absent.to_string().contains("private provider text"));
        assert_eq!(absent["evidence_sha256"].as_str().unwrap().len(), 64);
        meter.seconds = Some(0);
        let zero = meter_evidence(&Outcome::done(meter));
        assert_eq!(zero["provider_seconds"], 0);
        assert_eq!(zero["measurement"], "reported");
        for outcome in [
            Outcome::unknown("private error"),
            Outcome::failed("private error"),
        ] {
            let unknown = meter_evidence(&outcome);
            assert!(unknown["provider_seconds"].is_null());
            assert!(!unknown.to_string().contains("private error"));
        }
    }
}

impl Store {
    /// Expire a stopped computer under the current account and service policies.
    /// A wake or policy change shares this transaction's accounting lock.
    pub(crate) async fn expire_stopped(
        &self,
        owner: &str,
        id: &str,
        intent_generation: i64,
        now_ms: u64,
        configured_retention: Duration,
    ) -> Result<bool, DbError> {
        let mut c = self.db.independent().await?;
        let tx = c.transaction().await?;
        lock(&tx).await?;
        let Some(row) = tx
            .query_opt(
                &format!(
                    "SELECT {} FROM computers.computers WHERE id=$1 AND owner=$2 FOR UPDATE",
                    crate::store::COLUMNS
                ),
                &[&id, &owner],
            )
            .await?
        else {
            return Ok(false);
        };
        let e = crate::store::entry(&row)?;
        if e.desired != crate::store::Desired::Stopped
            || e.record.phase != Phase::Stopped
            || e.intent_generation != intent_generation
        {
            return Ok(false);
        }
        let seconds: Option<i64> = tx
            .query_one(
                "SELECT min(stopped_retention_seconds) FROM computers.usage_limits
             WHERE (scope='account' AND subject=$1) OR scope='service'",
                &[&owner],
            )
            .await?
            .get(0);
        let retention = seconds
            .map(|seconds| u64::try_from(seconds).map(Duration::from_secs))
            .transpose()
            .map_err(|_| DbError("The computer retention limit is invalid.".into()))?
            .map_or(configured_retention, |policy| {
                policy.min(configured_retention)
            });
        if Duration::from_millis(now_ms.saturating_sub(e.record.updated_ms)) < retention {
            return Ok(false);
        }
        tx.execute(
            "UPDATE computers.computers SET desired='deleted',
             intent_generation=intent_generation+1,usage_intent_generation=intent_generation+1,updated_at=now()
             WHERE id=$1 AND owner=$2", &[&id, &owner],
        ).await?;
        record_intent(&tx, owner, id).await?;
        tx.commit().await?;
        Ok(true)
    }

    /// Account-scoped, bounded evidence reads. No command, output, or secret text.
    pub async fn usage(&self, owner: &str, after: i64, limit: i64) -> Result<Value, DbError> {
        let rows = self.db.client().await?.query(
            "SELECT to_jsonb(e) FROM computers.usage_events e WHERE owner=$1 AND sequence>$2 ORDER BY sequence LIMIT $3",
            &[&owner,&after.max(0),&limit.clamp(1,200)],
        ).await?;
        let events: Vec<Value> = rows.iter().map(|r| r.get(0)).collect();
        let cursor = events
            .last()
            .and_then(|v| v["sequence"].as_i64())
            .unwrap_or(after.max(0));
        let reservations = self.db.client().await?.query_one(
            "SELECT jsonb_build_object('held_cpu_ms',CASE WHEN count(*) FILTER(WHERE state<>'settled' AND maximum_cpu_ms IS NULL)>0
                THEN NULL ELSE COALESCE(sum(maximum_cpu_ms) FILTER(WHERE state<>'settled'),0) END,
                'settled_cpu_ms',COALESCE(sum(used_cpu_ms),0),
                'unknown_allowances',count(*) FILTER(WHERE maximum_cpu_ms IS NULL),
                'unresolved_allowances',count(*) FILTER(WHERE state<>'settled'))
             FROM computers.usage_reservations WHERE owner=$1", &[&owner],
        ).await?.get::<_,Value>(0);
        Ok(
            json!({"schema":"openagents.computers.usage.v1","events":events,"after":cursor,
            "allocation_allowance":reservations,"customer_charge":null,"pricing":"not_enabled"}),
        )
    }

    /// Configure an operator policy while preserving all committed allowances.
    /// `None` names the service; an account policy cannot alter another account.
    pub async fn set_usage_limits(
        &self,
        owner: Option<&str>,
        limits: &Limits,
    ) -> Result<(), DbError> {
        if [
            limits.computer_count,
            limits.running_vcpus,
            limits.running_memory_mib,
            limits.boot_cpu_ms,
            limits.stopped_retention_seconds,
        ]
        .iter()
        .flatten()
        .any(|n| *n < 0)
            || limits
                .turn_seconds
                .is_some_and(|n| !(1..=3600).contains(&n))
        {
            return Err(DbError("The computer limits are invalid.".into()));
        }
        let mut c = self.db.independent().await?;
        let tx = c.transaction().await?;
        lock(&tx).await?;
        let scope = if owner.is_some() {
            "account"
        } else {
            "service"
        };
        tx.execute(
            "INSERT INTO computers.usage_limits(scope,subject,computer_count,running_vcpus,running_memory_mib,boot_cpu_ms,turn_seconds,stopped_retention_seconds)
             VALUES($1,$2,$3,$4,$5,$6,$7,$8) ON CONFLICT(scope,subject) DO UPDATE SET
             computer_count=$3,running_vcpus=$4,running_memory_mib=$5,boot_cpu_ms=$6,turn_seconds=$7,stopped_retention_seconds=$8",
            &[&scope,&owner.unwrap_or(""),&limits.computer_count,&limits.running_vcpus,&limits.running_memory_mib,
              &limits.boot_cpu_ms,&limits.turn_seconds.unwrap_or(3600),&limits.stopped_retention_seconds],
        ).await?;
        admit(&tx, owner.unwrap_or("")).await?;
        let unfinished: bool = tx.query_one(
            "SELECT EXISTS(SELECT 1 FROM computers.turns WHERE state IN ('queued','running','command_queued','command_running')
             AND ($1='service' OR owner=$2) AND timeout_seconds>$3)",
            &[&scope,&owner.unwrap_or(""),&limits.turn_seconds.unwrap_or(3600)],
        ).await?.get(0);
        if unfinished {
            return Err(DbError(QUOTA_ERROR.into()));
        }
        tx.commit().await?;
        Ok(())
    }

    /// Resolve an allowance by an explicit operator decision. Replays must match.
    /// This does not settle or debit the purchased compute ledger.
    pub async fn resolve_usage_allowance(
        &self,
        owner: &str,
        computer: &str,
        boot: i32,
        maximum: i64,
        used: i64,
        reason: &str,
    ) -> Result<(), DbError> {
        if maximum < 0 || used < 0 || used > maximum || reason.is_empty() || reason.len() > 1024 {
            return Err(DbError("The usage decision is invalid.".into()));
        }
        let mut c = self.db.independent().await?;
        let tx = c.transaction().await?;
        lock(&tx).await?;
        let row = tx.query_opt(
            "SELECT maximum_cpu_ms,state,used_cpu_ms,resolution FROM computers.usage_reservations
             WHERE computer=$1 AND owner=$2 AND boot=$3 FOR UPDATE", &[&computer,&owner,&boot],
        ).await?.ok_or_else(|| DbError("No such usage allowance.".into()))?;
        let old_max: Option<i64> = row.get(0);
        let old_state: &str = row.get(1);
        if old_max.is_some_and(|n| n != maximum)
            || old_state == "settled"
                && (row.get::<_, Option<i64>>(2) != Some(used)
                    || row.get::<_, Option<String>>(3).as_deref() != Some(reason))
        {
            return Err(DbError(
                "That usage decision already has different terms.".into(),
            ));
        }
        // A running boot cannot be made available again by an early settlement.
        let e = tx
            .query_one(
                "SELECT phase,record FROM computers.computers WHERE id=$1 AND owner=$2",
                &[&computer, &owner],
            )
            .await?;
        let computer_record: Computer =
            serde_json::from_value(e.get(1)).map_err(|e| DbError(e.to_string()))?;
        if old_state != "settled"
            && !matches!(computer_record.phase, Phase::Deleted)
            && (computer_record
                .boots
                .last()
                .is_none_or(|b| (b.number as i32) < boot)
                || computer_record
                    .boots
                    .last()
                    .is_some_and(|b| b.number as i32 == boot)
                    && !matches!(computer_record.phase, Phase::Stopped))
        {
            return Err(DbError(
                "Stop the computer before resolving its usage.".into(),
            ));
        }
        tx.execute(
            "UPDATE computers.usage_reservations SET maximum_cpu_ms=$4,state='settled',used_cpu_ms=$5,resolution=$6,settled_at=now()
             WHERE computer=$1 AND owner=$2 AND boot=$3 AND state<>'settled'", &[&computer,&owner,&boot,&maximum,&used,&reason],
        ).await?;
        admit(&tx, owner).await?;
        let e = crate::store::entry(
            &tx.query_one(
                &format!(
                    "SELECT {} FROM computers.computers WHERE id=$1 AND owner=$2",
                    crate::store::COLUMNS
                ),
                &[&computer, &owner],
            )
            .await?,
        )?;
        record(
            &tx,
            &e,
            "resolution",
            &format!("resolution:{boot}"),
            crate::reconcile::now_ms() as i64,
            json!({"source":"operator_policy","boot":boot,"maximum_cpu_ms":maximum,
                "used_cpu_ms":used,"reason":reason,"customer_charge":null}),
        )
        .await?;
        tx.commit().await?;
        Ok(())
    }
}
