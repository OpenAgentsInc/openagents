//! Saved customer environments (ENV-10) on the Pro subscription, with fake
//! billing and a fake credits ledger: the monthly allowance, extra hours
//! only when turned on and never past the cap, storage limits, refusals,
//! and restart recovery.
use std::collections::BTreeMap;

use retail_cloud::Error;
use retail_cloud::authority::Source;
use retail_cloud::environment::{
    self, Credits, Ending, EnvironmentPlan, EnvironmentRequest, ExtraHours, Gate, Period, Phase,
    PlanStatus, Refusal, Saved, Standing, Usage,
};
use retail_cloud::journal::Journal;

const NOW: i64 = 1_791_200_000;
const DAY: i64 = 86_400;
const HOUR: u64 = 3600;
const END: i64 = NOW + 30 * DAY;

fn plan() -> EnvironmentPlan {
    let mut p = environment::plan();
    p.status = PlanStatus::Published;
    p
}
fn gate(p: &EnvironmentPlan) -> Gate {
    Gate {
        contract_reviewed: true,
        plan: Some(p.digest()),
        qualification: Some("funded-qualification-receipt".into()),
    }
}
fn request() -> EnvironmentRequest {
    EnvironmentRequest {
        source: Source {
            repository: "https://github.com/OpenAgentsInc/example".into(),
            commit: "c".repeat(40),
        },
        objective: "Build and test the Rust workspace.".into(),
        profile: "rust-library".into(),
        checks: vec!["cargo test -p parser".into()],
        max_seconds: None,
    }
}
fn saved(n: u32) -> Saved {
    Saved {
        environment: "env-1".into(),
        version: format!("env-1-v{n}"),
        image_id: format!("oaenv-build-{n}"),
    }
}
fn refusal(e: Error) -> Refusal {
    match e {
        Error::Environment(r) => r,
        other => panic!("expected an environment refusal, got {other:?}"),
    }
}
fn subscribed(j: &mut Journal, account: &str) {
    environment::record_period(
        j,
        &Period {
            account: account.into(),
            plan: environment::plan().version,
            start: NOW - DAY,
            end: END,
        },
    )
    .unwrap();
}

/// One setup from offer to settlement.
fn run(
    j: &mut Journal,
    id: &str,
    seconds: u64,
    ending: Ending,
    image_gb: Option<u64>,
) -> environment::Receipt {
    let p = plan();
    let g = gate(&p);
    let o = environment::offer(j, &p, &g, "acct", id, &request(), NOW).unwrap();
    environment::confirm(j, &p, &g, "acct", id, &o.digest, true, NOW).unwrap();
    let saved = (ending == Ending::Saved).then(|| saved(id.len() as u32));
    environment::end(
        j,
        &p,
        id,
        ending,
        Some(Usage {
            machine_seconds: seconds,
            image_gb,
        }),
        saved,
        NOW + 1,
    )
    .unwrap();
    environment::settle(j, id, NOW + 2).unwrap()
}

#[derive(Default)]
struct FakeCredits {
    charged: BTreeMap<String, u64>,
    down: bool,
}
impl Credits for FakeCredits {
    fn debit(&mut self, _: &str, key: &str, usd: u64, _: i64) -> Result<(), String> {
        if self.down {
            return Err("unreachable".into());
        }
        self.charged.entry(key.into()).or_insert(usd);
        Ok(())
    }
}

#[test]
fn nothing_sells_until_the_owner_opens_it_and_needs_a_subscription() {
    let mut j = Journal::in_memory().unwrap();
    subscribed(&mut j, "acct");
    let proposed = environment::plan();
    let closed = Gate {
        contract_reviewed: true,
        plan: Some(proposed.digest()),
        qualification: Some("x".into()),
    };
    let e =
        environment::offer(&mut j, &proposed, &closed, "acct", "p1", &request(), NOW).unwrap_err();
    assert_eq!(refusal(e), Refusal::Closed);
    let p = plan();
    let e = environment::offer(&mut j, &p, &gate(&p), "other", "p1", &request(), NOW).unwrap_err();
    let r = refusal(e);
    assert_eq!(r, Refusal::NoPlan);
    assert_eq!(r.message(), "Saved environments come with the Pro plan.");
    assert!(environment::purchases(&j).unwrap().is_empty());
}

#[test]
fn hours_count_against_the_month_and_there_is_no_run_limit_of_ours() {
    let mut j = Journal::in_memory().unwrap();
    subscribed(&mut j, "acct");
    let p = plan();
    // A ten-hour setup: no limit of ours stops it, and it fits the month.
    let r = run(&mut j, "p1", 10 * HOUR, Ending::Saved, Some(6));
    assert_eq!(
        (r.included_seconds, r.extra_seconds, r.extra_usd_micros),
        (10 * HOUR, 0, 0)
    );
    let b = environment::budget(&j, &p, "acct", NOW + 3).unwrap();
    assert_eq!(b.included_left_seconds, 90 * HOUR);
    assert_eq!(b.extra_left_seconds, 0, "extra hours are off by default");
    assert_eq!(b.resets_at, END);
    // Settling again counts nothing twice.
    environment::settle(&mut j, "p1", NOW + 9).unwrap();
    assert_eq!(environment::budget(&j, &p, "acct", NOW + 9).unwrap(), b);
    // The saved image counts toward storage and is selectable.
    assert_eq!(environment::storage(&j, "acct").unwrap(), (6, 1));
    let purchase = environment::purchase(&j, "acct", "p1").unwrap().unwrap();
    assert!(environment::may_select(&j, &purchase, "acct", NOW + 3).unwrap());
    assert!(!environment::may_select(&j, &purchase, "other", NOW + 3).unwrap());
    // Another account cannot read it.
    assert_eq!(
        refusal(environment::purchase(&j, "other", "p1").unwrap_err()),
        Refusal::NotYours
    );
    let s = environment::summary(&j, &p, "acct", NOW + 3).unwrap();
    assert_eq!(
        (s.used_seconds, s.included_seconds),
        (10 * HOUR, 100 * HOUR)
    );
    assert!(matches!(s.standing, Standing::Active { .. }));
}

#[test]
fn a_used_up_month_refuses_with_a_plain_message_unless_extra_hours_are_on() {
    let mut j = Journal::in_memory().unwrap();
    subscribed(&mut j, "acct");
    let p = plan();
    // 120 hours with extra hours off: 100 counted, nothing charged.
    let r = run(&mut j, "p1", 120 * HOUR, Ending::Ended, None);
    assert_eq!(
        (r.included_seconds, r.extra_seconds, r.extra_usd_micros),
        (100 * HOUR, 20 * HOUR, 0)
    );
    assert!(environment::debits(&j).unwrap().is_empty());
    let e = environment::offer(&mut j, &p, &gate(&p), "acct", "p2", &request(), NOW).unwrap_err();
    let r = refusal(e);
    assert_eq!(
        r,
        Refusal::AllowanceUsed {
            hours: 100,
            resets_at: END
        }
    );
    assert!(r.message().starts_with(
        "You've used this month's 100 hours. Turn on extra hours in Settings, or wait until "
    ));
    // The person turns extra hours on with a $1 cap: five hours fit.
    environment::set_extra_hours(
        &mut j,
        "acct",
        ExtraHours {
            enabled: true,
            cap_usd_micros: 1_000_000,
        },
    )
    .unwrap();
    let b = environment::budget(&j, &p, "acct", NOW).unwrap();
    assert_eq!(b.extra_left_seconds, 1_000_000 * HOUR / 180_000);
    let r = run(&mut j, "p2", 2 * HOUR, Ending::Ended, None);
    assert_eq!((r.extra_seconds, r.extra_usd_micros), (2 * HOUR, 360_000));
    // A run past the cap is charged only up to the cap.
    let r = run(&mut j, "p3", 10 * HOUR, Ending::Ended, None);
    assert_eq!(r.extra_usd_micros, 640_000);
    let e = environment::offer(&mut j, &p, &gate(&p), "acct", "p4", &request(), NOW).unwrap_err();
    let r = refusal(e);
    assert_eq!(
        r,
        Refusal::CapReached {
            cap_usd_micros: 1_000_000,
            resets_at: END
        }
    );
    // Charges leave once, through the credits ledger; a failure retries.
    let mut credits = FakeCredits {
        down: true,
        ..FakeCredits::default()
    };
    assert!(
        environment::post_debits(&mut j, &mut credits, NOW)
            .unwrap()
            .is_empty()
    );
    credits.down = false;
    assert_eq!(
        environment::post_debits(&mut j, &mut credits, NOW)
            .unwrap()
            .len(),
        2
    );
    assert!(
        environment::post_debits(&mut j, &mut credits, NOW)
            .unwrap()
            .is_empty()
    );
    assert_eq!(credits.charged.values().sum::<u64>(), 1_000_000);
    // Unused hours don't roll over: a new month starts from its own 100.
    environment::record_period(
        &mut j,
        &Period {
            account: "acct".into(),
            plan: p.version.clone(),
            start: END,
            end: END + 30 * DAY,
        },
    )
    .unwrap();
    let b = environment::budget(&j, &p, "acct", END + 1).unwrap();
    assert_eq!(b.included_left_seconds, 100 * HOUR);
}

#[test]
fn storage_and_machines_have_plain_limits() {
    let mut j = Journal::in_memory().unwrap();
    subscribed(&mut j, "acct");
    let p = plan();
    let g = gate(&p);
    run(&mut j, "a1", HOUR, Ending::Saved, Some(15));
    // 15 + 6 GB is past 20.
    let o = environment::offer(&mut j, &p, &g, "acct", "a2", &request(), NOW).unwrap();
    environment::confirm(&mut j, &p, &g, "acct", "a2", &o.digest, true, NOW).unwrap();
    // While one setup runs, a second waits.
    let o3 = environment::offer(&mut j, &p, &g, "acct", "a3", &request(), NOW).unwrap();
    let e = environment::confirm(&mut j, &p, &g, "acct", "a3", &o3.digest, true, NOW).unwrap_err();
    assert_eq!(refusal(e), Refusal::MachinesBusy { machines: 2 });
    let usage = Some(Usage {
        machine_seconds: HOUR,
        image_gb: Some(6),
    });
    let e =
        environment::end(&mut j, &p, "a2", Ending::Saved, usage, Some(saved(2)), NOW).unwrap_err();
    let r = refusal(e);
    assert_eq!(r, Refusal::StorageFull { gb: 20 });
    assert_eq!(
        r.message(),
        "Your saved environments would use more than 20 GB. Delete one to save this one."
    );
    // Deleting the first frees the space.
    let due = environment::delete(&mut j, "acct", "a1", NOW).unwrap();
    assert_eq!(due.saved, saved(2));
    environment::end(&mut j, &p, "a2", Ending::Saved, usage, Some(saved(2)), NOW).unwrap();
    environment::settle(&mut j, "a2", NOW).unwrap();
    assert_eq!(environment::storage(&j, "acct").unwrap(), (6, 1));
    // A reader cannot start paid work.
    let e = environment::confirm(&mut j, &p, &g, "acct", "a3", &o3.digest, false, NOW).unwrap_err();
    assert_eq!(refusal(e), Refusal::NoSpendRight);
    // A changed plan refuses the old offer.
    let mut moved = p.clone();
    moved.included_machine_hours = 50;
    let e = environment::confirm(
        &mut j,
        &moved,
        &gate(&moved),
        "acct",
        "a3",
        &o3.digest,
        true,
        NOW,
    )
    .unwrap_err();
    assert_eq!(refusal(e), Refusal::Changed);
}

#[test]
fn unknown_endings_wait_and_a_restart_finishes_each_step_once() {
    let mut j = Journal::in_memory().unwrap();
    subscribed(&mut j, "acct");
    let p = plan();
    let g = gate(&p);
    let o = environment::offer(&mut j, &p, &g, "acct", "u1", &request(), NOW).unwrap();
    environment::confirm(&mut j, &p, &g, "acct", "u1", &o.digest, true, NOW).unwrap();
    environment::end(&mut j, &p, "u1", Ending::Unknown, None, None, NOW).unwrap();
    let rec = environment::recover(&mut j, NOW + 1).unwrap();
    assert_eq!(rec.held, vec!["u1".to_string()]);
    let usage = Some(Usage {
        machine_seconds: 3 * HOUR,
        image_gb: Some(4),
    });
    environment::end(
        &mut j,
        &p,
        "u1",
        Ending::Saved,
        usage,
        Some(saved(1)),
        NOW + 2,
    )
    .unwrap();
    // A known ending never changes.
    assert!(matches!(
        environment::end(&mut j, &p, "u1", Ending::Ended, usage, None, NOW + 3),
        Err(Error::Conflict(_))
    ));
    let rec = environment::recover(&mut j, NOW + 3).unwrap();
    assert_eq!(rec.settled, vec!["u1".to_string()]);
    let again = environment::recover(&mut j, NOW + 4).unwrap();
    assert!(again.settled.is_empty());
    let b = environment::budget(&j, &p, "acct", NOW + 4).unwrap();
    assert_eq!(b.included_left_seconds, 97 * HOUR);
    // No machine means nothing counted.
    let o = environment::offer(&mut j, &p, &g, "acct", "u2", &request(), NOW).unwrap();
    environment::confirm(&mut j, &p, &g, "acct", "u2", &o.digest, true, NOW).unwrap();
    environment::end(
        &mut j,
        &p,
        "u2",
        Ending::ProviderUnavailable,
        None,
        None,
        NOW,
    )
    .unwrap();
    let r = environment::settle(&mut j, "u2", NOW).unwrap();
    assert_eq!(r.included_seconds, 0);
    // After the subscription ends the image stays 30 days, then retires.
    let purchase = environment::purchase(&j, "acct", "u1").unwrap().unwrap();
    assert!(!environment::may_select(&j, &purchase, "acct", END + 1).unwrap());
    assert!(
        environment::recover(&mut j, END + 29 * DAY)
            .unwrap()
            .retired
            .is_empty()
    );
    let rec = environment::recover(&mut j, END + 30 * DAY).unwrap();
    assert_eq!(rec.retired.len(), 1);
    assert!(matches!(
        environment::purchase(&j, "acct", "u1")
            .unwrap()
            .unwrap()
            .phase,
        Phase::Settled { .. }
    ));
}
