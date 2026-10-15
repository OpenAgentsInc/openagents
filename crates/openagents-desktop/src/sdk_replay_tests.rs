//! Exercises recorded SDK conversations through the desktop's shared run model.
use super::*;
use coder_sdk::{
    observation::{Body, Channel, Ending, Observation, Recorder, Scope, TextMode},
    scripted::conversation::{Control, Conversation, Expected, Playback, Step},
};
use openagents_chat::coder_events::{CoderEvent, Line};
use route_contract::{
    Digest, digest_of, pending, resources as media,
    session::{self, control as config},
};
use std::time::Duration;

fn words(node: &Node<()>) -> Vec<String> {
    let mut out = Vec::new();
    let mut pending = vec![node];
    while let Some(node) = pending.pop() {
        match &node.element {
            Element::Text { value, .. } => out.push(value.clone()),
            Element::Button { label, .. } => out.push(label.clone()),
            Element::Markdown { blocks } => out.push(rust_native::markdown::plain(blocks)),
            Element::Stack { children, .. }
            | Element::List { children, .. }
            | Element::Message { children, .. }
            | Element::Tool { children, .. } => pending.extend(children),
            _ => {}
        }
    }
    out
}
fn text(value: &str, mode: TextMode) -> Body {
    Body::Text {
        channel: Channel::Text,
        mode,
        text: value.into(),
        complete: false,
        truncated: false,
    }
}
fn reference(scope: &Scope, bytes: &[u8]) -> media::Reference {
    let mut reference = media::Reference {
        schema: media::SCHEMA.into(),
        id: digest_of(&()),
        owner: media::Owner {
            host: scope.host.clone(),
            task: scope.task.clone(),
        },
        origin: media::Origin::Produced {
            scope: scope.clone(),
            item: "picture".into(),
        },
        digest: Digest::of_bytes(bytes),
        media_type: media::MediaType::Png,
        size: bytes.len(),
        name: "picture.png".into(),
        dimensions: Some(media::Dimensions {
            width: 1,
            height: 1,
        }),
        uses: vec![media::Usage::Preview, media::Usage::Read],
    };
    reference.seal();
    reference
}
fn requests(scope: &Scope) -> Vec<pending::Request> {
    let question = super::live_sdk_tests::snapshot(1).entries.remove(0).request;
    let mut approval = question.clone();
    approval.form = pending::Form::Approval;
    approval.native_call = "approve".into();
    approval.tool = "Bash".into();
    approval.prompt.command = "cargo test".into();
    [question, approval]
        .into_iter()
        .map(|mut request| {
            request.scope = scope.clone();
            request.seal();
            request
        })
        .collect()
}
fn snapshot(request: &pending::Request, revision: u64) -> pending::Snapshot {
    pending::Snapshot {
        schema: pending::SNAPSHOT_SCHEMA.into(),
        scope: request.scope.clone(),
        revision,
        active: true,
        entries: vec![pending::Entry {
            request: request.clone(),
            resolution: None,
            answer: None,
        }],
    }
}
fn feed(
    panel: &mut Panel,
    events: Vec<Observation>,
    native: Option<pending::Snapshot>,
    state: coder_run::State,
    at: Instant,
    seq: &mut u64,
) {
    let (ticket, request) = panel.runs.get_mut("chat").unwrap().tick(at).expect("poll");
    assert!(matches!(request, coder_run::Request::Poll { .. }));
    let mut lines = vec![];
    if *seq == 0 {
        let mut line: Line = serde_json::from_str(
            include_str!("../../openagents-chat/fixtures/coder-events/question-then-result.ndjson")
                .lines()
                .next()
                .unwrap(),
        )
        .unwrap();
        line.seq = 1;
        line.task = "task".into();
        line.thread = Some("chat".into());
        let CoderEvent::CoderStarted(started) = &mut line.event else {
            unreachable!()
        };
        started.provider = "claude".into();
        started.model = "fixture".into();
        let mut descriptor = session::Descriptor::default();
        descriptor.schema = session::SCHEMA.into();
        descriptor.owner = "local".into();
        descriptor.task = "task".into();
        descriptor.turn = 1;
        descriptor.resolved.selection = session::Selection {
            provider: "claude".into(),
            model: "fixture".into(),
            effort: None,
        };
        descriptor.resolved.adapter = "claude-sdk".into();
        descriptor.resolved.native.image_followups = true;
        descriptor.controls = session::Controls {
            stop: true,
            continue_task: true,
            queue: true,
            stop_and_send: true,
            answer: true,
        };
        started.session = Some(descriptor);
        lines.push(line);
        *seq = 1;
    }
    for observation in events {
        *seq += 1;
        lines.push(Line {
            seq: *seq,
            task: "task".into(),
            thread: Some("chat".into()),
            event: CoderEvent::Observation { observation },
        });
    }
    panel.runs.get_mut("chat").unwrap().outcome(
        ticket,
        Ok(coder_run::Answer::Lines {
            children: None,
            configuration: None,
            native,
            lines,
            state,
        }),
        at,
    );
}

#[test]
fn full_sdk_recording_keeps_controls_media_and_parent_state_separate() {
    let at = Instant::now();
    let scope = Scope {
        host: "local".into(),
        task: "task".into(),
        turn: 1,
        generation: 1,
    };
    let next = Scope {
        turn: 2,
        ..scope.clone()
    };
    let bytes = include_bytes!("../../coder-sdk/fixtures/resources/one-pixel.png");
    let resource = reference(&scope, bytes);
    let requests = requests(&scope);
    let answers = [
        pending::Answer::Questions {
            answers: vec![pending::QuestionAnswer {
                question: "notes".into(),
                choices: vec![],
                text: Some("Cover empty input".into()),
            }],
        },
        pending::Answer::AllowOnce,
    ];
    let replies: Vec<_> = requests
        .iter()
        .zip(answers)
        .map(|(request, answer)| pending::Reply {
            request: request.id.clone(),
            scope: scope.clone(),
            input_digest: request.input_digest.clone(),
            answer,
        })
        .collect();
    let native = config::Native {
        schema: config::NATIVE_SCHEMA.into(),
        host: "local".into(),
        task: "task".into(),
        turn: 1,
        session: "fixture-native-session".into(),
        provider: "claude".into(),
        model: "fixture".into(),
        adapter: "claude-sdk".into(),
        checkout: "/fixture".into(),
        target: "fixture-inode".into(),
        account: Some("fixture-account".into()),
    };
    let mut rec = Recorder::new(scope.clone());
    let mut opening = vec![
        rec.next(
            None,
            Body::Started {
                provider: "claude".into(),
                model: "fixture".into(),
            },
        ),
        rec.next(Some("reply".into()), text("Hello", TextMode::Snapshot)),
        rec.next(Some("reply".into()), text(" world", TextMode::Delta)),
        rec.next(
            None,
            Body::Plan {
                items: vec![route_contract::observation::PlanItem {
                    text: "Check the change".into(),
                    status: route_contract::observation::PlanStatus::InProgress,
                }],
            },
        ),
        rec.next(
            None,
            Body::Resource {
                reference: Box::new(resource.clone()),
            },
        ),
    ];
    let mut child = rec.next(
        None,
        Body::Child {
            parent: None,
            title: "Inspect".into(),
            harness: "fixture".into(),
            model: None,
            status: route_contract::children::Status::Running,
        },
    );
    child.child = Some("child-1".into());
    opening.push(child);
    let mut child_text = rec.next(
        Some("child-reply".into()),
        text("CHILD ONLY", TextMode::Snapshot),
    );
    child_text.child = Some("child-1".into());
    opening.push(child_text);
    let mut resumed = Recorder::new(next.clone());
    let recording = Conversation {
        schema: coder_sdk::scripted::conversation::SCHEMA.into(),
        owner: resource.owner.clone(),
        steps: vec![
            Step::Observations { events: opening },
            Step::Await {
                expected: Expected::Read {
                    reference: resource.id.clone(),
                },
            },
            Step::Await {
                expected: Expected::Reply {
                    reply: replies[0].clone(),
                },
            },
            Step::Await {
                expected: Expected::Reply {
                    reply: replies[1].clone(),
                },
            },
            Step::Observations {
                events: vec![rec.next(
                    None,
                    Body::Ended {
                        ending: Ending::Completed,
                        message: None,
                    },
                )],
            },
            Step::Await {
                expected: Expected::Input {
                    owner: resource.owner.clone(),
                    text: Digest::of_bytes(b"Inspect next"),
                    images: vec![Digest::of_bytes(bytes)],
                },
            },
            Step::Await {
                expected: Expected::Resume {
                    scope: next.clone(),
                    native: native.clone(),
                },
            },
            Step::Observations {
                events: vec![
                    resumed.next(
                        None,
                        Body::Started {
                            provider: "claude".into(),
                            model: "fixture".into(),
                        },
                    ),
                    resumed.next(Some("reply".into()), text("Resumed", TextMode::Snapshot)),
                ],
            },
            Step::Await {
                expected: Expected::Stop {
                    scope: next.clone(),
                },
            },
            Step::Observations {
                events: vec![
                    resumed.next(
                        None,
                        Body::Ended {
                            ending: Ending::Interrupted,
                            message: None,
                        },
                    ),
                    resumed.next(Some("reply".into()), text("LATE OUTPUT", TextMode::Delta)),
                    rec.next(
                        None,
                        Body::Resource {
                            reference: Box::new(resource.clone()),
                        },
                    ),
                ],
            },
        ],
    };
    let mut replay = Conversation::from_json(&serde_json::to_string(&recording).unwrap())
        .unwrap()
        .play()
        .unwrap();
    let mut panel = Panel::new(at);
    panel.session.select("chat");
    panel.selected_changed(None);
    panel
        .runs
        .insert("chat".into(), Run::follow("chat", "task", None, at));
    let mut seq = 0;
    feed(
        &mut panel,
        replay.next_observations().unwrap(),
        Some(snapshot(&requests[0], 2)),
        coder_run::State::Running,
        at,
        &mut seq,
    );
    let labels = panel
        .runs
        .get_mut("chat")
        .unwrap()
        .rows()
        .iter()
        .flat_map(words)
        .collect::<Vec<_>>();
    assert!(
        labels.iter().any(|s| s.contains("Hello world")),
        "{labels:?}"
    );
    assert!(!labels.iter().any(|s| s.contains("CHILD ONLY")));
    // An owner read can fail independently of the conversation or composer.
    let mut inspector = coder_run::resources::Inspector::new("task");
    assert!(inspector.register(&resource));
    let target = inspector.target(coder_run::resources::Control::Open(resource.id.clone()));
    panel
        .runs
        .get_mut("chat")
        .unwrap()
        .action(coder_run::Action::Resource(target), "");
    let (ticket, read) = panel.runs.get_mut("chat").unwrap().tick(at).unwrap();
    let coder_run::Request::ResourceRead {
        reference,
        generation,
        ..
    } = read
    else {
        panic!("owner resource read")
    };
    replay
        .deliver(&Control::Read {
            reference,
            generation,
        })
        .unwrap();
    panel.run_outcome(
        "chat".into(),
        ticket,
        Err("This attachment is unavailable. Add it again.".into()),
    );
    assert_eq!(panel.runs["chat"].state(), coder_run::State::Running);
    assert!(panel.runs["chat"].resource_preview().is_none());
    for index in 0..2 {
        if index == 1 {
            feed(
                &mut panel,
                vec![],
                Some(snapshot(&requests[index], 3)),
                coder_run::State::Running,
                at + Duration::from_secs(4),
                &mut seq,
            );
        }
        let action = if index == 0 {
            coder_run::Action::Send
        } else {
            coder_run::Action::NativeDecision {
                request: requests[index].id.clone(),
                control: openagents_chat_app::decision::Control::Pick(0),
            }
        };
        let (ticket, request) = panel
            .runs
            .get_mut("chat")
            .unwrap()
            .action(action, "Cover empty input")
            .unwrap();
        let coder_run::Request::Respond { reply, .. } = request else {
            panic!("callback answer")
        };
        assert_eq!(reply, replies[index]);
        assert!(requests[index].accepts(&reply));
        let control = Control::Reply {
            reply: reply.clone(),
        };
        assert!(replay.deliver(&control).unwrap());
        panel.run_outcome(
            "chat".into(),
            ticket,
            Ok(coder_run::Answer::Responded(pending::Receipt {
                request: reply.request,
                resolution: pending::Resolution::Answered,
                duplicate: false,
            })),
        );
        assert!(!replay.deliver(&control).unwrap());
    }
    // Queue photos with their message, then retry the same delivery after a lost acknowledgment.
    let image =
        openagents_chat_app::attachments::Image::decode("fixture.png", bytes.to_vec()).unwrap();
    let upload =
        openagents_chat_app::attachments::Upload::new(&image.name, image.bytes.clone()).unwrap();
    let run = panel.runs.get_mut("chat").unwrap();
    run.action_with_images(
        coder_run::Action::Queue,
        "Inspect next",
        vec![upload],
        vec![image.clone()],
    );
    assert!(run.take_consumed_input());
    feed(
        &mut panel,
        replay.next_observations().unwrap(),
        None,
        coder_run::State::Ended,
        at + Duration::from_secs(8),
        &mut seq,
    );
    let (ticket, request) = panel
        .runs
        .get_mut("chat")
        .unwrap()
        .tick(at + Duration::from_secs(9))
        .unwrap();
    let coder_run::Request::ContinueInput {
        message,
        configuration,
        ..
    } = &request
    else {
        panic!("queued media")
    };
    let mut attachment = resource.clone();
    attachment.origin = media::Origin::Input {
        message: message.id.clone(),
    };
    attachment.uses = vec![media::Usage::PromptImage, media::Usage::Preview];
    attachment.name = media::name(&image.name);
    attachment.seal();
    let mut input = media::Input {
        schema: media::SCHEMA.into(),
        id: digest_of(&()),
        message: message.id.clone(),
        turn: 2,
        owner: resource.owner.clone(),
        text: Digest::of_bytes(message.text.as_bytes()),
        choice: configuration.as_ref().map(digest_of),
        resources: vec![attachment],
    };
    input.seal();
    let receipt = media::Receipt {
        schema: media::SCHEMA.into(),
        input: input.id.clone(),
        task: "task".into(),
        turn: 2,
        revision: 3,
    };
    let control = Control::Input {
        input: input.clone(),
        receipt: receipt.clone(),
        text: message.text.clone(),
    };
    replay.deliver(&control).unwrap();
    panel.run_outcome("chat".into(), ticket, Err("The owner is offline.".into()));
    let (ticket, retry) = panel
        .runs
        .get_mut("chat")
        .unwrap()
        .action(coder_run::Action::Retry, "")
        .unwrap();
    assert_eq!(retry, request);
    replay = Playback::restore(&replay.checkpoint().unwrap()).unwrap();
    assert!(!replay.deliver(&control).unwrap());
    panel.session.images.add("chat", image.clone()).unwrap();
    let another =
        openagents_chat_app::attachments::Image::decode("new.png", bytes.to_vec()).unwrap();
    panel.session.images.add("chat", another.clone()).unwrap();
    panel.run_outcome(
        "chat".into(),
        ticket,
        Ok(coder_run::Answer::InputAccepted { input, receipt }),
    );
    assert_eq!(
        panel
            .session
            .images
            .get("chat")
            .iter()
            .map(|x| &x.id)
            .collect::<Vec<_>>(),
        vec![&another.id]
    );
    replay
        .deliver(&Control::Resume {
            scope: next.clone(),
            native: native.clone(),
        })
        .unwrap();
    let selection = session::Selection {
        provider: "claude".into(),
        model: "fixture".into(),
        effort: None,
    };
    let start = config::Start {
        request: Some(config::Request {
            schema: config::SCHEMA.into(),
            offer: digest_of(&()),
            adapter: "claude-sdk".into(),
            checkout: native.checkout.clone(),
            target: native.target.clone(),
            selection: selection.clone(),
            mode: config::Mode::NativeResume,
            resume: Some(native.id()),
        }),
        selection,
        account: native.account.clone(),
        task_model: None,
        context: None,
        native: Some(native.clone()),
    };
    assert_eq!(
        coder_sdk::session::claude_start(&start, Default::default())
            .unwrap()
            .resume
            .as_deref(),
        Some(native.session.as_str())
    );
    feed(
        &mut panel,
        replay.next_observations().unwrap(),
        None,
        coder_run::State::Running,
        at + Duration::from_secs(12),
        &mut seq,
    );
    let (ticket, request) = panel
        .runs
        .get_mut("chat")
        .unwrap()
        .action(coder_run::Action::Stop, "")
        .unwrap();
    assert!(matches!(request, coder_run::Request::Stop { .. }));
    replay.deliver(&Control::Stop { scope: next }).unwrap();
    panel.run_outcome("chat".into(), ticket, Ok(coder_run::Answer::Stopping));
    feed(
        &mut panel,
        replay.next_observations().unwrap(),
        None,
        coder_run::State::Ended,
        at + Duration::from_secs(16),
        &mut seq,
    );
    let labels = panel
        .runs
        .get_mut("chat")
        .unwrap()
        .rows()
        .iter()
        .flat_map(words)
        .collect::<Vec<_>>();
    assert!(
        !labels.iter().any(|s| s.contains("LATE OUTPUT")),
        "{labels:?}"
    );
    assert!(replay.finished());
    assert_eq!(panel.runs["chat"].state(), coder_run::State::Ended);
    assert_eq!(*another.bytes, bytes.to_vec());
}
