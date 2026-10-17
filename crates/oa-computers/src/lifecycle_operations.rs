//! PostgreSQL ownership of accepted SPACE operations across API restarts.
use crate::Db;
use computer_driver::space::{JournalFuture, LifecycleClaim, LifecycleJournal};

#[derive(Clone)]
pub struct Journal(pub Db);

const UNAVAILABLE: &str = "The saved lifecycle request couldn't be read.";

impl LifecycleJournal for Journal {
    fn claim<'a>(
        &'a self,
        key: &'a str,
        resource: &'a str,
        action: &'a str,
    ) -> JournalFuture<'a, LifecycleClaim> {
        Box::pin(async move {
            let client = self.0.client().await.map_err(|_| UNAVAILABLE)?;
            let inserted = client
                .execute(
                    "INSERT INTO computers.lifecycle_operations (operation,resource,action)
                 VALUES ($1,$2,$3) ON CONFLICT (operation) DO NOTHING",
                    &[&key, &resource, &action],
                )
                .await
                .map_err(|_| UNAVAILABLE)?;
            if inserted == 1 {
                return Ok(LifecycleClaim::Fresh);
            }
            let row = client.query_one(
                "SELECT resource,action,disposition,provider_operation FROM computers.lifecycle_operations WHERE operation=$1",
                &[&key],
            ).await.map_err(|_| UNAVAILABLE)?;
            if row.get::<_, &str>(0) != resource || row.get::<_, &str>(1) != action {
                return Err("The saved lifecycle request has different settings.".into());
            }
            Ok(match row.get::<_, &str>(2) {
                "accepted" => LifecycleClaim::Accepted(row.get(3)),
                "rejected" => LifecycleClaim::Rejected,
                _ => LifecycleClaim::Uncertain,
            })
        })
    }

    fn accepted<'a>(&'a self, key: &'a str, provider_operation: &'a str) -> JournalFuture<'a, ()> {
        Box::pin(async move {
            let client = self.0.client().await.map_err(|_| UNAVAILABLE)?;
            let changed = client.execute(
                "UPDATE computers.lifecycle_operations SET disposition='accepted',provider_operation=$2
                 WHERE operation=$1 AND (disposition='requested' OR (disposition='accepted' AND provider_operation=$2))",
                &[&key, &provider_operation],
            ).await.map_err(|_| UNAVAILABLE)?;
            if changed != 1 {
                return Err(UNAVAILABLE.into());
            }
            Ok(())
        })
    }

    fn rejected<'a>(&'a self, key: &'a str) -> JournalFuture<'a, ()> {
        Box::pin(async move {
            let client = self.0.client().await.map_err(|_| UNAVAILABLE)?;
            let changed = client
                .execute(
                    "UPDATE computers.lifecycle_operations SET disposition='rejected'
                 WHERE operation=$1 AND disposition IN ('requested','rejected')",
                    &[&key],
                )
                .await
                .map_err(|_| UNAVAILABLE)?;
            if changed != 1 {
                return Err(UNAVAILABLE.into());
            }
            Ok(())
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn requests_survive_restart_without_granting_a_second_mutation() {
        let Ok(admin) = std::env::var("OA_COMPUTERS_TEST_DATABASE_URL") else {
            return;
        };
        let (db, _) = Db::scratch(&admin).await.unwrap();
        let journal = Journal(db.clone());
        assert_eq!(
            journal.claim("one", "sbx_one", "suspend").await.unwrap(),
            LifecycleClaim::Fresh
        );
        let restarted = Journal(db.clone());
        assert_eq!(
            restarted.claim("one", "sbx_one", "suspend").await.unwrap(),
            LifecycleClaim::Uncertain
        );
        restarted.accepted("one", "op_saved").await.unwrap();
        assert_eq!(
            journal.claim("one", "sbx_one", "suspend").await.unwrap(),
            LifecycleClaim::Accepted("op_saved".into())
        );
        assert!(journal.claim("one", "sbx_other", "suspend").await.is_err());
        assert!(journal.claim("one", "sbx_one", "wake").await.is_err());
        assert!(journal.accepted("one", "op_changed").await.is_err());
        assert_eq!(
            journal.claim("two", "sbx_one", "wake").await.unwrap(),
            LifecycleClaim::Fresh
        );
        journal.rejected("two").await.unwrap();
        assert_eq!(
            restarted.claim("two", "sbx_one", "wake").await.unwrap(),
            LifecycleClaim::Rejected
        );
    }
}
