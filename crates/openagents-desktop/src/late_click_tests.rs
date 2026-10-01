//! Transcript buttons that must work while events stream (#10046): a click
//! whose press and release straddle one new revision still runs when the
//! transcript offers the same action for the same target, and only then.

use super::coder_events::{tasks, window};
use super::*;
use openagents_chat::{
    basic_chats::Spawned,
    basic_coder::Turn,
    coder_events::Line,
    service::{Command, Snapshot},
};
use openagents_chat_app::coder_run::{Answer, Request as RunRequest, State as RunState};
use openagents_desktop::chat::{Panel, TRANSCRIPT};
use openagents_desktop::chat_action::Action as ChatAction;
use rust_native_desktop::input::SurfaceInput;
use std::time::Duration;

const QUESTION_THEN_RESULT: &str =
    include_str!("../../openagents-chat/fixtures/coder-events/question-then-result.ndjson");
const OTHER_ENDINGS: &str =
    include_str!("../../openagents-chat/fixtures/coder-events/other-endings.ndjson");

fn chat(app: &DesktopApp) -> String {
    app.chat
        .as_ref()
        .unwrap()
        .state()
        .and_then(|state| state.chat.clone())
        .unwrap()
}

/// The next Coder poll's answer is `lines`, `state`: one Coder event.
fn coder_event(panel: &mut Panel, chat: &str, lines: &[Line], state: RunState) {
    let start = Instant::now() + Duration::from_secs(5);
    for step in 0..40 {
        match panel.tick(start + Duration::from_millis(step * 300)) {
            Some(Request::CoderRun {
                ticket,
                request: RunRequest::Poll { .. },
                ..
            }) => {
                panel.run_outcome(
                    chat.into(),
                    ticket,
                    Ok(Answer::Lines {
                        lines: lines.to_vec(),
                        state,
                    }),
                );
                return;
            }
            Some(Request::CoderRun { ticket, .. }) => {
                panel.run_outcome(chat.into(), ticket, Err("not in this test".into()));
            }
            _ => {}
        }
    }
    panic!("no poll");
}

/// Presses `key` in the transcript, runs `between` (each call a new shown
/// revision), and releases where the press began. Returns the request the
/// click dispatches, if it activated anything.
fn click_across(
    app: &mut DesktopApp,
    key: &str,
    between: &mut [&mut dyn FnMut(&mut Panel)],
) -> Option<Option<Request>> {
    app.present();
    rust_native_desktop::capture(app, 1200.0, 840.0, 1.0);
    let now = Instant::now();
    let panel = app.chat.as_mut().unwrap();
    let bounds = panel
        .transcript
        .control_bounds(key)
        .unwrap_or_else(|| panic!("{key} shows"));
    let (x, y) = (bounds.x + bounds.w / 2.0, bounds.y + bounds.h / 2.0);
    assert!(panel.surface(TRANSCRIPT, SurfaceInput::Down { x, y, shift: false }, now));
    for step in between.iter_mut() {
        step(app.chat.as_mut().unwrap());
        app.present();
        rust_native_desktop::capture(app, 1200.0, 840.0, 1.0);
    }
    let panel = app.chat.as_mut().unwrap();
    panel.surface(TRANSCRIPT, SurfaceInput::Up { x, y }, now);
    let activated = panel.take_activated();
    if activated.is_empty() {
        return None;
    }
    assert_eq!(activated, vec![key.to_owned()]);
    let view = app.view().clone();
    Some(
        app.chat
            .as_mut()
            .unwrap()
            .action(ChatAction::Card { key: key.into() }, &view, now),
    )
}

fn run_request(request: Option<Option<Request>>) -> RunRequest {
    match request {
        Some(Some(Request::CoderRun { request, .. })) => request,
        other => panic!("no Coder request: {other:?}"),
    }
}

/// A run whose Stop Coder shows, and the event that comes next: a new
/// row above the button.
fn running() -> (DesktopApp, String, Vec<Line>) {
    let whole = tasks(QUESTION_THEN_RESULT).remove(0);
    let app = window(&whole[..7], RunState::Running);
    let chat = chat(&app);
    (app, chat, whole[7..10].to_vec())
}

#[test]
fn stop_coder_stops_when_an_event_lands_between_press_and_release() {
    let (mut app, chat, next) = running();
    let task = next[0].task.clone();
    let request = click_across(
        &mut app,
        "coder-stop",
        &mut [&mut |panel| coder_event(panel, &chat, &next, RunState::Running)],
    );
    assert_eq!(run_request(request), RunRequest::Stop { task });
}

/// While Coder runs, the transcript's Stop Coder is as wide as its words,
/// as the phone draws its Coder controls, not the reading band's width
/// (#10091, #10075).
#[test]
fn the_running_stop_coder_control_is_content_sized() {
    let (mut app, _, _) = running();
    for (width, height) in [(1200.0, 840.0), (760.0, 540.0)] {
        app.present();
        let (_, scene) = rust_native_desktop::capture(&mut app, width, height, 1.0);
        let band = scene.surface_rect(TRANSCRIPT).expect("the transcript").w;
        let panel = app.chat.as_ref().unwrap();
        let stop = panel
            .transcript
            .control_bounds("coder-stop")
            .expect("Stop Coder shows while Coder runs");
        assert!(
            stop.w < 160.0 && stop.w < band / 3.0,
            "Stop Coder is {} wide in a {band}-point transcript at {width}",
            stop.w
        );
    }
}

#[test]
fn a_click_spanning_two_revisions_is_still_ignored() {
    let (mut app, chat, next) = running();
    let (first, second) = next.split_at(1);
    let request = click_across(
        &mut app,
        "coder-stop",
        &mut [
            &mut |panel| coder_event(panel, &chat, first, RunState::Running),
            &mut |panel| coder_event(panel, &chat, second, RunState::Running),
        ],
    );
    assert!(request.is_none(), "{request:?}");
}

/// A run waiting on an approval, and a Coder event that keeps it: the
/// failed read of the next poll, shown under the buttons.
fn approval() -> (DesktopApp, String) {
    let app = window(&tasks(OTHER_ENDINGS)[0], RunState::Waiting);
    let chat = chat(&app);
    (app, chat)
}

fn failed_read(panel: &mut Panel, chat: &str) {
    let start = Instant::now() + Duration::from_secs(5);
    for step in 0..40 {
        if let Some(Request::CoderRun {
            ticket,
            request: RunRequest::Poll { .. },
            ..
        }) = panel.tick(start + Duration::from_secs(step * 3))
        {
            panel.run_outcome(chat.into(), ticket, Err("Coder's log is busy.".into()));
            return;
        }
    }
    panic!("no poll");
}

#[test]
fn approve_and_deny_answer_when_an_event_lands_between_press_and_release() {
    for (key, text) in [("coder-approve", "Approved."), ("coder-deny", "Denied.")] {
        let (mut app, chat) = approval();
        let task = tasks(OTHER_ENDINGS)[0][0].task.clone();
        let request = click_across(&mut app, key, &mut [&mut |panel| failed_read(panel, &chat)]);
        assert_eq!(
            run_request(request),
            RunRequest::Continue {
                task,
                text: text.into()
            },
            "{key}"
        );
    }
}

#[test]
fn approve_is_ignored_when_the_event_is_another_approval() {
    let (mut app, chat) = approval();
    let mut next = tasks(OTHER_ENDINGS)[0].last().unwrap().clone();
    next.seq += 1;
    let request = click_across(
        &mut app,
        "coder-approve",
        &mut [&mut |panel| coder_event(panel, &chat, &[next.clone()], RunState::Waiting)],
    );
    assert!(request.is_none(), "{request:?}");
}

/// Queues `texts` for the running Coder's next turn from the composer.
fn queue(app: &mut DesktopApp, texts: &[&str]) {
    use rust_native_desktop::input::TextInput;
    let now = Instant::now();
    for text in texts {
        app.present();
        app.text_input(TextInput::Commit(text), now);
        app.activate(
            Intent::Chat {
                action: ChatAction::Send,
            },
            now,
        );
    }
    let chat = chat(app);
    assert_eq!(
        app.chat
            .as_ref()
            .unwrap()
            .coder_run(&chat)
            .unwrap()
            .queued(),
        texts.len()
    );
}

#[test]
fn queued_send_now_and_remove_work_when_an_event_lands_between_press_and_release() {
    let (mut app, chat, next) = running();
    let task = next[0].task.clone();
    queue(&mut app, &["cover empty input", "and unicode"]);
    let request = click_across(
        &mut app,
        "coder-queued-1-remove",
        &mut [&mut |panel| coder_event(panel, &chat, &next[..1], RunState::Running)],
    );
    assert_eq!(request, Some(None), "removing sends nothing");
    assert_eq!(
        app.chat
            .as_ref()
            .unwrap()
            .coder_run(&chat)
            .unwrap()
            .queued(),
        1
    );
    // Sending now while Coder runs steers: stop, then continue with it.
    let request = click_across(
        &mut app,
        "coder-queued-0-now",
        &mut [&mut |panel| coder_event(panel, &chat, &next[1..], RunState::Running)],
    );
    assert_eq!(run_request(request), RunRequest::Stop { task });
    assert_eq!(
        app.chat
            .as_ref()
            .unwrap()
            .coder_run(&chat)
            .unwrap()
            .queued(),
        0
    );
}

#[test]
fn choose_folder_asks_when_the_chat_updates_between_press_and_release() {
    let mut app = super::tests::chat_fixture(0).0;
    let panel = app.chat.as_mut().unwrap();
    let Request::Chat {
        ticket,
        command: Command::Create { chat },
    } = panel.new_chat()
    else {
        panic!("create")
    };
    let snapshot = Snapshot {
        chat: Some(chat.clone()),
        computer: true,
        ready_computer: Some("Scratch Mac".into()),
        total: 2,
        turns: vec![
            Turn::user("fix the flaky test in openagents"),
            Turn::assistant(
                "Ready for Coder.",
                Some(openagents_chat::router::Meta {
                    route: Some(openagents_chat::delegation::DISPATCH_ROUTE.into()),
                    ..Default::default()
                }),
            ),
        ],
        ..Default::default()
    };
    panel.outcome(ticket, Ok(snapshot.clone()));
    app.present();
    let view = app.view().clone();
    let panel = app.chat.as_mut().unwrap();
    panel.action(
        ChatAction::Card {
            key: "coder-run".into(),
        },
        &view,
        Instant::now(),
    );
    let mut started = false;
    for step in 0..20 {
        match panel.tick(Instant::now() + Duration::from_millis(step * 10)) {
            Some(Request::Chat { ticket, .. }) => panel.outcome(ticket, Ok(snapshot.clone())),
            Some(Request::CoderRun {
                ticket,
                request: RunRequest::Start { .. },
                ..
            }) => {
                panel.run_outcome(
                    chat.clone(),
                    ticket,
                    Ok(Answer::NeedsProject {
                        why: "/w is not a Git checkout.".into(),
                    }),
                );
                started = true;
                break;
            }
            _ => {}
        }
    }
    assert!(started);
    // Before a task exists no Coder event arrives; the chat's own reply
    // does.
    let later = Snapshot {
        total: 3,
        turns: vec![
            Turn::user("fix the flaky test in openagents"),
            Turn::assistant(
                "Ready for Coder.",
                Some(openagents_chat::router::Meta {
                    route: Some(openagents_chat::delegation::DISPATCH_ROUTE.into()),
                    ..Default::default()
                }),
            ),
            Turn::assistant("Coder needs its project folder.", None),
        ],
        ..snapshot.clone()
    };
    let request = click_across(
        &mut app,
        "coder-choose",
        &mut [&mut |panel| {
            for step in 0..20 {
                if let Some(Request::Chat { ticket, .. }) =
                    panel.tick(Instant::now() + Duration::from_secs(10 + step))
                {
                    panel.outcome(ticket, Ok(later.clone()));
                    return;
                }
            }
            // No read due: the reply arrives as the answer to one.
            let Request::Chat { ticket, .. } = panel
                .select_numeric(0)
                .unwrap_or_else(|| panic!("no chat read"))
            else {
                panic!("read")
            };
            panel.outcome(ticket, Ok(later.clone()));
        }],
    );
    assert_eq!(run_request(request), RunRequest::Choose);
}

/// A remote task's Approve (#10003): the activity summary a Coder host
/// publishes while it waits bumps the task's revision, not its approval.
#[test]
fn a_remote_task_s_approve_answers_across_a_summary_update_for_the_same_approval() {
    use nostr::activity_summary::{self, Attention, Phase, SubjectKind, SummaryDraft};
    use openagents_chat_app::task_chat;
    let summary = |task: &str, sequence, headline: &str| {
        activity_summary::encode(&SummaryDraft {
            host: &"a".repeat(64),
            subject_kind: SubjectKind::Task,
            subject: task,
            sequence,
            phase: Phase::Waiting,
            headline,
            attention: Attention::Approval,
            updated_at: unix_now(),
        })
        .unwrap()
    };
    let activity = |panel: &mut Panel, sequence, headline: &'static str| {
        let start = Instant::now() + Duration::from_secs(sequence * 30);
        for step in 0..40 {
            match panel.tick(start + Duration::from_secs(step)) {
                Some(Request::TaskChat {
                    chat,
                    ticket,
                    request: task_chat::Request::Activity { task },
                }) => {
                    panel.task_outcome(
                        chat,
                        ticket,
                        Ok(task_chat::Answer::Activity(summary(
                            &task, sequence, headline,
                        ))),
                    );
                    return;
                }
                Some(Request::TaskChat { chat, ticket, .. }) => {
                    panel.task_outcome(
                        chat,
                        ticket,
                        Err(openagents_desktop::control::ControlError::Unreachable),
                    );
                }
                _ => {}
            }
        }
        panic!("no activity read");
    };
    for (headline, answered) in [("May I delete slugs.py?", true), ("May I push?", false)] {
        let mut app = super::tests::chat_fixture(0).0;
        let panel = app.chat.as_mut().unwrap();
        let Request::Chat {
            ticket,
            command: Command::Create { chat },
        } = panel.new_chat()
        else {
            panic!("create")
        };
        panel.outcome(
            ticket,
            Ok(Snapshot {
                chat: Some(chat),
                coder: Some(Spawned {
                    host: "a".repeat(64),
                    task: "b".repeat(64),
                    project: Some("scratch".into()),
                    at: None,
                }),
                ..Snapshot::default()
            }),
        );
        activity(panel, 4, "May I delete slugs.py?");
        let request = click_across(
            &mut app,
            "task-approve",
            &mut [&mut |panel| activity(panel, 5, headline)],
        );
        match request {
            Some(Some(Request::TaskChat { .. })) => assert!(answered, "{headline}"),
            None => assert!(!answered, "{headline}"),
            other => panic!("{other:?}"),
        }
    }
}
