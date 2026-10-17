//! Wire-level recovery checks preserve every accepted or uncertain request.
use super::*;
use std::{
    collections::BTreeMap,
    sync::{Arc, Mutex},
};

#[derive(Default)]
struct Journal(
    Mutex<BTreeMap<String, LifecycleClaim>>,
    Mutex<BTreeMap<String, String>>,
);
impl LifecycleJournal for Journal {
    fn authorized_retry<'a>(
        &'a self,
        key: &'a str,
        _resource: &'a str,
        _action: &'a str,
    ) -> JournalFuture<'a, Option<String>> {
        Box::pin(async move { Ok(self.1.lock().unwrap().get(key).cloned()) })
    }

    fn claim<'a>(
        &'a self,
        key: &'a str,
        _resource: &'a str,
        _action: &'a str,
    ) -> JournalFuture<'a, LifecycleClaim> {
        Box::pin(async move {
            let mut values = self.0.lock().unwrap();
            Ok(match values.get(key) {
                Some(saved) => saved.clone(),
                None => {
                    values.insert(key.into(), LifecycleClaim::Uncertain);
                    LifecycleClaim::Fresh
                }
            })
        })
    }
    fn accepted<'a>(&'a self, key: &'a str, operation: &'a str) -> JournalFuture<'a, ()> {
        Box::pin(async move {
            self.0
                .lock()
                .unwrap()
                .insert(key.into(), LifecycleClaim::Accepted(operation.into()));
            Ok(())
        })
    }
    fn rejected<'a>(&'a self, key: &'a str) -> JournalFuture<'a, ()> {
        Box::pin(async move {
            self.0
                .lock()
                .unwrap()
                .insert(key.into(), LifecycleClaim::Rejected);
            Ok(())
        })
    }
}
fn authorized() -> (Arc<Journal>, String) {
    let journal = Arc::new(Journal::default());
    let child = format!("lifecycle-retry:{}", "a".repeat(64));
    journal
        .0
        .lock()
        .unwrap()
        .insert("refused-root".into(), LifecycleClaim::Rejected);
    journal
        .1
        .lock()
        .unwrap()
        .insert("refused-root".into(), child.clone());
    (journal, child)
}

#[tokio::test]
async fn authorized_retry_uses_a_new_wire_key_and_resumes_the_accepted_child() {
    let (journal, child) = authorized();
    let expected = child.clone();
    let (driver, server) = super::tests::create_server_checked(
        vec![
            (
                "/v1/sandboxes/sbx_test:suspend",
                202,
                json!({"operation_id":"op_child"}),
            ),
            ("/v1/operations/op_child", 200, json!({"state":"succeeded"})),
            ("/v1/operations/op_child", 200, json!({"state":"succeeded"})),
        ],
        move |headers| {
            if headers.starts_with("POST ") {
                let key = headers.lines().find_map(|line| {
                    line.split_once(':').and_then(|(k, v)| {
                        k.eq_ignore_ascii_case("idempotency-key")
                            .then_some(v.trim())
                    })
                });
                assert_eq!(key, Some(expected.as_str()));
            }
        },
    );
    let driver = driver.with_lifecycle_journal(journal.clone());
    for _ in 0..2 {
        assert!(
            matches!(driver.suspend_operation("sbx_test","refused-root").await,Outcome::Done{value} if value=="op_child")
        );
    }
    server.join().unwrap();
    assert_eq!(
        journal.0.lock().unwrap().get(&child),
        Some(&LifecycleClaim::Accepted("op_child".into()))
    );
}

#[tokio::test]
async fn an_unknown_child_response_is_never_submitted_twice() {
    let (journal, child) = authorized();
    let (driver, server) = super::tests::create_server_checked(
        vec![
            // Successful HTTP without an operation identity is still uncertain.
            ("/v1/sandboxes/sbx_test:suspend", 202, json!({})),
        ],
        |_| {},
    );
    let driver = driver.with_lifecycle_journal(journal.clone());
    assert!(matches!(
        driver.suspend_operation("sbx_test", "refused-root").await,
        Outcome::Unknown { .. }
    ));
    server.join().unwrap();
    assert!(matches!(
        driver.suspend_operation("sbx_test", "refused-root").await,
        Outcome::Unknown { .. }
    ));
    assert_eq!(
        journal.0.lock().unwrap().get(&child),
        Some(&LifecycleClaim::Uncertain)
    );
}

#[tokio::test]
async fn missing_authority_invalid_aliases_and_cycles_cannot_send_mutations() {
    let driver = SpaceDriver::new(
        "http://127.0.0.1:1",
        "http://127.0.0.1:1",
        Secret::new("test"),
        "seed:latest",
    )
    .unwrap();
    let journal = Arc::new(Journal::default());
    journal
        .0
        .lock()
        .unwrap()
        .insert("refused-root".into(), LifecycleClaim::Rejected);
    let driver = driver.with_lifecycle_journal(journal.clone());
    assert!(matches!(
        driver.suspend_operation("sbx_test", "refused-root").await,
        Outcome::Failed { .. }
    ));
    for alias in [
        "refused-root".to_owned(),
        "untrusted-key".to_owned(),
        format!("lifecycle-retry:{}", "b".repeat(64)),
    ] {
        journal
            .1
            .lock()
            .unwrap()
            .insert("refused-root".into(), alias.clone());
        journal
            .0
            .lock()
            .unwrap()
            .insert(alias.clone(), LifecycleClaim::Rejected);
        journal.1.lock().unwrap().insert(alias.clone(), alias);
        assert!(matches!(
            driver.suspend_operation("sbx_test", "refused-root").await,
            Outcome::Unknown { .. }
        ));
    }
}
