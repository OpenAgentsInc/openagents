use super::*;
use crate::task::sales::{expenses, training};

struct Script {
    source: expenses::Source,
    inputs: Vec<training::TurnInput>,
    fail_after: Option<usize>,
    oversized: bool,
}
impl training::Model for Script {
    fn source(&self) -> &expenses::Source {
        &self.source
    }
    fn turn(&mut self, input: &training::TurnInput, caps: &expenses::Source) -> Result<String> {
        assert_eq!(caps, &self.source);
        self.inputs.push(input.clone());
        if self.fail_after.is_some_and(|n| self.inputs.len() > n) {
            return Err("fixture adapter failed; provider response must not be retained".into());
        }
        if self.oversized {
            return Ok("x".repeat(caps.max_output_tokens as usize + 1));
        }
        Ok(match input.role {
            training::Speaker::Buyer => "Synthetic buyer asks about the reviewed offer and its limits.".into(),
            training::Speaker::Student => "Reviewed limits apply; no contact permission, revenue, or external send is established.".into(),
        })
    }
}
fn prepared(
    situation: training::Situation,
    max_turns: usize,
) -> (Fixture, training::Schedule, Script) {
    let (mut f, claims) = super::helpers::prepared();
    let source = expenses::Source {
        basis: expenses::Basis::LocalDeterministic,
        kind: expenses::Kind::Training,
        source_revision: digest(b"synthetic-script-model-v1"),
        price_revision: digest(b"zero-local-script-cost"),
        recipient: "human:operator".into(),
        max_input_bytes: 65536,
        max_output_tokens: 1024,
        max_attempts: 1,
        max_elapsed_secs: 30,
        input_usd_millionths_per_million: 0,
        output_usd_millionths_per_million: 0,
    };
    let policy = expenses::Policy {
        schema: "openagents.sales-model-policy.v1".into(),
        revision: 2,
        floor_daily_usd_millionths: 5000000,
        agent_daily_usd_millionths: 1000000,
        request_usd_millionths: 100000,
        sources: vec![source.clone()],
    };
    f.store
        .publish_sales_model_policy(&f.owner, &policy, &policy.sha256().unwrap())
        .unwrap();
    let schedule = training::Schedule {
        id: "synthetic-run".into(),
        agent: f.anchor.clone(),
        playbook: f.policy.playbook.clone(),
        situation,
        release: claims.release,
        claims: claims.claims,
        source: source.clone(),
        bounds: training::Bounds {
            max_turns,
            max_output_bytes: 1024,
            max_total_output_bytes: 4096,
            max_elapsed_secs: 60,
        },
    };
    f.store
        .schedule_sales_roleplay(&f.owner, &schedule)
        .unwrap();
    (
        f,
        schedule,
        Script {
            source,
            inputs: vec![],
            fail_after: None,
            oversized: false,
        },
    )
}
fn run(f: &mut Fixture, script: &mut Script) -> training::Run {
    let temporary =
        Store::open_with_clock(&f.dir.path().join("unused-training-root"), now).unwrap();
    let store = std::mem::replace(&mut f.store, temporary);
    let clock = store.clock;
    let run = store
        .run_sales_roleplay(&f.owner, "synthetic-run", script)
        .unwrap();
    f.store = Store::open_with_clock(&f.dir.path().join("host"), clock).unwrap();
    run
}
#[test]
fn all_synthetic_personas_replay_bounded_practice_without_new_customers_or_certification() {
    for persona in training::personas() {
        let (mut f, schedule, mut script) = prepared(persona.situation, 6);
        let before = f.store.state.leads.len();
        let result = run(&mut f, &mut script);
        assert_eq!(result.persona_sha256, persona.sha256().unwrap());
        assert_eq!(result.label, training::LABEL);
        assert!(!result.outbound_authority);
        assert_eq!(
            result.stop,
            if persona.situation == training::Situation::OptOut {
                training::Stop::OptOut
            } else {
                training::Stop::Completed
            }
        );
        assert!(result.turns.len() <= 4);
        assert_eq!(result.turns.len(), result.attempts.len());
        assert_eq!(f.store.state.leads.len(), before);
        assert!(f.store.state.agents.certificates.is_empty());
        for input in &script.inputs {
            assert_eq!(input.label, training::LABEL);
            assert_eq!(
                input.role == training::Speaker::Buyer,
                input.buyer_situation.is_some()
            );
            assert!(
                !serde_json::to_string(input)
                    .unwrap()
                    .contains("private-buyer")
            );
        }
        for turn in &result.turns {
            assert_eq!(turn.label, training::LABEL);
            let expense = f
                .store
                .sales_model_reservation(&f.owner, &turn.expense_reference)
                .unwrap();
            assert_eq!(expense.status, expenses::Status::Known);
            assert!(!expense.execution_unknown);
            assert_eq!(
                expense.training.unwrap().persona_sha256,
                result.persona_sha256
            );
            assert!(expense.lead.is_empty());
            assert!(expense.assignment.is_empty());
        }
        let repeated = f
            .store
            .schedule_sales_roleplay(&f.owner, &schedule)
            .unwrap();
        assert_eq!(repeated.sha256().unwrap(), result.sha256().unwrap());
        let temporary =
            Store::open_with_clock(&f.dir.path().join("second-unused-root"), now).unwrap();
        let store = std::mem::replace(&mut f.store, temporary);
        assert!(
            store
                .run_sales_roleplay(&f.owner, "synthetic-run", &mut script)
                .is_err()
        );
    }
}
#[test]
fn failure_and_output_limit_keep_partial_evidence_and_original_unknown_attempt() {
    for oversized in [false, true] {
        let (mut f, _, mut script) = prepared(training::Situation::PressureTrap, 6);
        script.oversized = oversized;
        script.fail_after = Some(1);
        let result = run(&mut f, &mut script);
        assert_eq!(result.stop, training::Stop::ModelFailure);
        assert_eq!(result.turns.len(), usize::from(!oversized));
        assert_eq!(result.attempts.len(), result.turns.len() + 1);
        let original = f
            .store
            .sales_model_reservation(&f.owner, result.attempts.last().unwrap())
            .unwrap();
        assert!(original.execution_unknown);
        assert!(
            !serde_json::to_string(&result)
                .unwrap()
                .contains("provider response")
        );
    }
}
#[test]
fn turn_cap_stale_evidence_and_native_source_changes_never_restart_practice() {
    let (mut f, _, mut script) = prepared(training::Situation::FullPrice, 2);
    assert_eq!(run(&mut f, &mut script).stop, training::Stop::TurnLimit);
    let (mut f, _, mut script) = prepared(training::Situation::UnsupportedFeature, 6);
    std::fs::write(f.dir.path().join("contract"), b"changed reviewed evidence").unwrap();
    assert_eq!(
        run(&mut f, &mut script).stop,
        training::Stop::EvidenceUnavailable
    );
    assert!(script.inputs.is_empty());
    assert!(
        f.store.state.training.runs["synthetic-run"]
            .attempts
            .is_empty()
    );
}

#[test]
fn priced_training_budget_refusal_preserves_schedule_without_calling_model() {
    let (mut f, _, mut script) = prepared(training::Situation::FullPrice, 6);
    script.source.basis = expenses::Basis::ListPrice;
    script.source.input_usd_millionths_per_million = 1_000_000_000;
    script.source.output_usd_millionths_per_million = 1_000_000_000;
    let policy = expenses::Policy {
        schema: "openagents.sales-model-policy.v1".into(),
        revision: 3,
        floor_daily_usd_millionths: 5_000_000,
        agent_daily_usd_millionths: 1_000_000,
        request_usd_millionths: 100_000,
        sources: vec![script.source.clone()],
    };
    f.store
        .publish_sales_model_policy(&f.owner, &policy, &policy.sha256().unwrap())
        .unwrap();
    let mut schedule = f
        .store
        .sales_roleplay(&f.owner, "synthetic-run")
        .unwrap()
        .schedule;
    schedule.id = "priced-practice".into();
    schedule.source = script.source.clone();
    f.store
        .schedule_sales_roleplay(&f.owner, &schedule)
        .unwrap();
    let temporary = Store::open_with_clock(&f.dir.path().join("budget-unused-root"), now).unwrap();
    let store = std::mem::replace(&mut f.store, temporary);
    let result = store
        .run_sales_roleplay(&f.owner, "priced-practice", &mut script)
        .unwrap();
    assert_eq!(result.stop, training::Stop::BudgetExhausted);
    assert!(result.attempts.is_empty());
    assert!(result.turns.is_empty());
    assert!(script.inputs.is_empty());
}

static TRAINING_CLOCK: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
fn training_clock() -> u64 {
    TRAINING_CLOCK.load(std::sync::atomic::Ordering::SeqCst)
}
struct SlowScript(Script);
impl training::Model for SlowScript {
    fn source(&self) -> &expenses::Source {
        &self.0.source
    }
    fn turn(&mut self, input: &training::TurnInput, caps: &expenses::Source) -> Result<String> {
        let result = self.0.turn(input, caps)?;
        TRAINING_CLOCK.store(now() + 61, std::sync::atomic::Ordering::SeqCst);
        Ok(result)
    }
}
#[test]
fn wall_clock_deadline_retains_original_unknown_attempt_and_no_successful_turn() {
    let (mut f, _, script) = prepared(training::Situation::PressureTrap, 6);
    TRAINING_CLOCK.store(now(), std::sync::atomic::Ordering::SeqCst);
    f.store.clock = training_clock;
    let temporary =
        Store::open_with_clock(&f.dir.path().join("deadline-unused-root"), now).unwrap();
    let store = std::mem::replace(&mut f.store, temporary);
    let result = store
        .run_sales_roleplay(&f.owner, "synthetic-run", &mut SlowScript(script))
        .unwrap();
    assert_eq!(result.stop, training::Stop::Deadline);
    assert!(result.turns.is_empty());
    assert_eq!(result.attempts.len(), 1);
    let mut store = Store::open_with_clock(&f.dir.path().join("host"), training_clock).unwrap();
    let expense = store
        .sales_model_reservation(&f.owner, &result.attempts[0])
        .unwrap();
    assert!(expense.execution_unknown);
}

#[test]
fn fresh_private_training_never_creates_a_customer_assignment_or_contact_grant() {
    let (mut f, claims) = super::helpers::prepared_with(Fixture::without_customers());
    let mut model = training::Scripted::new("human:operator");
    let source = training::Model::source(&model).clone();
    let policy = expenses::Policy {
        schema: "openagents.sales-model-policy.v1".into(),
        revision: 2,
        floor_daily_usd_millionths: 5_000_000,
        agent_daily_usd_millionths: 1_000_000,
        request_usd_millionths: 100_000,
        sources: vec![source.clone()],
    };
    f.store
        .publish_sales_model_policy(&f.owner, &policy, &policy.sha256().unwrap())
        .unwrap();
    let schedule = training::Schedule {
        id: "synthetic-run".into(),
        agent: f.anchor.clone(),
        playbook: f.policy.playbook.clone(),
        situation: training::Situation::AmbiguousConsent,
        release: claims.release,
        claims: claims.claims,
        source,
        bounds: training::Bounds {
            max_turns: 6,
            max_output_bytes: 4096,
            max_total_output_bytes: 16384,
            max_elapsed_secs: 60,
        },
    };
    f.store
        .schedule_sales_roleplay(&f.owner, &schedule)
        .unwrap();
    assert!(f.store.state.leads.is_empty());
    assert!(f.store.state.agents.policies.is_empty());
    assert!(f.store.state.agents.certificates.is_empty());
    let temporary = Store::open_with_clock(&f.dir.path().join("blank-unused"), now).unwrap();
    let store = std::mem::replace(&mut f.store, temporary);
    let result = store
        .run_sales_roleplay(&f.owner, "synthetic-run", &mut model)
        .unwrap();
    assert_eq!(result.stop, training::Stop::Completed);
    assert!(!result.outbound_authority);
    f.store = Store::open_with_clock(&f.dir.path().join("host"), now).unwrap();
    assert!(f.store.state.leads.is_empty());
    assert!(f.store.state.agents.policies.is_empty());
    assert!(f.store.state.agents.certificates.is_empty());
    for turn in result.turns {
        let expense = f
            .store
            .sales_model_reservation(&f.owner, &turn.expense_reference)
            .unwrap();
        assert!(expense.lead.is_empty());
        assert!(expense.assignment.is_empty());
        assert_eq!(expense.status, expenses::Status::Known);
        assert!(!expense.execution_unknown);
        assert_eq!(expense.native, f.anchor);
    }
}

#[test]
fn training_crash_child() {
    let Some(root) = std::env::var_os("OA_TRAINING_CRASH_ROOT") else {
        return;
    };
    let credential = std::env::var_os("OA_TRAINING_CRASH_CREDENTIAL").unwrap();
    let ready = PathBuf::from(std::env::var_os("OA_TRAINING_CRASH_READY").unwrap());
    let mut store = Store::open_with_clock(Path::new(&root), now).unwrap();
    let owner = store
        .authenticate(&Store::read_credential(Path::new(&credential)).unwrap())
        .unwrap();
    let source = store
        .sales_roleplay(&owner, "killed-practice")
        .unwrap()
        .schedule
        .source;
    struct Blocking {
        source: expenses::Source,
        ready: PathBuf,
        calls: usize,
    }
    impl training::Model for Blocking {
        fn source(&self) -> &expenses::Source {
            &self.source
        }
        fn turn(&mut self, _: &training::TurnInput, _: &expenses::Source) -> Result<String> {
            self.calls += 1;
            if self.calls == 1 {
                return Ok("Synthetic buyer asks for the reviewed limits.".into());
            }
            let pending = self.ready.with_extension("writing");
            std::fs::write(&pending, b"second original attempt is running").unwrap();
            std::fs::rename(pending, &self.ready).unwrap();
            loop {
                std::thread::sleep(std::time::Duration::from_millis(100));
            }
        }
    }
    let mut model = Blocking {
        source,
        ready,
        calls: 0,
    };
    store
        .run_sales_roleplay(&owner, "killed-practice", &mut model)
        .unwrap();
}

#[test]
fn killed_training_process_retains_partial_turns_and_original_unknown_liability() {
    let (mut f, _, mut model) = prepared(training::Situation::PressureTrap, 6);
    model.source.basis = expenses::Basis::ListPrice;
    model.source.input_usd_millionths_per_million = 1_000_000;
    model.source.output_usd_millionths_per_million = 1_000_000;
    let policy = expenses::Policy {
        schema: "openagents.sales-model-policy.v1".into(),
        revision: 3,
        floor_daily_usd_millionths: 5_000_000,
        agent_daily_usd_millionths: 1_000_000,
        request_usd_millionths: 100_000,
        sources: vec![model.source.clone()],
    };
    f.store
        .publish_sales_model_policy(&f.owner, &policy, &policy.sha256().unwrap())
        .unwrap();
    let mut schedule = f
        .store
        .sales_roleplay(&f.owner, "synthetic-run")
        .unwrap()
        .schedule;
    schedule.id = "killed-practice".into();
    schedule.source = model.source.clone();
    f.store
        .schedule_sales_roleplay(&f.owner, &schedule)
        .unwrap();
    let root = f.dir.path().join("host");
    let credential = f.dir.path().join("owner");
    let ready = f.dir.path().join("training-crash-ready");
    drop(f.store);
    let mut child = std::process::Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "task::sales::agents::tests::training::training_crash_child",
            "--nocapture",
        ])
        .env("OA_TRAINING_CRASH_ROOT", &root)
        .env("OA_TRAINING_CRASH_CREDENTIAL", &credential)
        .env("OA_TRAINING_CRASH_READY", &ready)
        .stdout(std::process::Stdio::null())
        .spawn()
        .unwrap();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(15);
    while !ready.exists() && std::time::Instant::now() < deadline {
        if child.try_wait().unwrap().is_some() {
            panic!("synthetic child exited before its second attempt");
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    if !ready.exists() {
        child.kill().unwrap();
        child.wait().unwrap();
        panic!("synthetic second attempt timed out");
    }
    child.kill().unwrap();
    assert!(!child.wait().unwrap().success());
    f.store = Store::open_with_clock(&root, now).unwrap();
    let result = f.store.sales_roleplay(&f.owner, "killed-practice").unwrap();
    assert_eq!(result.stop, training::Stop::Interrupted);
    assert_eq!(result.turns.len(), 1);
    assert_eq!(result.attempts.len(), 2);
    assert!(!result.outbound_authority);
    let original = f
        .store
        .sales_model_reservation(&f.owner, result.attempts.last().unwrap())
        .unwrap();
    assert_eq!(original.status, expenses::Status::Unknown);
    assert!(original.execution_unknown);
    assert!(original.maximum_usd_millionths > 0);
    let temporary = Store::open_with_clock(&f.dir.path().join("killed-unused-root"), now).unwrap();
    let store = std::mem::replace(&mut f.store, temporary);
    assert!(
        store
            .run_sales_roleplay(&f.owner, "killed-practice", &mut model)
            .is_err()
    );
    assert!(model.inputs.is_empty());
}
