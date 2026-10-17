//! Explicit recovery never resets accepted or uncertain lifecycle receipts.
use super::*;
use computer_driver::space::{LifecycleClaim, LifecycleJournal};
use oa_computers::{
    lifecycle_operations::{Journal, RetryRequest},
    store::Entry,
};

async fn database() -> Option<(Db, String)> {
    let admin = std::env::var("OA_COMPUTERS_TEST_DATABASE_URL").ok()?;
    Some(Db::scratch(&admin).await.unwrap())
}
fn request(entry: &Entry, operation: &str, action: &str) -> RetryRequest {
    RetryRequest {
        computer: entry.id.clone(),
        operation: operation.into(),
        resource: entry.record.resource().unwrap().into(),
        action: action.into(),
        reason: "The framing defect is fixed; the saved request was definitively refused.".into(),
    }
}

#[tokio::test]
async fn lifecycle_retry_is_account_bound_durable_and_duplicate_safe() {
    let Some((db, dsn)) = database().await else {
        return;
    };
    tokio::task::LocalSet::new().run_until(async move {
        let store=Store::new(db.clone());let root=tempfile::tempdir().unwrap();
        let a=Simulated::new(&root.path().join("a"),false);let b=Simulated::new(&root.path().join("b"),true);
        let driver=configured("b",&a,&b);
        let (entry,_)=store.create_placed("alice",&CreateIntent::default(),driver.catalog()).await.unwrap();
        let entry=phase(&db,&store,&driver,&entry.id,"awake").await;
        let journal=Journal(db.clone());let resource=entry.record.resource().unwrap();
        assert_eq!(journal.claim("refused",resource,"suspend").await.unwrap(),LifecycleClaim::Fresh);
        assert!(store.retry_rejected_lifecycle("alice",&request(&entry,"refused","suspend")).await.is_err());
        journal.rejected("refused").await.unwrap();
        let retry=request(&entry,"refused","suspend");
        for wrong in ["mallory",""] { assert!(store.retry_rejected_lifecycle(wrong,&retry).await.is_err()); }
        let mut altered=retry.clone();altered.action="wake".into();
        assert!(store.retry_rejected_lifecycle("alice",&altered).await.is_err());
        altered=retry.clone();altered.resource="sbx_other".into();
        assert!(store.retry_rejected_lifecycle("alice",&altered).await.is_err());
        let mut tasks=Vec::new();for _ in 0..12 {
            let store=store.clone();let retry=retry.clone();
            tasks.push(tokio::spawn(async move{store.retry_rejected_lifecycle("alice",&retry).await.unwrap()}));
        }
        let mut children=std::collections::BTreeSet::new();for task in tasks{children.insert(task.await.unwrap());}
        assert_eq!(children.len(),1);let child=children.first().unwrap();
        let restarted=Store::new(Db::connect(&dsn).await.unwrap());
        assert_eq!(&restarted.retry_rejected_lifecycle("alice",&retry).await.unwrap(),child);
        assert_eq!(journal.claim("refused",resource,"suspend").await.unwrap(),LifecycleClaim::Rejected);
        assert_eq!(journal.authorized_retry("refused",resource,"suspend").await.unwrap(),Some(child.clone()));
        assert!(journal.authorized_retry("refused",resource,"wake").await.unwrap().is_none());
        assert_eq!(journal.claim(child,resource,"suspend").await.unwrap(),LifecycleClaim::Fresh);
        assert_eq!(journal.claim(child,resource,"suspend").await.unwrap(),LifecycleClaim::Uncertain);
        assert!(restarted.retry_rejected_lifecycle("alice",&request(&entry,child,"suspend")).await.is_err());
        journal.accepted(child,"op_saved").await.unwrap();
        assert!(restarted.retry_rejected_lifecycle("alice",&request(&entry,child,"suspend")).await.is_err());
        assert_eq!(journal.claim(child,resource,"suspend").await.unwrap(),LifecycleClaim::Accepted("op_saved".into()));
        altered=retry.clone();altered.reason="A different reason".into();
        assert!(restarted.retry_rejected_lifecycle("alice",&altered).await.is_err());
        let client=db.client().await.unwrap();
        assert_eq!(client.query_one("SELECT disposition FROM computers.lifecycle_operations WHERE operation='refused'",&[]).await.unwrap().get::<_,String>(0),"rejected");
        assert_eq!(client.query_one("SELECT count(*) FROM computers.lifecycle_retries",&[]).await.unwrap().get::<_,i64>(0),1);
        assert!(client.execute("DELETE FROM computers.lifecycle_retries",&[]).await.is_err());
    }).await;
}

#[tokio::test]
async fn lifecycle_retry_budget_is_finite_and_closed_accounts_cannot_wake() {
    let Some((db, _)) = database().await else {
        return;
    };
    tokio::task::LocalSet::new()
        .run_until(async move {
            let store = Store::new(db.clone());
            let root = tempfile::tempdir().unwrap();
            let a = Simulated::new(&root.path().join("a"), false);
            let b = Simulated::new(&root.path().join("b"), true);
            let driver = configured("b", &a, &b);
            let (entry, _) = store
                .create_placed("alice", &CreateIntent::default(), driver.catalog())
                .await
                .unwrap();
            let entry = phase(&db, &store, &driver, &entry.id, "awake").await;
            let journal = Journal(db.clone());
            let resource = entry.record.resource().unwrap();
            let mut operation = "root-budget".to_owned();
            journal
                .claim(&operation, resource, "suspend")
                .await
                .unwrap();
            journal.rejected(&operation).await.unwrap();
            for _ in 0..8 {
                operation = store
                    .retry_rejected_lifecycle("alice", &request(&entry, &operation, "suspend"))
                    .await
                    .unwrap();
                assert_eq!(
                    journal
                        .claim(&operation, resource, "suspend")
                        .await
                        .unwrap(),
                    LifecycleClaim::Fresh
                );
                journal.rejected(&operation).await.unwrap();
            }
            assert!(
                store
                    .retry_rejected_lifecycle("alice", &request(&entry, &operation, "suspend"))
                    .await
                    .is_err()
            );
            journal
                .claim("closed-wake", resource, "wake")
                .await
                .unwrap();
            journal.rejected("closed-wake").await.unwrap();
            store.request_account_cleanup("alice").await.unwrap();
            assert!(
                store
                    .retry_rejected_lifecycle("alice", &request(&entry, "closed-wake", "wake"))
                    .await
                    .is_err()
            );
            assert_eq!(
                db.client()
                    .await
                    .unwrap()
                    .query_one("SELECT count(*) FROM computers.lifecycle_retries", &[])
                    .await
                    .unwrap()
                    .get::<_, i64>(0),
                8
            );
        })
        .await;
}
