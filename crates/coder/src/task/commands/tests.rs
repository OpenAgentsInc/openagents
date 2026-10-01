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
        edited: None,
        promoted: false,
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
        question: None,
        paused: false,
        archived: false,
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
    // No question waits while a turn runs.
    let answer = entry(request("b", Kind::Answer, 2, "Yes"));
    assert_eq!(
        decided(&answer, &running),
        Decision::Done(Outcome::Rejected {
            reason: Rejection::Conflict
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
                    path: if cfg!(windows) {
                        r"C:\example\checkout"
                    } else {
                        "/example/checkout"
                    }
                    .into(),
                    source_revision: None,
                },
                configuration: RequestedConfiguration {
                    adapter: "microcoder-repository".into(),
                    model: Some("gpt-6-luna".into()),
                },
                images: Vec::new(),
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

#[test]
fn an_answer_continues_only_a_turn_that_asked_and_the_first_one_wins() {
    let answer = entry(request("a", Kind::Answer, 3, "Use the second option."));
    let asked = View {
        question: Some(crate::task::interaction::Kind::Question),
        ..view(Phase::Ended, 3, 2)
    };
    assert_eq!(
        decided(&answer, &asked),
        Decision::Dispatch(Effect::Continue("Use the second option.".into()))
    );
    // A turn that asked nothing has no question to answer.
    assert_eq!(
        decided(&answer, &view(Phase::Ended, 3, 2)),
        Decision::Done(Outcome::Rejected {
            reason: Rejection::Conflict
        })
    );
    // Once an answer continued the task, a competing one finds a new turn.
    for phase in [Phase::Pending, Phase::Running] {
        assert_eq!(
            decided(&answer, &view(phase, 4, 4)),
            Decision::Done(Outcome::Rejected {
                reason: Rejection::Stale
            })
        );
    }
    // An answer to a question from an earlier turn is stale.
    let late = entry(request("b", Kind::Answer, 1, "Yes."));
    assert_eq!(
        decided(&late, &asked),
        Decision::Done(Outcome::Rejected {
            reason: Rejection::Stale
        })
    );
    assert_eq!(
        decided(&answer, &view(Phase::Unknown, 3, 2)),
        Decision::Done(Outcome::Rejected {
            reason: Rejection::Unavailable
        })
    );
}

#[test]
fn a_leased_queue_waits_and_an_edit_keeps_the_request_for_replay() {
    let temp = tempfile::tempdir().unwrap();
    let dir = temp.path().join("tasks");
    ended_task(&dir);
    let (phone, tablet) = (sender("phone"), sender("tablet"));
    record(
        &dir,
        &phone,
        &request("a", Kind::Send, 2, "First."),
        &STEERING,
        &always,
        NOW,
    )
    .unwrap();
    let second = request("b", Kind::Queue, 3, "Second.");
    record(&dir, &phone, &second, &STEERING, &always, NOW).unwrap();
    record(
        &dir,
        &tablet,
        &request("c", Kind::Queue, 3, "Third."),
        &STEERING,
        &always,
        NOW,
    )
    .unwrap();
    let edit = |sender: &Sender, edit: QueueEdit, now: u64| {
        edit_queue(&dir, "task", sender, &edit, &STEERING, &always, now)
    };
    // Changes need the lease, and only one device holds it.
    assert!(matches!(
        edit(
            &phone,
            QueueEdit::Remove {
                command: second.command.clone()
            },
            NOW
        ),
        Err(Error::Conflict)
    ));
    let (leased, _) = edit(&phone, QueueEdit::Lease, NOW).unwrap();
    assert_eq!(leased.lease.as_ref().unwrap().expires_at, NOW + LEASE);
    assert!(matches!(
        edit(&tablet, QueueEdit::Lease, NOW),
        Err(Error::Conflict)
    ));
    // A device edits only its own messages.
    assert!(matches!(
        edit(
            &phone,
            QueueEdit::Edit {
                command: "c".repeat(64),
                text: "Mine now.".into()
            },
            NOW
        ),
        Err(Error::NotFound)
    ));
    let (edited, _) = edit(
        &phone,
        QueueEdit::Edit {
            command: second.command.clone(),
            text: "Second, edited.".into(),
        },
        NOW,
    )
    .unwrap();
    assert_eq!(
        edited
            .items
            .iter()
            .map(|item| item.text.as_str())
            .collect::<Vec<_>>(),
        ["Second, edited.", "Third."]
    );
    // A reorder names the exact permutation of the queued messages.
    assert!(matches!(
        edit(
            &phone,
            QueueEdit::Reorder {
                commands: vec![second.command.clone()]
            },
            NOW
        ),
        Err(Error::Conflict)
    ));
    let (reordered, _) = edit(
        &phone,
        QueueEdit::Reorder {
            commands: vec!["c".repeat(64), second.command.clone()],
        },
        NOW,
    )
    .unwrap();
    assert_eq!(reordered.items[0].text, "Third.");
    // The turn ends while the lease holds: nothing is promoted.
    end(&mut Store::open(&dir).unwrap(), "end-2", 3);
    assert!(
        process(&dir, "task", &STEERING, &always, NOW + 1)
            .unwrap()
            .is_empty()
    );
    // A replay of the edited command still matches its request.
    let (replayed, _) = record(&dir, &phone, &second, &STEERING, &always, NOW + 2).unwrap();
    assert_eq!(replayed.state, State::Held { priority: false });
    // Releasing the lease runs the queue in its new order.
    let (released, continued) = edit(&phone, QueueEdit::Release, NOW + 3).unwrap();
    assert!(released.lease.is_none());
    assert_eq!(continued.len(), 1);
    let task = Store::open(&dir).unwrap().show("task").unwrap();
    assert_eq!(task.effective_prompt(), "Third.");
    end(&mut Store::open(&dir).unwrap(), "end-3", 5);
    process(&dir, "task", &STEERING, &always, NOW + 4).unwrap();
    let task = Store::open(&dir).unwrap().show("task").unwrap();
    assert_eq!(task.effective_prompt(), "Second, edited.");
}

#[test]
fn a_lapsed_lease_lets_the_queue_run_and_a_removed_message_never_runs() {
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
    let dropped = request("b", Kind::Queue, 3, "Never mind.");
    record(&dir, &phone, &dropped, &STEERING, &always, NOW).unwrap();
    record(
        &dir,
        &phone,
        &request("c", Kind::Queue, 3, "Kept."),
        &STEERING,
        &always,
        NOW,
    )
    .unwrap();
    edit_queue(
        &dir,
        "task",
        &phone,
        &QueueEdit::Lease,
        &STEERING,
        &always,
        NOW,
    )
    .unwrap();
    let removal = QueueEdit::Remove {
        command: dropped.command.clone(),
    };
    let (removed, _) = edit_queue(&dir, "task", &phone, &removal, &STEERING, &always, NOW).unwrap();
    assert_eq!(removed.items.len(), 1);
    // Removing it again succeeds again.
    edit_queue(&dir, "task", &phone, &removal, &STEERING, &always, NOW).unwrap();
    end(&mut Store::open(&dir).unwrap(), "end-2", 3);
    assert!(
        process(&dir, "task", &STEERING, &always, NOW + LEASE - 1)
            .unwrap()
            .is_empty()
    );
    // The lease lapsed without a renewal.
    let continued = process(&dir, "task", &STEERING, &always, NOW + LEASE).unwrap();
    assert_eq!(continued.len(), 1);
    let task = Store::open(&dir).unwrap().show("task").unwrap();
    assert_eq!(task.effective_prompt(), "Kept.");
    let removed = entries(&dir)
        .unwrap()
        .into_iter()
        .find(|entry| entry.request.command == dropped.command)
        .unwrap();
    assert_eq!(removed.state, State::Done(Outcome::Cancelled));
}

#[test]
fn a_message_sent_now_stops_the_turn_and_runs_ahead_of_the_queue() {
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
    let urgent = request("c", Kind::Queue, 3, "Urgent.");
    record(&dir, &phone, &urgent, &STEERING, &always, NOW).unwrap();
    edit_queue(
        &dir,
        "task",
        &phone,
        &QueueEdit::Lease,
        &STEERING,
        &always,
        NOW,
    )
    .unwrap();
    end(&mut Store::open(&dir).unwrap(), "end-2", 3);
    // The lease holds the queue, but a message sent now runs ahead of it.
    let (sent, continued) = edit_queue(
        &dir,
        "task",
        &phone,
        &QueueEdit::SendNow {
            command: urgent.command.clone(),
        },
        &STEERING,
        &always,
        NOW,
    )
    .unwrap();
    assert_eq!(continued.len(), 1);
    assert_eq!(sent.task.effective_prompt(), "Urgent.");
    assert_eq!(
        sent.items
            .iter()
            .map(|item| item.text.as_str())
            .collect::<Vec<_>>(),
        ["Queued."]
    );
    // While a turn runs, a message sent now is the engine's emulated
    // steering: it stops the turn and continues with the message.
    let mut promoted = entry(request("d", Kind::Queue, 5, "Now."));
    promoted.promoted = true;
    promoted.state = State::Held { priority: false };
    assert_eq!(
        decided(&promoted, &view(Phase::Running, 5, 5)),
        Decision::Dispatch(Effect::CancelThenContinue("Now.".into()))
    );
    // An edited message runs with its new text.
    let mut edited = entry(request("e", Kind::Queue, 5, "Old."));
    edited.edited = Some("New.".into());
    assert_eq!(
        decided(&edited, &view(Phase::Ended, 5, 5)),
        Decision::Dispatch(Effect::Continue("New.".into()))
    );
}

#[test]
fn a_queued_message_never_revives_an_archived_task() {
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
    // The turn is stopped and the task archived while the message waits.
    end(&mut Store::open(&dir).unwrap(), "end-2", 3);
    crate::task::archive::archive(
        &dir,
        "task",
        "Archived by a test",
        crate::task::archive::By::Owner,
        NOW,
    )
    .unwrap();
    assert!(
        process(&dir, "task", &STEERING, &always, NOW)
            .unwrap()
            .is_empty()
    );
    let task = Store::open(&dir).unwrap().show("task").unwrap();
    assert_eq!(task.status, Status::Cancelled);
    assert_eq!(
        entries(&dir).unwrap()[1].state,
        State::Done(Outcome::Rejected {
            reason: Rejection::Conflict
        })
    );
    // A later send is refused the same way.
    let (sent, _) = record(
        &dir,
        &phone,
        &request("c", Kind::Send, 4, "Again."),
        &STEERING,
        &always,
        NOW,
    )
    .unwrap();
    assert_eq!(
        sent.state,
        State::Done(Outcome::Rejected {
            reason: Rejection::Conflict
        })
    );
}
