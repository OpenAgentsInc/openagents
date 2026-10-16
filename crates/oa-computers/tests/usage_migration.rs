//! Upgrade an existing allocation without inventing past usage.

use coder_working_computer::{
    Applied, Command, Computer, Phase, Trigger, apply,
    driver::{self, Held},
    on_driver::OnDriver,
    provider::{Outcome, Provider},
};
use computer_driver::{
    Driver,
    fake::{FakeDriver, Inject},
};
use oa_computers::{Db, Store, registry::CreateIntent, store::PgLease};
use std::time::Duration;
use tokio_postgres::NoTls;

const OLD_MIGRATIONS: [&str; 6] = [
    include_str!("../migrations/0001_computers.sql"),
    include_str!("../migrations/0002_turns.sql"),
    include_str!("../migrations/0003_create_requests.sql"),
    include_str!("../migrations/0004_commands.sql"),
    include_str!("../migrations/0005_deployments.sql"),
    include_str!("../migrations/0006_lifecycle_intents.sql"),
];

fn changed(record: &Computer, command: Command, now: u64) -> Computer {
    match apply(record, &command, now).unwrap() {
        Applied::Changed(next) => *next,
        Applied::Unchanged => panic!("expected a lifecycle change"),
    }
}

#[tokio::test]
async fn migration_preserves_unknown_allocation_and_recovers_with_the_new_writer() {
    let Ok(admin) = std::env::var("OA_COMPUTERS_TEST_DATABASE_URL") else {
        return;
    };
    let (fresh, dsn) = Db::scratch(&admin).await.unwrap();
    drop(fresh);
    // Keep this connection open across the upgrade, as an old service would.
    let (old, connection) = tokio_postgres::connect(&dsn, NoTls).await.unwrap();
    tokio::spawn(async move { connection.await.unwrap() });
    old.batch_execute(
        "DROP SCHEMA computers CASCADE; CREATE SCHEMA computers;
         CREATE TABLE computers.migrations(version integer PRIMARY KEY,
             applied_at timestamptz NOT NULL DEFAULT now())",
    )
    .await
    .unwrap();
    for (index, sql) in OLD_MIGRATIONS.into_iter().enumerate() {
        old.batch_execute(sql).await.unwrap();
        old.execute(
            "INSERT INTO computers.migrations(version) VALUES($1)",
            &[&((index + 1) as i32)],
        )
        .await
        .unwrap();
    }

    let root = tempfile::tempdir().unwrap();
    let fake = FakeDriver::new(root.path());
    let provider = OnDriver::new(fake.clone(), None);
    let now = oa_computers::reconcile::now_ms();
    let new =
        oa_computers::api::admit_create(&fake.capabilities(), "alice", &CreateIntent::default())
            .unwrap();
    let operation = "migration-create".to_owned();
    let requested = changed(
        &new,
        Command::RequestCreate {
            operation: operation.clone(),
        },
        now,
    );
    fake.inject("create", Inject::LoseReply);
    let outcome = Provider::create(&provider, &requested, &operation).await;
    assert!(matches!(outcome, Outcome::Unknown { .. }));
    let unknown = changed(
        &requested,
        Command::ObserveCreate { operation, outcome },
        now,
    );
    assert!(matches!(unknown.phase, Phase::Unknown { .. }));
    assert_eq!(fake.live().len(), 1);
    let resource = fake.live()[0].0.clone();
    let record = serde_json::to_value(&unknown).unwrap();
    old.execute(
        "INSERT INTO computers.computers(id,owner,driver,size,desired,phase,revision,record)
         VALUES($1,'alice','fake',$2,'running','unknown',$3,$4)",
        &[
            &unknown.id,
            &unknown.size,
            &(unknown.revision as i64),
            &record,
        ],
    )
    .await
    .unwrap();
    old.execute(
        "INSERT INTO computers.turns(id,computer,owner,state,command,timeout_seconds)
         VALUES('old-turn',$1,'alice','command_queued','true',1)",
        &[&unknown.id],
    )
    .await
    .unwrap();
    old.execute(
        "INSERT INTO computers.commands(turn,owner,computer,request_key,digest,options,status)
         VALUES('old-turn','alice',$1,'old-request',$2,'{}','queued')",
        &[&unknown.id, &"0".repeat(64)],
    )
    .await
    .unwrap();

    let db = Db::connect(&dsn).await.unwrap();
    let store = Store::new(db.clone());
    let migrated = store.usage("alice", 0, 200).await.unwrap();
    let events = migrated["events"].as_array().unwrap();
    assert_eq!(events.len(), 1);
    assert_eq!(events[0]["event_key"], "migration:7");
    assert_eq!(events[0]["phase"], "unknown");
    assert_eq!(events[0]["evidence"]["history"], "unknown");
    assert!(events[0]["evidence"]["provider_seconds"].is_null());
    assert!(migrated["customer_charge"].is_null());
    assert!(migrated["allocation_allowance"]["held_cpu_ms"].is_null());
    assert_eq!(migrated["allocation_allowance"]["unknown_allowances"], 1);
    assert_eq!(migrated["allocation_allowance"]["unresolved_allowances"], 1);

    // Old workers must not keep taking jobs or extending an existing lease.
    assert!(
        old.query_opt(
            "UPDATE computers.computers SET lease_holder='old-worker',
         lease_until=now()+interval '30 seconds',binding_lease_until=now()+interval '30 seconds'
         WHERE id=(SELECT id FROM computers.computers WHERE NOT done AND next_attempt_at<=now()
             AND (lease_until IS NULL OR lease_until<now()) ORDER BY next_attempt_at
             LIMIT 1 FOR UPDATE SKIP LOCKED) RETURNING id",
            &[],
        )
        .await
        .unwrap()
        .is_none()
    );
    let old_holder = "old-worker";
    let held = store
        .claim(old_holder, Duration::from_secs(30))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(held.id, unknown.id);
    let before: serde_json::Value = old
        .query_one(
            "SELECT to_jsonb(c) FROM computers.computers c WHERE id=$1",
            &[&unknown.id],
        )
        .await
        .unwrap()
        .get(0);
    assert_eq!(
        old.execute(
            "UPDATE computers.computers SET lease_until=now()+interval '1 hour'
         WHERE id=$1 AND lease_holder=$2",
            &[&unknown.id, &old_holder],
        )
        .await
        .unwrap(),
        0
    );
    assert_eq!(
        old.execute(
            "UPDATE computers.computers SET lease_holder=NULL,lease_until=NULL,
         next_attempt_at=GREATEST(next_attempt_at,now()+interval '1 hour'),
         attempts=9,last_error='old-worker',done=true,updated_at=now()
         WHERE id=$1 AND lease_holder=$2",
            &[&unknown.id, &old_holder],
        )
        .await
        .unwrap(),
        0
    );
    assert_eq!(
        old.execute(
            "UPDATE computers.computers SET next_attempt_at=now() WHERE id=$1 AND NOT done",
            &[&unknown.id],
        )
        .await
        .unwrap(),
        0
    );
    let after: serde_json::Value = old
        .query_one(
            "SELECT to_jsonb(c) FROM computers.computers c WHERE id=$1",
            &[&unknown.id],
        )
        .await
        .unwrap()
        .get(0);
    assert_eq!(
        after, before,
        "old lease and scheduling writes must change nothing"
    );
    assert!(
        store
            .renew(&unknown.id, old_holder, Duration::from_secs(30))
            .await
            .unwrap()
    );
    store
        .release(&unknown.id, old_holder, Duration::ZERO, 0, None, false)
        .await
        .unwrap();
    store.poke(&unknown.id).await.unwrap();

    assert_eq!(
        old.execute(
            "UPDATE computers.computers SET revision=revision+1,
         record=jsonb_set(record,'{revision}',to_jsonb(revision+1)) WHERE id=$1",
            &[&unknown.id],
        )
        .await
        .unwrap(),
        0
    );
    assert_eq!(old.execute(
        "UPDATE computers.computers SET desired='deleted',intent_generation=intent_generation+1 WHERE id=$1",
        &[&unknown.id],
    ).await.unwrap(), 0);
    assert!(
        old.execute(
            "INSERT INTO computers.turns(id,computer,owner,state,command,timeout_seconds)
         VALUES('late-old-turn',$1,'alice','queued','true',3600)",
            &[&unknown.id],
        )
        .await
        .is_err()
    );
    assert!(
        old.execute(
            "UPDATE computers.turns SET state='failed' WHERE id='old-turn'",
            &[]
        )
        .await
        .is_err()
    );
    assert!(
        old.execute(
            "UPDATE computers.commands SET cancel_requested=true WHERE turn='old-turn'",
            &[]
        )
        .await
        .is_err()
    );

    let lease_seconds = Duration::from_secs(30);
    let entry = store
        .claim("migration-worker", lease_seconds)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(entry.id, unknown.id);
    let lease = PgLease {
        store: store.clone(),
        id: entry.id,
        holder: "migration-worker".into(),
        lease: lease_seconds,
    };
    let settled = driver::settle(&provider, &lease, Trigger::Wake, now)
        .await
        .unwrap();
    assert!(matches!(settled.computer().phase, Phase::Awake));
    assert_eq!(settled.computer().resource(), Some(resource.as_str()));
    assert_eq!(
        fake.live().len(),
        1,
        "recovery must reuse the original create operation"
    );
    assert!(lease.read().await.unwrap().revision > unknown.revision);
    let usage = store.usage("alice", 0, 200).await.unwrap();
    assert!(usage["allocation_allowance"]["held_cpu_ms"].is_null());
    assert_eq!(usage["allocation_allowance"]["unknown_allowances"], 1);
    assert_eq!(usage["allocation_allowance"]["unresolved_allowances"], 1);
    let allowance = db.client().await.unwrap().query_one(
        "SELECT state,maximum_cpu_ms FROM computers.usage_reservations WHERE computer=$1 AND boot=1", &[&unknown.id],
    ).await.unwrap();
    assert_eq!(allowance.get::<_, &str>(0), "unknown");
    assert_eq!(allowance.get::<_, Option<i64>>(1), None);
    db.client()
        .await
        .unwrap()
        .execute(
            "UPDATE computers.commands SET status='cancelled' WHERE turn='old-turn'",
            &[],
        )
        .await
        .unwrap();
    db.client()
        .await
        .unwrap()
        .execute(
            "UPDATE computers.turns SET state='failed' WHERE id='old-turn'",
            &[],
        )
        .await
        .unwrap();
}
