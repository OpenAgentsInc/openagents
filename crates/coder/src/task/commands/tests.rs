use super::*;
use crate::task::adapter::STEERING;
use crate::task::{RequestedConfiguration, TaskIntent, Workspace};

const NOW: u64 = 1_790_000_000;

fn request(command: &str, kind: Kind, based_on: u64, text: &str) -> Request {
    Request {
        command: command.repeat(64 / command.len()),
        task: "task".into(),
        kind,
        based_on,
        text: text.into(),
        emulate: false,
        issued_at: NOW,
    }
}

fn entry(request: Request) -> Entry {
    Entry {
        sender: sender("phone"),
        request,
        received_at: NOW,
        state: State::Received,
    }
}

fn sender(device: &str) -> Sender {
    Sender {
        device: device.into(),
        grant: Some("g".repeat(64)),
        epoch: Some(1),
    }
}

fn view(phase: Phase, revision: u64, turn_started: u64) -> View {
    View {
        phase,
        revision,
        turn_started,
    }
}

fn decided(entry: &Entry, view: &View) -> Decision {
    decide(entry, view, &STEERING, false, false, true, NOW)
}

#[test]
fn an_interrupt_supersedes_only_older_interrupts_and_never_a_past_turn() {
    let running = view(Phase::Running, 4, 3);
    let stop = entry(request("a", Kind::Interrupt, 4, "Stop"));
    assert_eq!(
        decided(&stop, &running),
        Decision::Dispatch(Effect::Cancel("Stop".into()))
    );
    // A newer interrupt supersedes this one.
    assert_eq!(
        decide(&stop, &running, &STEERING, true, false, true, NOW),
        Decision::Done(Outcome::Superseded)
    );
    // An interrupt based on an earlier turn does not stop the current one.
    let old = entry(request("b", Kind::Interrupt, 2, "Stop"));
    assert_eq!(decided(&old, &running), Decision::Done(Outcome::Superseded));
    // Nothing runs: nothing to stop.
    assert_eq!(
        decided(&stop, &view(Phase::Ended, 4, 3)),
        Decision::Done(Outcome::Superseded)
    );
    // Queued messages and steers are never superseded by an interrupt.
    let queued = entry(request("c", Kind::Queue, 4, "Next"));
    assert_eq!(
        decide(&queued, &running, &STEERING, true, false, true, NOW),
        Decision::Hold { priority: false }
    );
}

#[test]
fn expired_and_revoked_commands_never_run() {
    let ended = view(Phase::Ended, 2, 1);
    let mut late = entry(request("a", Kind::Send, 2, "Hello"));
    late.request.issued_at = NOW - TTL - 1;
    assert_eq!(decided(&late, &ended), Decision::Done(Outcome::Expired));
    let mut future = entry(request("b", Kind::Send, 2, "Hello"));
    future.request.issued_at = NOW + SKEW + 1;
    assert_eq!(
        decided(&future, &ended),
        Decision::Done(Outcome::Rejected {
            reason: Rejection::Bounds
        })
    );
    let send = entry(request("c", Kind::Send, 2, "Hello"));
    assert_eq!(
        decide(&send, &ended, &STEERING, false, false, false, NOW),
        Decision::Done(Outcome::Rejected {
            reason: Rejection::Revoked
        })
    );
}

#[test]
fn a_send_continues_an_ended_task_and_conflicts_with_a_running_one() {
    let send = entry(request("a", Kind::Send, 2, "Also add a test."));
    assert_eq!(
        decided(&send, &view(Phase::Ended, 2, 1)),
        Decision::Dispatch(Effect::Continue("Also add a test.".into()))
    );
    for phase in [Phase::Pending, Phase::Running, Phase::Stopping] {
        assert_eq!(
            decided(&send, &view(phase, 2, 1)),
            Decision::Done(Outcome::Rejected {
                reason: Rejection::Conflict
            })
        );
    }
    // A queue ahead of it keeps a send from jumping it.
    assert_eq!(
        decide(
            &send,
            &view(Phase::Ended, 2, 1),
            &STEERING,
            false,
            true,
            true,
            NOW
        ),
        Decision::Done(Outcome::Rejected {
            reason: Rejection::Conflict
        })
    );
    assert_eq!(
        decided(&send, &view(Phase::Unknown, 2, 1)),
        Decision::Done(Outcome::Rejected {
            reason: Rejection::Unavailable
        })
    );
}

#[test]
fn a_steer_follows_the_engines_stated_semantics() {
    let running = view(Phase::Running, 2, 1);
    let native = entry(request("a", Kind::Steer, 2, "Only the parser."));
    // Microcoder cannot steer a running turn natively.
    assert_eq!(
        decided(&native, &running),
        Decision::Done(Outcome::Rejected {
            reason: Rejection::Unsupported
        })
    );
    let mut emulated = native.clone();
    emulated.request.emulate = true;
    assert_eq!(
        decided(&emulated, &running),
        Decision::Dispatch(Effect::CancelThenContinue("Only the parser.".into()))
    );
    // A turn that has not started takes new instructions.
    assert_eq!(
        decided(&native, &view(Phase::Pending, 2, 1)),
        Decision::Dispatch(Effect::Correct("Only the parser.".into()))
    );
    // A steer whose turn ended becomes the next turn.
    assert_eq!(
        decided(&native, &view(Phase::Ended, 3, 1)),
        Decision::Dispatch(Effect::Continue("Only the parser.".into()))
    );
    let answer = entry(request("b", Kind::Answer, 2, "Yes"));
    assert_eq!(
        decided(&answer, &running),
        Decision::Done(Outcome::Rejected {
            reason: Rejection::Unsupported
        })
    );
}

fn ended_task(dir: &Path) {
    let mut store = Store::open(dir).unwrap();
    let submit = Command {
        schema: COMMAND_SCHEMA.into(),
        command_id: "submit".into(),
        task_id: "task".into(),
        expected_revision: None,
        action: Action::Submit {
            intent: TaskIntent {
                title: "Chat".into(),
                prompt: "Explain the parser.".into(),
                workspace: Workspace {
                    path: "/example/checkout".into(),
                    source_revision: None,
                },
                configuration: RequestedConfiguration {
                    adapter: "microcoder-repository".into(),
                    model: Some("gpt-6-luna".into()),
                },
            },
        },
    };
    store.apply(&serde_json::to_vec(&submit).unwrap()).unwrap();
    end(&mut store, "end-1", 1);
}

fn end(store: &mut Store, id: &str, revision: u64) {
    let cancel = Command {
        schema: COMMAND_SCHEMA.into(),
        command_id: id.into(),
        task_id: "task".into(),
        expected_revision: Some(revision),
        action: Action::Cancel {
            reason: "Ended".into(),
        },
    };
    store.apply(&serde_json::to_vec(&cancel).unwrap()).unwrap();
}

fn always(_: &Sender) -> bool {
    true
}

#[test]
fn follow_ups_continue_the_task_and_replays_never_run_twice() {
    let temp = tempfile::tempdir().unwrap();
    let dir = temp.path().join("tasks");
    ended_task(&dir);
    let send = request("a", Kind::Send, 2, "Now the lexer.");
    let (recorded, continued) =
        record(&dir, &sender("phone"), &send, &STEERING, &always, NOW).unwrap();
    assert_eq!(
        recorded.state,
        State::Done(Outcome::Applied { revision: 3 })
    );
    assert_eq!(
        continued,
        vec![Continued {
            task: "task".into(),
            turn: 3,
            device: "phone".into()
        }]
    );
    let task = recorded.task.unwrap();
    assert_eq!(task.status, Status::Queued);
    assert_eq!(task.effective_prompt(), "Now the lexer.");
    // A replay after a reconnect returns the recorded outcome and starts
    // nothing new.
    let (again, continued) =
        record(&dir, &sender("phone"), &send, &STEERING, &always, NOW + 60).unwrap();
    assert_eq!(again.state, recorded.state);
    assert!(continued.is_empty());
    assert_eq!(Store::open(&dir).unwrap().show("task").unwrap().revision, 3);
    // The same ID with other content is a conflict; another device's
    // identical ID is its own command.
    let mut changed = send.clone();
    changed.text = "Something else.".into();
    assert!(matches!(
        record(&dir, &sender("phone"), &changed, &STEERING, &always, NOW),
        Err(Error::Conflict)
    ));
    let (other, _) = record(&dir, &sender("tablet"), &send, &STEERING, &always, NOW).unwrap();
    assert_eq!(
        other.state,
        State::Done(Outcome::Rejected {
            reason: Rejection::Conflict
        })
    );
}

#[test]
fn queued_messages_run_in_order_as_each_turn_ends() {
    let temp = tempfile::tempdir().unwrap();
    let dir = temp.path().join("tasks");
    ended_task(&dir);
    let phone = sender("phone");
    record(
        &dir,
        &phone,
        &request("a", Kind::Send, 2, "First."),
        &STEERING,
        &always,
        NOW,
    )
    .unwrap();
    for (id, text) in [("b", "Second."), ("c", "Third.")] {
        let (recorded, _) = record(
            &dir,
            &phone,
            &request(id, Kind::Queue, 3, text),
            &STEERING,
            &always,
            NOW,
        )
        .unwrap();
        assert_eq!(recorded.state, State::Held { priority: false });
    }
    assert_eq!(open_tasks(&dir), vec!["task".to_owned()]);
    // Nothing moves while the turn has not ended.
    assert!(
        process(&dir, "task", &STEERING, &always, NOW)
            .unwrap()
            .is_empty()
    );
    end(&mut Store::open(&dir).unwrap(), "end-2", 3);
    let continued = process(&dir, "task", &STEERING, &always, NOW).unwrap();
    assert_eq!(continued.len(), 1);
    assert_eq!(
        Store::open(&dir)
            .unwrap()
            .show("task")
            .unwrap()
            .effective_prompt(),
        "Second."
    );
    end(&mut Store::open(&dir).unwrap(), "end-3", 5);
    process(&dir, "task", &STEERING, &always, NOW).unwrap();
    assert_eq!(
        Store::open(&dir)
            .unwrap()
            .show("task")
            .unwrap()
            .effective_prompt(),
        "Third."
    );
    assert!(open_tasks(&dir).is_empty());
}

#[test]
fn a_held_command_rechecks_its_sender_before_it_runs() {
    let temp = tempfile::tempdir().unwrap();
    let dir = temp.path().join("tasks");
    ended_task(&dir);
    let phone = sender("phone");
    record(
        &dir,
        &phone,
        &request("a", Kind::Send, 2, "First."),
        &STEERING,
        &always,
        NOW,
    )
    .unwrap();
    record(
        &dir,
        &phone,
        &request("b", Kind::Queue, 3, "Second."),
        &STEERING,
        &always,
        NOW,
    )
    .unwrap();
    end(&mut Store::open(&dir).unwrap(), "end-2", 3);
    // The device was revoked while its message waited.
    let revoked = |_: &Sender| false;
    assert!(
        process(&dir, "task", &STEERING, &revoked, NOW)
            .unwrap()
            .is_empty()
    );
    let entries = entries(&dir).unwrap();
    assert_eq!(
        entries[1].state,
        State::Done(Outcome::Rejected {
            reason: Rejection::Revoked
        })
    );
    assert_eq!(Store::open(&dir).unwrap().show("task").unwrap().revision, 4);
}

#[test]
fn an_emulated_steer_stops_the_turn_and_goes_before_queued_messages() {
    let temp = tempfile::tempdir().unwrap();
    let dir = temp.path().join("tasks");
    ended_task(&dir);
    let phone = sender("phone");
    record(
        &dir,
        &phone,
        &request("a", Kind::Send, 2, "First."),
        &STEERING,
        &always,
        NOW,
    )
    .unwrap();
    record(
        &dir,
        &phone,
        &request("b", Kind::Queue, 3, "Queued."),
        &STEERING,
        &always,
        NOW,
    )
    .unwrap();
    let mut steer = request("c", Kind::Steer, 3, "Steered.");
    steer.emulate = true;
    let (recorded, _) = record(&dir, &phone, &steer, &STEERING, &always, NOW).unwrap();
    // The queued turn had not started, so the steer corrected it in place.
    assert_eq!(
        recorded.state,
        State::Done(Outcome::Applied { revision: 4 })
    );
    let task = Store::open(&dir).unwrap().show("task").unwrap();
    assert_eq!(task.effective_prompt(), "Steered.");
    assert_eq!(task.unconsumed_steers().len(), 1);
}

#[test]
fn a_recorded_dispatch_is_applied_again_byte_for_byte_after_a_crash() {
    let temp = tempfile::tempdir().unwrap();
    let dir = temp.path().join("tasks");
    ended_task(&dir);
    let phone = sender("phone");
    let send = request("a", Kind::Send, 2, "Continue.");
    let (recorded, _) = record(&dir, &phone, &send, &STEERING, &always, NOW).unwrap();
    assert_eq!(
        recorded.state,
        State::Done(Outcome::Applied { revision: 3 })
    );
    // Put the entry back as a crash would leave it: marked for dispatch,
    // with the effect already applied.
    let mut journal = read(&dir).unwrap();
    let store_command = Command {
        schema: COMMAND_SCHEMA.into(),
        command_id: store_command_id(&journal.entries[0], ""),
        task_id: "task".into(),
        expected_revision: Some(2),
        action: Action::Continue {
            prompt: "Continue.".into(),
        },
    };
    journal.entries[0].state = State::Dispatching {
        bytes: serde_json::to_string(&store_command).unwrap(),
        then_hold: false,
    };
    write(&dir, &journal).unwrap();
    process(&dir, "task", &STEERING, &always, NOW).unwrap();
    let task = Store::open(&dir).unwrap().show("task").unwrap();
    assert_eq!(task.revision, 3, "the follow-up ran once");
    assert_eq!(task.follow_ups.len(), 1);
    assert_eq!(
        entries(&dir).unwrap()[0].state,
        State::Done(Outcome::Applied { revision: 3 })
    );
}
