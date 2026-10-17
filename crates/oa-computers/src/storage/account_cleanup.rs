//! Durable account closure for computer resources and private command contents.

use crate::{Store, db::DbError, store::Desired};
use serde_json::{Value, json};

impl Store {
    /// Fence new work and schedule deletion. Repeated requests preserve the job.
    /// Storage holds remain until a complete inventory resolves each computer.
    pub async fn request_account_cleanup(&self, owner: &str) -> Result<bool, DbError> {
        if owner.is_empty() || owner.len() > 256 || owner.chars().any(char::is_control) {
            return Err(DbError("The account identifier is invalid.".into()));
        }
        let mut client = self.db.independent().await?;
        let tx = client.transaction().await?;
        crate::usage::lock(&tx).await?;
        let changed = tx
            .execute(
                "INSERT INTO computers.account_cleanup(owner) VALUES($1) ON CONFLICT DO NOTHING",
                &[&owner],
            )
            .await?;
        tx.commit().await?;
        Ok(changed == 1)
    }

    pub async fn account_cleanup_status(&self, owner: &str) -> Result<Value, DbError> {
        let client = self.db.client().await?;
        let row = client.query_one(
            "SELECT EXISTS(SELECT 1 FROM computers.account_cleanup WHERE owner=$1),
             EXISTS(SELECT 1 FROM computers.account_cleanup WHERE owner=$1 AND completed_at IS NOT NULL),
             (SELECT count(*) FROM computers.computers WHERE owner=$1 AND (NOT done OR phase<>'deleted')),
             (SELECT count(*) FROM computers.storage_reservations WHERE owner=$1 AND state='held')",
            &[&owner],
        ).await?;
        Ok(
            json!({"requested":row.get::<_,bool>(0),"complete":row.get::<_,bool>(1),
            "pending_computers":row.get::<_,i64>(2),"held_storage_allowances":row.get::<_,i64>(3)}),
        )
    }

    /// Make bounded progress after restart. Reconciliation owns provider calls;
    /// an unknown provider outcome cannot complete this account's cleanup.
    pub async fn advance_account_cleanup(&self, limit: i64) -> Result<u64, DbError> {
        let mut client = self.db.independent().await?;
        let tx = client.transaction().await?;
        crate::usage::lock(&tx).await?;
        let rows = tx
            .query(
                "SELECT owner FROM computers.account_cleanup WHERE completed_at IS NULL
             AND next_attempt_at<=clock_timestamp() ORDER BY next_attempt_at,owner
             LIMIT $1 FOR UPDATE SKIP LOCKED",
                &[&limit.clamp(1, 8)],
            )
            .await?;
        for row in &rows {
            let owner: String = row.get(0);
            let changed = tx.query(
                &format!("UPDATE computers.computers SET desired='deleted',
                    intent_generation=intent_generation+1,usage_intent_generation=intent_generation+1,
                    next_attempt_at=clock_timestamp(),attempts=0,updated_at=clock_timestamp()
                 WHERE id IN (SELECT id FROM computers.computers WHERE owner=$1 AND desired<>'deleted'
                    ORDER BY id LIMIT 100) RETURNING {}", crate::store::COLUMNS), &[&owner],
            ).await?;
            tx.execute(
                "UPDATE computers.commands SET cancel_requested=true,
                   status=CASE WHEN status='queued' THEN 'cancelled' ELSE status END,
                   finished_at=CASE WHEN status='queued' THEN clock_timestamp() ELSE finished_at END
                 WHERE turn IN (SELECT turn FROM computers.commands WHERE owner=$1
                    AND finished_at IS NULL AND NOT cancel_requested ORDER BY turn LIMIT 500)",
                &[&owner],
            )
            .await?;
            tx.execute(
                "UPDATE computers.turns SET state='failed',finished_at=clock_timestamp()
                 WHERE id IN (SELECT id FROM computers.turns WHERE owner=$1 AND (state='queued' OR id IN
                    (SELECT turn FROM computers.commands WHERE owner=$1 AND status='cancelled'))
                    AND finished_at IS NULL ORDER BY id LIMIT 500)",
                &[&owner],
            )
            .await?;
            for row in &changed {
                let e = crate::store::entry(row)?;
                debug_assert_eq!(e.desired, Desired::Deleted);
                crate::usage::record(&tx,&e,"accepted",&format!("intent:{}",e.intent_generation),
                    crate::reconcile::now_ms() as i64,
                    json!({"source":"account_cleanup","provider_seconds":null,"customer_charge":null})).await?;
            }
            let mut complete: bool = tx.query_one(
                "SELECT NOT EXISTS(SELECT 1 FROM computers.computers WHERE owner=$1 AND (NOT done OR phase<>'deleted'))
                 AND NOT EXISTS(SELECT 1 FROM computers.storage_reservations WHERE owner=$1 AND state='held')
                 AND NOT EXISTS(SELECT 1 FROM computers.commands WHERE owner=$1 AND finished_at IS NULL)
                 AND NOT EXISTS(SELECT 1 FROM computers.turns WHERE owner=$1 AND finished_at IS NULL)", &[&owner],
            ).await?.get(0);
            if complete {
                // Keep only operational identities and accounting evidence.
                tx.execute(
                    "UPDATE computers.commands SET options='{}',stdout='',stderr=''
                    WHERE turn IN (SELECT turn FROM computers.commands WHERE owner=$1
                       AND (options<>'{}' OR octet_length(stdout)>0 OR octet_length(stderr)>0)
                       ORDER BY turn LIMIT 500)",
                    &[&owner],
                )
                .await?;
                tx.execute("UPDATE computers.turns SET command='',cwd=NULL,env_names='{}',stdout=NULL,stderr=NULL,error=NULL
                    WHERE id IN (SELECT id FROM computers.turns WHERE owner=$1 AND
                       (command<>'' OR cwd IS NOT NULL OR cardinality(env_names)>0 OR stdout IS NOT NULL
                        OR stderr IS NOT NULL OR error IS NOT NULL) ORDER BY id LIMIT 500)", &[&owner]).await?;
                complete=tx.query_one("SELECT NOT EXISTS(SELECT 1 FROM computers.commands WHERE owner=$1
                       AND (options<>'{}' OR octet_length(stdout)>0 OR octet_length(stderr)>0))
                    AND NOT EXISTS(SELECT 1 FROM computers.turns WHERE owner=$1 AND
                       (command<>'' OR cwd IS NOT NULL OR cardinality(env_names)>0 OR stdout IS NOT NULL
                        OR stderr IS NOT NULL OR error IS NOT NULL))", &[&owner]).await?.get(0);
            }
            tx.execute(
                "UPDATE computers.account_cleanup SET next_attempt_at=clock_timestamp()+interval '30 seconds',
                 completed_at=CASE WHEN $2 THEN clock_timestamp() ELSE NULL END WHERE owner=$1",
                &[&owner,&complete],
            ).await?;
        }
        tx.commit().await?;
        Ok(rows.len() as u64)
    }
}
