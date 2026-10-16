//! Recover command results and dispatch identities across lifecycle write gaps.

use coder_working_computer::{
    Phase, Trigger,
    driver::{self, Held, Settled},
    on_driver::OnDriver,
};
use computer_driver::{
    CommandCursor, CommandProgress, CommandSpec, Driver, Outcome, fake::FakeDriver,
};
use oa_computers::{
    Db, Store,
    commands::Request,
    reconcile::{Reconciler, Settings, now_ms},
    registry::CreateIntent,
    store::{Desired, NewComputer, PgLease},
    turns::Secrets,
};
use std::{rc::Rc, time::Duration};

const OWNER: &str = "command-recovery-test";
const HOLDER: &str = "before-crash";
const LEASE: Duration = Duration::from_secs(30);

struct Rig {
    db: Db,
    store: Store,
    fake: FakeDriver,
    root: tempfile::TempDir,
    lease: PgLease,
    provider: OnDriver<FakeDriver>,
    id: String,
}

impl Rig {
    async fn new() -> Option<Self> {
        let admin = std::env::var("OA_COMPUTERS_TEST_DATABASE_URL").ok()?;
        if admin.is_empty() {
            return None;
        }
        let (db, _) = Db::scratch(&admin).await.unwrap();
        let store = Store::new(db.clone());
        let root = tempfile::tempdir().unwrap();
        let fake = FakeDriver::new(root.path()).boot_observations(0);
        let record =
            oa_computers::api::admit_create(&fake.capabilities(), OWNER, &CreateIntent::default())
                .unwrap();
        let (entry, created) = store
            .create(&NewComputer {
                owner: OWNER,
                chat: None,
                idempotency_key: Some("recovery-computer"),
                driver: "fake",
                image: None,
                record: &record,
            })
            .await
            .unwrap();
        assert!(created);
        assert_eq!(
            store.claim(HOLDER, LEASE).await.unwrap().unwrap().id,
            entry.id
        );
        let lease = PgLease {
            store: store.clone(),
            id: entry.id.clone(),
            holder: HOLDER.into(),
            lease: LEASE,
        };
        let mut provider = OnDriver::new(fake.clone(), None);
        provider.poll = Duration::from_millis(1);
        provider.deadline = Duration::from_secs(5);
        assert_eq!(
            driver::settle(&provider, &lease, Trigger::Wake, now_ms())
                .await
                .unwrap()
                .computer()
                .phase,
            Phase::Awake
        );
        Some(Self {
            db,
            store,
            fake,
            root,
            lease,
            provider,
            id: entry.id,
        })
    }

    async fn queue(&self, key: &str, effect: &str) -> (String, Request) {
        let request = Request {
            command: format!("printf '{effect}\\n' >> counter; printf '{effect}'"),
            cwd: ".".into(),
            timeout_seconds: 5,
            env: Default::default(),
            idempotency_key: key.into(),
            credential_ref: None,
        };
        let (id, created) = self
            .store
            .add_command(OWNER, &self.id, &request)
            .await
            .unwrap();
        assert!(created);
        (id, request)
    }

    async fn start_core_turn(&self) {
        let dispatch = driver::settle(&self.provider, &self.lease, Trigger::Prompt, now_ms())
            .await
            .unwrap();
        assert!(matches!(dispatch, Settled::Dispatch { generation: 1, .. }));
    }

    async fn release(&self) {
        self.store
            .release(&self.id, HOLDER, Duration::ZERO, 0, None, false)
            .await
            .unwrap();
    }

    async fn visit(&self) {
        self.store.poke(&self.id).await.unwrap();
        let mut settings = Settings::new("after-crash");
        settings.lease = LEASE;
        settings.revisit = Duration::ZERO;
        settings.backoff = Duration::ZERO;
        settings.heartbeat = Duration::from_millis(100);
        settings.driver_poll = Duration::from_millis(1);
        settings.driver_deadline = Duration::from_secs(5);
        let entry = self
            .store
            .claim(&settings.holder, LEASE)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(entry.id, self.id);
        let reconciler = Rc::new(Reconciler {
            store: self.store.clone(),
            driver: self.fake.clone(),
            settings,
            turns: Secrets::default(),
        });
        tokio::time::timeout(Duration::from_secs(10), reconciler.visit(entry))
            .await
            .expect("one recovery visit must finish");
    }

    fn starts(&self) -> usize {
        self.fake
            .calls()
            .iter()
            .filter(|call| call.starts_with("start_command:"))
            .count()
    }

    async fn effects(&self) -> String {
        let entry = self.store.get(OWNER, &self.id).await.unwrap().unwrap();
        std::fs::read_to_string(
            self.root
                .path()
                .join(entry.record.resource().unwrap())
                .join("counter"),
        )
        .unwrap()
    }

    async fn execute_and_save_result_without_finishing_core(&self, id: &str, request: &Request) {
        let record = self.lease.read().await.unwrap();
        let resource = record.resource().unwrap();
        let command = self
            .store
            .command(OWNER, &self.id, id)
            .await
            .unwrap()
            .unwrap();
        // This marked fixture connection writes the same queue ownership that
        // command dispatch persists, while leaving the core's generation active.
        self.db.client().await.unwrap().execute(
            "WITH owned AS (UPDATE computers.turns SET state='command_running',generation=1,started_at=now()
                WHERE id=$1 RETURNING id)
             UPDATE computers.commands SET status='running',provider_resource=$2,provider_reference=$1,dispatched_at=now()
                WHERE turn IN (SELECT id FROM owned)", &[&id,&resource],
        ).await.unwrap();
        let spec = CommandSpec {
            id: id.into(),
            command: request.command.clone(),
            cwd: request.cwd.clone(),
            credential_names: Default::default(),
            env: Default::default(),
            timeout_seconds: request.timeout_seconds,
            digest: command.digest.clone(),
        };
        assert!(matches!(
            self.fake.start_command(resource, &spec).await,
            Outcome::Done { .. }
        ));
        let output = tokio::time::timeout(Duration::from_secs(5), async {
            loop {
                if let Outcome::Done { value } = self
                    .fake
                    .read_command(resource, id, CommandCursor::default(), 65536)
                    .await
                    && value.progress == (CommandProgress::Exited { code: 0 })
                {
                    assert_eq!(value.digest.as_deref(), Some(command.digest.as_str()));
                    break value.stdout;
                }
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await
        .expect("the detached fixture command must finish");
        // Simulate death after the atomic command/queue result commit, before
        // driver::turn_finished can update the core or checkpoint the computer.
        self.db.client().await.unwrap().execute(
            "WITH ended AS (UPDATE computers.commands SET status='succeeded',stdout=$2,finished_at=now(),
                    expires_at=now()-interval '1 second' WHERE turn=$1 RETURNING turn)
             UPDATE computers.turns SET state='done',exit_code=0,finished_at=now()
                WHERE id IN (SELECT turn FROM ended)", &[&id,&output],
        ).await.unwrap();
    }
}

#[tokio::test]
async fn terminal_result_finishes_its_generation_before_a_later_command() {
    tokio::task::LocalSet::new()
        .run_until(async {
            let Some(rig) = Rig::new().await else { return };
            let (first, request) = rig.queue("first", "first").await;
            rig.start_core_turn().await;
            rig.execute_and_save_result_without_finishing_core(&first, &request)
                .await;
            rig.store.prune_command_output().await.unwrap();
            assert!(
                rig.store
                    .command(OWNER, &rig.id, &first)
                    .await
                    .unwrap()
                    .unwrap()
                    .stdout
                    .is_empty()
            );
            let before = rig.lease.read().await.unwrap();
            assert_eq!(before.phase, Phase::Turn { generation: 1 });
            assert_eq!(before.turn.completed, 0);
            let (second, _) = rig.queue("second", "second").await;
            rig.release().await;

            rig.visit().await;
            let recovered = rig.store.get(OWNER, &rig.id).await.unwrap().unwrap();
            assert_eq!(recovered.record.phase, Phase::Stopped);
            assert_eq!(recovered.desired, Desired::Running);
            assert_eq!(recovered.record.turn.completed, 1);
            assert_eq!(recovered.record.turn.dispatched, 1);
            let first_turn = rig
                .store
                .turn(OWNER, &rig.id, &first)
                .await
                .unwrap()
                .unwrap();
            assert_eq!(first_turn.state, "done");
            assert_eq!(first_turn.generation, Some(1));
            let pending = rig
                .store
                .turn(OWNER, &rig.id, &second)
                .await
                .unwrap()
                .unwrap();
            assert_eq!(pending.state, "command_queued");
            assert_eq!(pending.generation, None);
            assert_eq!(
                rig.starts(),
                1,
                "recovery must not replay or start the later command"
            );
            assert_eq!(rig.effects().await, "first\n");

            rig.visit().await;
            let finished = rig.store.get(OWNER, &rig.id).await.unwrap().unwrap();
            assert_eq!(finished.record.phase, Phase::Stopped);
            assert_eq!(finished.record.turn.completed, 2);
            assert_eq!(finished.record.turn.dispatched, 2);
            let second_turn = rig
                .store
                .turn(OWNER, &rig.id, &second)
                .await
                .unwrap()
                .unwrap();
            assert_eq!(second_turn.state, "done");
            assert_eq!(second_turn.generation, Some(2));
            assert_eq!(rig.starts(), 2);
            assert_eq!(rig.effects().await, "first\nsecond\n");
            rig.visit().await;
            assert_eq!(
                rig.starts(),
                2,
                "a further visit must not dispatch either command again"
            );
        })
        .await;
}

#[tokio::test]
async fn a_turn_without_a_saved_queue_generation_waits_on_lookup_failure_then_recovers_once() {
    tokio::task::LocalSet::new()
        .run_until(async {
            let Some(rig) = Rig::new().await else { return };
            let (command, _) = rig.queue("unbound", "once").await;
            rig.start_core_turn().await;
            let queued = rig
                .store
                .turn(OWNER, &rig.id, &command)
                .await
                .unwrap()
                .unwrap();
            assert_eq!(queued.state, "command_queued");
            assert_eq!(queued.generation, None);
            let before = rig.lease.read().await.unwrap();
            rig.release().await;
            let calls = rig.fake.calls();
            // A missing queue is an error, not evidence that the active generation
            // has no command. The other tables remain available for lease release.
            rig.db
                .client()
                .await
                .unwrap()
                .batch_execute("ALTER TABLE computers.turns RENAME TO turns_unavailable")
                .await
                .unwrap();
            rig.visit().await;
            rig.db
                .client()
                .await
                .unwrap()
                .batch_execute("ALTER TABLE computers.turns_unavailable RENAME TO turns")
                .await
                .unwrap();
            let blocked = rig.store.get(OWNER, &rig.id).await.unwrap().unwrap();
            assert_eq!(blocked.attempts, 1);
            assert!(blocked.last_error.is_some());
            assert_eq!(blocked.record.revision, before.revision);
            assert_eq!(blocked.record.phase, Phase::Turn { generation: 1 });
            assert_eq!(rig.fake.calls(), calls);
            assert_eq!(rig.starts(), 0);

            rig.visit().await;
            let recovered = rig.store.get(OWNER, &rig.id).await.unwrap().unwrap();
            assert_eq!(recovered.record.phase, Phase::Stopped);
            assert_eq!(recovered.record.turn.completed, 1);
            assert_eq!(recovered.record.turn.dispatched, 1);
            assert_eq!(recovered.attempts, 0);
            assert!(recovered.last_error.is_none());
            let finished = rig
                .store
                .turn(OWNER, &rig.id, &command)
                .await
                .unwrap()
                .unwrap();
            assert_eq!(finished.state, "done");
            assert_eq!(finished.generation, Some(1));
            assert_eq!(rig.starts(), 1);
            assert_eq!(rig.effects().await, "once\n");
            rig.visit().await;
            assert_eq!(rig.starts(), 1);
        })
        .await;
}
