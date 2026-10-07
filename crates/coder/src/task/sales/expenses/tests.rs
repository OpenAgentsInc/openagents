use super::*;
use crate::task::sales::expenses as e;
fn source(kind: e::Kind) -> e::Source {
    e::Source {
        basis: e::Basis::ListPrice,
        kind,
        source_revision: "a".repeat(64),
        price_revision: "b".repeat(64),
        recipient: "human:operator".into(),
        max_input_bytes: 100,
        max_output_tokens: 100,
        max_attempts: 1,
        max_elapsed_secs: 100,
        input_usd_millionths_per_million: 5_000_000,
        output_usd_millionths_per_million: 5_000_000,
    }
}
fn input(request: &str, source: e::Source) -> e::Input {
    let bytes = vec![b'x'; 100];
    e::Input {
        request: request.into(),
        attempt: 1,
        source,
        input_bytes: bytes.len() as u64,
        input_sha256: digest(&bytes),
    }
}
fn policy(sources: Vec<e::Source>, floor: u64, agent: u64, request: u64) -> e::Policy {
    e::Policy {
        schema: "openagents.sales-model-policy.v1".into(),
        revision: 1,
        floor_daily_usd_millionths: floor,
        agent_daily_usd_millionths: agent,
        request_usd_millionths: request,
        sources,
    }
}
fn publish(f: &mut Fixture, p: &e::Policy) {
    f.store
        .publish_sales_model_policy(&f.owner, p, &p.sha256().unwrap())
        .unwrap();
}
fn settled(id: &str, estimate: Option<u64>, billed: Option<u64>) -> e::Settlement {
    e::Settlement {
        request: id.into(),
        estimated_usd_millionths: estimate,
        billed_usd_millionths: billed,
        evidence_sha256: "c".repeat(64),
    }
}
#[test]
fn concurrent_native_admissions_keep_one_floor_and_agent_bound() {
    let mut f = Fixture::with_execution_budget(2_000);
    let s = source(e::Kind::Planner);
    publish(&mut f, &policy(vec![s.clone()], 2_000, 2_000, 2_000));
    let root = f.dir.path().join("host");
    let secret = Store::read_credential(&f.credential).unwrap();
    drop(f.store);
    let guards = std::thread::scope(|scope| {
        let workers = (0..8)
            .map(|n| {
                let root = root.clone();
                let secret = secret.clone();
                let s = s.clone();
                scope.spawn(move || {
                    let mut store = Store::open_with_clock(&root, now).unwrap();
                    let access = store.authenticate_sales_agent(&secret).unwrap();
                    store.reserve_sales_model(&access, &input(&format!("concurrent-{n}"), s))
                })
            })
            .collect::<Vec<_>>();
        workers
            .into_iter()
            .map(|w| w.join().unwrap())
            .collect::<Vec<_>>()
    });
    assert_eq!(guards.iter().filter(|r| r.is_ok()).count(), 2);
    let mut store = Store::open_with_clock(&root, now).unwrap();
    let total: u64 = store
        .sales_model_reservations(&f.owner, None, 100)
        .unwrap()
        .iter()
        .map(|r| r.maximum_usd_millionths)
        .sum();
    assert_eq!(total, 2_000);
    assert!(
        store
            .sales_model_reservations(&f.owner, None, 100)
            .unwrap()
            .iter()
            .all(|r| r.status == e::Status::Reserved)
    );
    drop(guards);
    drop(store);
    store = Store::open_with_clock(&root, now).unwrap();
    assert!(
        store
            .sales_model_reservations(&f.owner, None, 100)
            .unwrap()
            .iter()
            .all(|r| r.status == e::Status::Unknown)
    );
    let a = store.authenticate_sales_agent(&secret).unwrap();
    assert!(
        store
            .reserve_sales_model(&a, &input("restart-does-not-refund", s))
            .is_err()
    );
}
#[test]
fn request_attempt_price_and_unknown_costs_remain_original() {
    let mut f = Fixture::with_execution_budget(2_000);
    let s = source(e::Kind::Retry);
    let p = policy(vec![s.clone()], 5_000, 5_000, 1_500);
    publish(&mut f, &p);
    let a = f.access();
    let original = input("one-request", s.clone());
    let hold = f.store.reserve_sales_model(&a, &original).unwrap();
    let r = hold.receipt().clone();
    let replay = f.store.reserve_sales_model(&a, &original).unwrap();
    assert_eq!(replay.receipt(), &r);
    assert!(!replay.may_execute());
    let mut conflicting = original.clone();
    conflicting.input_sha256 = "f".repeat(64);
    assert!(f.store.reserve_sales_model(&a, &conflicting).is_err());
    let unknown = settled("unknown-usage", None, None);
    assert_eq!(
        f.store
            .settle_sales_model(&f.owner, &r.id, &unknown)
            .unwrap()
            .status,
        e::Status::Unknown
    );
    assert!(
        f.store
            .reserve_sales_model(&a, &input("another-agent-request", s.clone()))
            .is_err()
    );
    let clear = settled("known-estimate", Some(1_000), None);
    let known = f.store.settle_sales_model(&f.owner, &r.id, &clear).unwrap();
    assert_eq!(known.status, e::Status::Known);
    assert_eq!(known.input, original);
    assert_eq!(
        f.store.settle_sales_model(&f.owner, &r.id, &clear).unwrap(),
        known
    );
    let mut retry = original.clone();
    retry.attempt = 2;
    assert!(f.store.reserve_sales_model(&a, &retry).is_err());
    let mut p2 = p.clone();
    p2.revision = 2;
    p2.request_usd_millionths = 5_000;
    p2.sources[0].price_revision = "e".repeat(64);
    publish(&mut f, &p2);
    assert_eq!(
        f.store
            .sales_model_reservation(&f.owner, &r.id)
            .unwrap()
            .input
            .source,
        s
    );
    retry.source = p2.sources[0].clone();
    assert!(f.store.reserve_sales_model(&a, &retry).is_err());
    assert!(
        f.store
            .settle_sales_model(&f.owner, &r.id, &settled("reprice", Some(1), None))
            .is_err()
    );
    let billed = f
        .store
        .settle_sales_model(
            &f.owner,
            &r.id,
            &settled("actual-provider-bill", Some(1_000), Some(1_400)),
        )
        .unwrap();
    assert_eq!(billed.status, e::Status::Breach);
    assert_eq!(billed.maximum_usd_millionths, 1_000);
    assert!(
        f.store
            .settle_sales_model(&f.owner, &r.id, &settled("erase-bill", Some(1_000), None))
            .is_err()
    );
}
#[test]
fn every_paid_work_kind_requires_current_native_price_recipient_and_budget() {
    let kinds = [
        e::Kind::Planner,
        e::Kind::Reporter,
        e::Kind::Coder,
        e::Kind::Jev,
        e::Kind::Claim,
        e::Kind::Price,
        e::Kind::CitedAnswer,
        e::Kind::Recommendation,
        e::Kind::Embedding,
        e::Kind::Reflection,
        e::Kind::Verification,
        e::Kind::Correction,
        e::Kind::Training,
        e::Kind::DayPlan,
        e::Kind::Retry,
    ];
    let mut f = Fixture::with_execution_budget(1_000);
    let sources = kinds.into_iter().map(source).collect::<Vec<_>>();
    publish(&mut f, &policy(sources.clone(), 20_000, 20_000, 1_000));
    let a = f.access();
    for (n, s) in sources.into_iter().enumerate() {
        let i = input(&format!("kind-{n}"), s.clone());
        let attempt = f.store.reserve_sales_model(&a, &i).unwrap();
        let r = attempt.receipt().clone();
        assert_eq!(r.input.source.kind, s.kind);
        f.store
            .settle_sales_model(
                &f.owner,
                &r.id,
                &settled(&format!("known-{n}"), Some(1_000), None),
            )
            .unwrap();
    }
    let mut wrong = input("wrong-recipient", source(e::Kind::Claim));
    wrong.source.recipient = "provider:ungranted".into();
    assert!(f.store.reserve_sales_model(&a, &wrong).is_err());
    let mut unpriced = source(e::Kind::Claim);
    unpriced.output_usd_millionths_per_million = 0;
    assert!(unpriced.sha256().is_err());
    let mut excessive = policy(
        vec![source(e::Kind::Claim)],
        e::FLOOR_USD_MILLIONTHS + 1,
        1_000,
        1_000,
    );
    assert!(excessive.sha256().is_err());
    excessive.floor_daily_usd_millionths = e::FLOOR_USD_MILLIONTHS;
    assert!(excessive.sha256().is_ok());
    let mut zero = Fixture::new();
    publish(
        &mut zero,
        &policy(vec![source(e::Kind::Claim)], 1_000, 1_000, 1_000),
    );
    let a = zero.access();
    assert!(
        zero.store
            .reserve_sales_model(&a, &input("no-native-budget", source(e::Kind::Claim)))
            .is_err()
    );
}
struct Local {
    source: e::Source,
    calls: usize,
}
impl e::Adapter for Local {
    type Output = String;
    fn source(&self) -> &e::Source {
        &self.source
    }
    fn execute(&mut self, body: &[u8], caps: &e::Source) -> Result<String> {
        assert_eq!(caps.basis, e::Basis::LocalDeterministic);
        assert!(body.len() as u64 <= caps.max_input_bytes);
        self.calls += 1;
        Ok("deterministic local fixture read; no model or bill".into())
    }
}
#[test]
fn deterministic_local_read_consumes_one_right_without_model_or_billing_claims() {
    let mut f = Fixture::new();
    let mut s = source(e::Kind::Claim);
    s.basis = e::Basis::LocalDeterministic;
    s.input_usd_millionths_per_million = 0;
    s.output_usd_millionths_per_million = 0;
    publish(&mut f, &policy(vec![s.clone()], 1_000, 1_000, 1_000));
    let a = f.access();
    let i = input("local-read", s.clone());
    let admission = f.store.reserve_sales_model(&a, &i).unwrap();
    let r = admission.receipt().clone();
    let root = f.dir.path().join("host");
    drop(f.store);
    let mut local = Local {
        source: s,
        calls: 0,
    };
    let (answer, execution) = admission.execute(&vec![b'x'; 100], &mut local).unwrap();
    assert_eq!(local.calls, 1);
    assert!(answer.contains("no model or bill"));
    let mut store = Store::open_with_clock(&root, now).unwrap();
    assert!(
        store
            .settle_sales_model(&f.owner, &r.id, &settled("pretend-bill", Some(0), Some(0)))
            .is_err()
    );
    assert!(
        store
            .settle_sales_model(&f.owner, &r.id, &settled("pretend-price", Some(1), None))
            .is_err()
    );
    let result = store
        .settle_sales_model(&f.owner, &r.id, &settled("known-local-zero", Some(0), None))
        .unwrap();
    assert_eq!(result.status, e::Status::Known);
    assert_eq!(result.maximum_usd_millionths, 0);
    assert_eq!(execution.receipt().id, result.id);
    drop(execution);
    let retry = store.reserve_sales_model(&a, &i).unwrap();
    assert!(!retry.may_execute());
    drop(store);
    assert!(retry.execute(&vec![b'x'; 100], &mut local).is_err());
    assert_eq!(local.calls, 1);
}

#[test]
fn native_crash_child() {
    let Some(root) = std::env::var_os("OA_EXPENSE_CRASH_ROOT") else {
        return;
    };
    let credential = std::env::var_os("OA_EXPENSE_CRASH_CREDENTIAL").unwrap();
    let ready = std::env::var_os("OA_EXPENSE_CRASH_READY").unwrap();
    let mut store = Store::open_with_clock(Path::new(&root), now).unwrap();
    let access = store
        .authenticate_sales_agent(&Store::read_credential(Path::new(&credential)).unwrap())
        .unwrap();
    let admission = store
        .reserve_sales_model(
            &access,
            &input("killed-native-attempt", source(e::Kind::Coder)),
        )
        .unwrap();
    let ready = PathBuf::from(ready);
    let pending = ready.with_extension("writing");
    std::fs::write(&pending, &admission.receipt().id).unwrap();
    std::fs::rename(pending, ready).unwrap();
    drop(store);
    loop {
        std::thread::sleep(std::time::Duration::from_millis(100));
        std::hint::black_box(&admission);
    }
}

#[test]
fn killed_native_process_keeps_original_cost_and_stops_floor() {
    let mut f = Fixture::with_execution_budget(2_000);
    publish(
        &mut f,
        &policy(vec![source(e::Kind::Coder)], 2_000, 2_000, 2_000),
    );
    let root = f.dir.path().join("host");
    let ready = f.dir.path().join("crash-ready");
    drop(f.store);
    let mut child = std::process::Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "task::sales::agents::tests::expense_tests::native_crash_child",
            "--nocapture",
        ])
        .env("OA_EXPENSE_CRASH_ROOT", &root)
        .env("OA_EXPENSE_CRASH_CREDENTIAL", &f.credential)
        .env("OA_EXPENSE_CRASH_READY", &ready)
        .stdout(std::process::Stdio::null())
        .spawn()
        .unwrap();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(15);
    while !ready.exists() && std::time::Instant::now() < deadline {
        if child.try_wait().unwrap().is_some() {
            panic!("native crash child exited before admission")
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    if !ready.exists() {
        child.kill().unwrap();
        child.wait().unwrap();
        panic!("native crash admission timed out")
    }
    let id = std::fs::read_to_string(&ready).unwrap();
    child.kill().unwrap();
    assert!(!child.wait().unwrap().success());
    let mut store = Store::open_with_clock(&root, now).unwrap();
    let r = store.sales_model_reservation(&f.owner, &id).unwrap();
    assert_eq!(r.status, e::Status::Unknown);
    assert_eq!(r.maximum_usd_millionths, 1_000);
    assert!(r.execution_unknown);
    let a = store
        .authenticate_sales_agent(&Store::read_credential(&f.credential).unwrap())
        .unwrap();
    assert!(
        store
            .reserve_sales_model(&a, &input("after-kill", source(e::Kind::Coder)))
            .is_err()
    );
}

fn next_day() -> u64 {
    now() + 86_400
}
fn backwards() -> u64 {
    now() - 1
}
#[test]
fn chicago_calendar_and_unknown_holds_do_not_roll_over() {
    let seconds = |s: &str| s.parse::<jiff::Timestamp>().unwrap().as_second() as u64;
    for (s, day) in [
        ("2026-11-01T04:59:59Z", 20261031),
        ("2026-11-01T05:00:00Z", 20261101),
        ("2026-11-01T06:30:00Z", 20261101),
        ("2026-11-01T07:30:00Z", 20261101),
        ("2026-11-02T06:00:00Z", 20261102),
        ("2026-03-08T06:00:00Z", 20260308),
        ("2026-03-09T05:00:00Z", 20260309),
    ] {
        assert_eq!(business_day(seconds(s)).unwrap(), day);
    }
    let mut f = Fixture::with_execution_budget(1_000);
    publish(
        &mut f,
        &policy(vec![source(e::Kind::Reflection)], 1_000, 1_000, 1_000),
    );
    let a = f.access();
    let hold = f
        .store
        .reserve_sales_model(&a, &input("midnight-original", source(e::Kind::Reflection)))
        .unwrap();
    let original = hold.receipt().clone();
    let root = f.dir.path().join("host");
    drop(f.store);
    assert!(Store::open_with_clock(&root, backwards).is_err());
    let mut store = Store::open_with_clock(&root, next_day).unwrap();
    let r = store
        .sales_model_reservation(&f.owner, &original.id)
        .unwrap();
    assert_eq!(r.day, business_day(now()).unwrap());
    assert_eq!(r.maximum_usd_millionths, 1_000);
    assert_eq!(r.status, e::Status::Unknown);
    assert!(hold.verify_input(&vec![b'x'; 100]).is_err());
}

#[test]
fn native_lifecycle_preserves_expense_scope_and_owner_history() {
    use crate::task::agent_lifecycle as life;
    let mut f = Fixture::with_execution_budget(1_000);
    publish(
        &mut f,
        &policy(vec![source(e::Kind::Training)], 1_000, 1_000, 1_000),
    );
    let a = f.access();
    let hold = f
        .store
        .reserve_sales_model(&a, &input("original-training", source(e::Kind::Training)))
        .unwrap();
    let original = hold.receipt().clone();
    drop(hold);
    let root = f.dir.path().join("host");
    let native = agent::Store::with_keys(&root, "paul", std::sync::Arc::new(FileKeys)).unwrap();
    let owner_key = secp256k1::SecretKey::from_byte_array([17; 32]).unwrap();
    let screen = secret_screen::Screen::shapes();
    let scope = native.load().unwrap().unwrap().sales_model_scope.unwrap();
    assert_eq!(scope.floor, original.floor);
    assert_eq!(scope.actor, original.actor);
    let snapshot = life::export(
        &native,
        &screen,
        life::MemoryChoice::None,
        Some(&owner_key),
        now(),
    )
    .unwrap();
    assert_eq!(snapshot.sales_model_scope.as_ref(), Some(&scope));
    let moved_root = f.dir.path().join("other-host");
    let moved =
        agent::Store::with_keys(&moved_root, "paul", std::sync::Arc::new(FileKeys)).unwrap();
    let imported = life::import(
        &moved,
        &screen,
        &snapshot,
        f.dir.path(),
        Some(&owner_key),
        now() + 5000,
        now(),
    )
    .unwrap();
    assert_eq!(imported.sales_model_scope.as_ref(), Some(&scope));
    assert!(
        imported
            .requires
            .iter()
            .any(|r| r == "sales-model-budget.v1")
    );
    let mut blank = Store::open_with_clock(&moved_root, now).unwrap();
    let moved_owner_file = f.dir.path().join("moved-owner");
    blank.initialize("operator", &moved_owner_file).unwrap();
    let moved_owner = blank
        .authenticate(&Store::read_credential(&moved_owner_file).unwrap())
        .unwrap();
    let p = policy(vec![source(e::Kind::Training)], 1_000, 1_000, 1_000);
    blank
        .publish_sales_model_policy(&moved_owner, &p, &p.sha256().unwrap())
        .unwrap();
    let training = e::TrainingContext {
        agent: "paul".into(),
        anchor: blank.sales_agent_anchor(&moved_owner, "paul").unwrap(),
        persona_sha256: "d".repeat(64),
        run: "migrated-native-run".into(),
        synthetic_source_sha256: "e".repeat(64),
    };
    assert!(
        blank
            .reserve_sales_training(
                &moved_owner,
                &training,
                &input("blank-controller-reset", source(e::Kind::Training))
            )
            .is_err()
    );
    drop(blank);
    life::rotate(
        &native,
        &screen,
        &owner_key,
        "fixture key rotation",
        now() + 5000,
        now(),
    )
    .unwrap();
    assert_eq!(
        native.load().unwrap().unwrap().sales_model_scope.as_ref(),
        Some(&scope)
    );
    assert!(
        f.store
            .reserve_sales_model(&a, &input("rotated", source(e::Kind::Training)))
            .is_err()
    );
    life::retire(&native, Some(&owner_key), now()).unwrap();
    let retained = f
        .store
        .sales_model_reservation(&f.owner, &original.id)
        .unwrap();
    assert_eq!(retained.status, e::Status::Unknown);
    assert_eq!(retained.native, original.native);
    assert_eq!(retained.actor, original.actor);
    assert_eq!(
        f.store
            .settle_sales_model(
                &f.owner,
                &original.id,
                &settled("after-retirement", Some(1_000), None)
            )
            .unwrap()
            .status,
        e::Status::Known
    );
}

#[test]
fn actual_adapter_body_rejects_credentials_customer_data_and_changed_authority() {
    for candidate in 0..3 {
        let mut f = Fixture::new();
        let body = match candidate {
            0 => "private workflow".to_string(),
            1 => "other-buyer@fixture.invalid".to_string(),
            _ => Store::read_credential(&f.credential).unwrap(),
        };
        let mut s = source(e::Kind::Claim);
        s.basis = e::Basis::LocalDeterministic;
        s.input_usd_millionths_per_million = 0;
        s.output_usd_millionths_per_million = 0;
        publish(&mut f, &policy(vec![s.clone()], 1_000, 1_000, 1_000));
        let a = f.access();
        let mut i = input("protected-body", s.clone());
        i.input_bytes = body.len() as u64;
        i.input_sha256 = digest(body.as_bytes());
        let admission = f.store.reserve_sales_model(&a, &i).unwrap();
        drop(f.store);
        let mut local = Local {
            source: s,
            calls: 0,
        };
        assert!(admission.execute(body.as_bytes(), &mut local).is_err());
        assert_eq!(local.calls, 0);
    }
}

#[test]
fn actual_adapter_post_call_rechecks_policy_and_discards_protected_results() {
    struct Changed {
        source: e::Source,
        root: PathBuf,
        owner: Access,
        policy: e::Policy,
        changed: bool,
    }
    impl e::Adapter for Changed {
        type Output = String;
        fn source(&self) -> &e::Source {
            &self.source
        }
        fn execute(&mut self, _body: &[u8], _caps: &e::Source) -> Result<String> {
            if self.changed {
                let mut store = Store::open_with_clock(&self.root, now)?;
                store.publish_sales_model_policy(
                    &self.owner,
                    &self.policy,
                    &self.policy.sha256()?,
                )?;
                Ok("opaque local result".into())
            } else {
                Ok("private workflow".into())
            }
        }
    }
    for changed in [false, true] {
        let mut f = Fixture::new();
        let mut s = source(e::Kind::Claim);
        s.basis = e::Basis::LocalDeterministic;
        s.input_usd_millionths_per_million = 0;
        s.output_usd_millionths_per_million = 0;
        let p = policy(vec![s.clone()], 1_000, 1_000, 1_000);
        publish(&mut f, &p);
        let a = f.access();
        let admission = f
            .store
            .reserve_sales_model(&a, &input("post-fence", s.clone()))
            .unwrap();
        let id = admission.receipt().id.clone();
        let root = f.dir.path().join("host");
        drop(f.store);
        let mut p2 = p;
        p2.revision = 2;
        let mut adapter = Changed {
            source: s,
            root: root.clone(),
            owner: f.owner,
            policy: p2,
            changed,
        };
        assert!(admission.execute(&vec![b'x'; 100], &mut adapter).is_err());
        let mut store = Store::open_with_clock(&root, now).unwrap();
        let retained = store.sales_model_reservation(&adapter.owner, &id).unwrap();
        assert!(retained.execution_unknown);
        assert_eq!(retained.status, e::Status::Known);
    }
}

#[test]
fn synthetic_training_has_no_customer_or_outbound_authority_and_shares_floor() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("host");
    let mut store = Store::open_with_clock(&root, now).unwrap();
    let owner_file = dir.path().join("owner");
    store.initialize("operator", &owner_file).unwrap();
    let owner = store
        .authenticate(&Store::read_credential(&owner_file).unwrap())
        .unwrap();
    let native = agent::Store::with_keys(&root, "paul", std::sync::Arc::new(FileKeys)).unwrap();
    let record = native
        .open_as(dir.path(), now(), agent::preset("paul"))
        .unwrap();
    let record = native.ensure_key(record, now()).unwrap();
    native
        .attest(
            record,
            &secp256k1::SecretKey::from_byte_array([17; 32]).unwrap(),
            now() + 5000,
            now(),
        )
        .unwrap();
    let context = e::TrainingContext {
        agent: "paul".into(),
        anchor: store.sales_agent_anchor(&owner, "paul").unwrap(),
        persona_sha256: "d".repeat(64),
        run: "synthetic-run".into(),
        synthetic_source_sha256: "e".repeat(64),
    };
    let paid = source(e::Kind::Training);
    let p = policy(vec![paid.clone()], 1_000, 1_000, 1_000);
    store
        .publish_sales_model_policy(&owner, &p, &p.sha256().unwrap())
        .unwrap();
    assert!(store.state.leads.is_empty());
    let admission = store
        .reserve_sales_training(&owner, &context, &input("practice", paid.clone()))
        .unwrap();
    let r = admission.receipt().clone();
    assert!(r.lead.is_empty());
    assert!(r.assignment.is_empty());
    assert_eq!(r.training.as_ref(), Some(&context));
    assert_eq!(r.maximum_usd_millionths, 1_000);
    assert!(
        store
            .reserve_sales_training(&owner, &context, &input("practice-again", paid))
            .is_err()
    );
    drop(admission);
    assert_eq!(
        store.sales_model_reservation(&owner, &r.id).unwrap().status,
        e::Status::Unknown
    );
    assert!(store.state.leads.is_empty());
}
