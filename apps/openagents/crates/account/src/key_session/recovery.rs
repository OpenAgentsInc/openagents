//! Commit recovery while holding the same generation lock as every account switch.
use super::*;
use openagents::CancellationToken;

impl Controller {
    /// Validate the expected account, commit its local identity, and reserve sign-in atomically.
    ///
    /// `restore` performs only synchronous identity-store validation/write and the loaded-identity
    /// update. It must not call back into this controller or perform network work. A failed store
    /// operation leaves the account generation and session intact. Credential/session work remains
    /// in the existing cancellable sign-in path after the state lock is released.
    pub fn restore_identity_if_current<E>(
        self: &Arc<Self>,
        expected_generation: u64,
        restore: impl FnOnce() -> Result<Arc<Identity>, E>,
        stale: E,
        computer: String,
    ) -> Result<
        (
            Arc<Identity>,
            std::pin::Pin<Box<dyn Future<Output = ()> + Send + 'static>>,
        ),
        E,
    > {
        let (identity, ticket) = {
            let mut state = self.state.lock().unwrap();
            if state.generation != expected_generation || state.generation == u64::MAX {
                return Err(stale);
            }
            let identity = restore()?;
            state.cancel.cancel();
            state.generation += 1;
            state.cancel = CancellationToken::new();
            state.checking_saved = false;
            let ticket = Ticket {
                generation: state.generation,
                cancel: state.cancel.clone(),
                previous: state.session.take(),
            };
            self.updates.send_replace(crate::Snapshot {
                generation: state.generation,
                phase: Phase::Starting,
            });
            (identity, ticket)
        };
        let this = self.clone();
        let signing_identity = identity.clone();
        Ok((
            identity,
            Box::pin(async move {
                if this.current(ticket.generation) {
                    this.start_inner(ticket, signing_identity, computer, true)
                        .await;
                }
            }),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::desktop::MemoryStore;
    use openagents::{Account, Client, Deployment};
    use std::{
        sync::{
            Mutex,
            atomic::{AtomicBool, Ordering},
            mpsc,
        },
        time::Duration,
    };

    fn controller(dir: &std::path::Path) -> Arc<Controller> {
        let origin = "http://127.0.0.1:9";
        Controller::new(
            Client::new(Deployment::new(&format!("{origin}/v1"), origin).unwrap()).unwrap(),
            Arc::new(MemoryStore::default()),
            dir.join("signed-out"),
        )
    }
    fn new_account() -> Phase {
        Phase::SignedIn {
            account: Account {
                id: "synthetic-new-account".into(),
                label: "Synthetic account".into(),
                email: None,
                avatar_url: None,
            },
        }
    }
    #[test]
    fn switched_account_rejects_restore_without_calling_identity_writer() {
        let dir = tempfile::tempdir().unwrap();
        let c = controller(dir.path());
        let old = c.snapshot().generation;
        c.begin(new_account());
        let called = AtomicBool::new(false);
        let result = c.restore_identity_if_current(
            old,
            || {
                called.store(true, Ordering::SeqCst);
                Ok(Arc::new(Identity::from_entropy(&[0; 16]).unwrap()))
            },
            "stale",
            "fixture".into(),
        );
        assert!(matches!(result, Err("stale")));
        assert!(!called.load(Ordering::SeqCst));
        assert!(
            matches!(c.snapshot().phase,Phase::SignedIn{account} if account.id=="synthetic-new-account")
        );
    }
    #[test]
    fn account_switch_cannot_interleave_validation_and_identity_commit() {
        let dir = tempfile::tempdir().unwrap();
        let c = controller(dir.path());
        let expected = c.snapshot().generation;
        let bytes = Arc::new(Mutex::new(vec![0u8; 16]));
        let (entered_tx, entered_rx) = mpsc::channel();
        let (release_tx, release_rx) = mpsc::channel();
        let restore_c = c.clone();
        let restore_bytes = bytes.clone();
        let restoring = std::thread::spawn(move || {
            let (_, future) = restore_c
                .restore_identity_if_current(
                    expected,
                    || {
                        entered_tx.send(()).unwrap();
                        release_rx.recv_timeout(Duration::from_secs(5)).unwrap();
                        *restore_bytes.lock().unwrap() = vec![1; 16];
                        Ok(Arc::new(Identity::from_entropy(&[1; 16]).unwrap()))
                    },
                    "stale",
                    "fixture".into(),
                )
                .unwrap();
            drop(future); // Synthetic test never polls a network future.
        });
        entered_rx.recv_timeout(Duration::from_secs(5)).unwrap();
        assert!(
            c.state.try_lock().is_err(),
            "The actual account-switch lock must cover the store write"
        );
        let switch_c = c.clone();
        let switch_bytes = bytes.clone();
        let switching = std::thread::spawn(move || {
            switch_c.begin(new_account());
            *switch_bytes.lock().unwrap() = vec![2; 16];
        });
        release_tx.send(()).unwrap();
        restoring.join().unwrap();
        switching.join().unwrap();
        assert_eq!(*bytes.lock().unwrap(), vec![2; 16]);
        assert_eq!(c.snapshot().generation, expected + 2);
        assert!(
            matches!(c.snapshot().phase,Phase::SignedIn{account} if account.id=="synthetic-new-account")
        );
        let called = AtomicBool::new(false);
        assert!(
            c.restore_identity_if_current(
                expected,
                || {
                    called.store(true, Ordering::SeqCst);
                    Ok(Arc::new(Identity::from_entropy(&[0; 16]).unwrap()))
                },
                "stale",
                "fixture".into()
            )
            .is_err()
        );
        assert!(!called.load(Ordering::SeqCst));
    }
    #[test]
    fn rejected_identity_write_keeps_generation_and_active_account() {
        let dir = tempfile::tempdir().unwrap();
        let c = controller(dir.path());
        c.begin(new_account());
        let generation = c.snapshot().generation;
        let result = c.restore_identity_if_current(
            generation,
            || Err("wallet differs"),
            "stale",
            "fixture".into(),
        );
        assert!(matches!(result, Err("wallet differs")));
        assert_eq!(c.snapshot().generation, generation);
        assert!(
            matches!(c.snapshot().phase,Phase::SignedIn{account} if account.id=="synthetic-new-account")
        );
    }
}
