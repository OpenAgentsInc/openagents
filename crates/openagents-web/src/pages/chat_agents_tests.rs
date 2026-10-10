use super::*;
use crate::chat_store::Message;

fn chat() -> Conversation {
    Conversation {
        id: "11111111-1111-4111-8111-111111111111".into(),
        owner: "v_owner".into(),
        revision: 1,
        title: "Fix the login".into(),
        messages: vec![Message {
            role: Role::User,
            text: "hi".into(),
            request_id: None,
        }],
        pending: None,
        requests: Vec::new(),
        selection: None,
        updated_unix: 1,
        pinned_unix: None,
        archived_unix: None,
        project: None,
        terminal: None,
        environment: Some(ChatEnvironment {
            id: "env-1".into(),
            repository: "acme/app".into(),
            version: Some(3),
            removed: false,
        }),
        tasks: Vec::new(),
        opened_unix: None,
        branch: None,
    }
}

fn agent(name: &str) -> TaskAgent {
    TaskAgent {
        name: name.into(),
        branch: format!("agent/{name}"),
        run: 1,
        inbox: Vec::new(),
    }
}

fn task(id: &str, state: TaskState, agent: Option<TaskAgent>) -> ChatTask {
    ChatTask {
        id: id.into(),
        kind: TaskKind::Claude,
        environment: "env-1".into(),
        title: "Fix the login redirect".into(),
        state,
        started_unix: 1_000,
        after_message: 1,
        version: Some(3),
        finished_unix: None,
        agent,
    }
}

#[test]
fn a_form_asks_for_one_to_five_agents() {
    assert_eq!(count(None), Ok(1));
    assert_eq!(count(Some(" ")), Ok(1));
    assert_eq!(count(Some("3")), Ok(3));
    assert_eq!(count(Some("5")), Ok(5));
    for bad in ["0", "6", "two", "-1"] {
        assert!(count(Some(bad)).is_err(), "{bad}");
    }
}

#[test]
fn each_agent_gets_a_name_unique_in_the_chat_and_its_own_branch() {
    let mut chat = chat();
    let first = plan(&chat, "Fix the login redirect", 3);
    let names: Vec<&str> = first.iter().map(|a| a.name.as_str()).collect();
    assert_eq!(
        names,
        ["fix-the-login-1", "fix-the-login-2", "fix-the-login-3"]
    );
    assert_eq!(first[1].branch, "agent/fix-the-login-2");
    assert!(first.iter().all(|a| a.run == 1 && a.inbox.is_empty()));
    for planned in &first {
        planned.validate().unwrap();
    }
    chat.tasks = first
        .into_iter()
        .enumerate()
        .map(|(n, a)| task(&format!("claude-env-1-{n}"), TaskState::Working, Some(a)))
        .collect();
    let next = plan(&chat, "Fix the login redirect", 2);
    let names: Vec<&str> = next.iter().map(|a| a.name.as_str()).collect();
    assert_eq!(names, ["fix-the-login-4", "fix-the-login-5"]);
}

#[test]
fn prompts_carry_the_branch_and_keep_the_request_last() {
    let one = agent("fix-1");
    let prompt = fleet_prompt("  Fix the login redirect  ", &one, 3);
    assert!(prompt.contains("one of 3 agents") && prompt.contains("agent/fix-1"));
    assert!(prompt.ends_with("\n\nThe request:\nFix the login redirect"));
    assert_eq!(request_of(&prompt), "Fix the login redirect");

    let next = follow_up_prompt(
        &prompt,
        &one,
        Some("Changed the redirect."),
        &["Also add a test.".to_owned(), "Keep it small.".to_owned()],
    );
    assert!(next.contains("fetch it and check it out"));
    assert!(next.contains("Changed the redirect."));
    assert!(next.contains("- Also add a test.\n- Keep it small."));
    assert_eq!(request_of(&next), "Fix the login redirect");
    // A second follow-up still finds the request once.
    let again = follow_up_prompt(&next, &one, None, &["More.".to_owned()]);
    assert_eq!(request_of(&again), "Fix the login redirect");
    assert_eq!(again.matches("The request:").count(), 1);

    // However long the report and messages, the prompt fits and keeps the
    // request.
    let long = vec!["m".repeat(4_000); MAX_AGENT_INBOX];
    let huge = follow_up_prompt(&prompt, &one, Some(&"r".repeat(50_000)), &long);
    assert!(huge.len() <= MAX_PROMPT, "{}", huge.len());
    assert!(huge.ends_with("The request:\nFix the login redirect"));
}

#[test]
fn a_result_is_short_and_says_how_it_ended() {
    let one = agent("fix-1");
    let mut done = task("claude-env-1-1", TaskState::Done, Some(one.clone()));
    done.finished_unix = Some(1_252);
    let text = result_text(
        &done,
        &one,
        TaskState::Done,
        Some("Fixed the redirect."),
        None,
        Some(0.42),
        9_999,
    );
    assert!(
        text.starts_with("Agent fix-1 (Claude Code) finished after 4m 12s · $0.42."),
        "{text}"
    );
    assert!(text.contains("Its work is on branch agent/fix-1."));
    assert!(text.contains("Its report:\nFixed the redirect."));

    let long = result_text(
        &done,
        &one,
        TaskState::Done,
        Some(&"x".repeat(5_000)),
        None,
        None,
        9_999,
    );
    assert!(long.ends_with("[The rest is in its transcript.]"));
    assert!(long.len() < 2_000, "{}", long.len());

    let failed = result_text(
        &done,
        &one,
        TaskState::Failed,
        Some("partial"),
        Some("The computer stopped."),
        None,
        9_999,
    );
    assert!(failed.contains("ran into a problem") && failed.contains("The computer stopped."));
    assert!(!failed.contains("partial"), "a failed run shows no report");

    let stopped = result_text(&done, &one, TaskState::Stopped, None, None, None, 9_999);
    assert!(stopped.contains("was stopped"));
}

#[test]
fn ended_agent_runs_put_their_result_in_the_chat_once() {
    use super::super::work::{Seen, observe};
    let mut chat = chat();
    chat.tasks = vec![
        task("a", TaskState::Working, Some(agent("fix-1"))),
        task("b", TaskState::Working, Some(agent("fix-2"))),
        task("c", TaskState::Working, None),
    ];
    let next = observe(&chat, |_, run| {
        Some(Seen {
            state: match run {
                "a" => TaskState::Done,
                "b" => TaskState::Failed,
                _ => TaskState::Working,
            },
            version: Some(3),
            reply: Some("Fixed.".into()),
            error: Some("It broke.".into()),
            cost_usd: Some(0.5),
        })
    })
    .expect("two ended");
    let said: Vec<&str> = next.messages[1..].iter().map(|m| m.text.as_str()).collect();
    assert_eq!(said.len(), 2, "{said:?}");
    assert!(
        said[0].starts_with("Agent fix-1 (Claude Code) finished") && said[0].contains("Fixed.")
    );
    assert!(said[1].starts_with("Agent fix-2 (Claude Code) ran into a problem"));
    assert!(said[1].contains("It broke."));
    assert!(next.messages[1..].iter().all(|m| m.role == Role::Assistant));
    // Read again: the ended agents are not read, and nothing new joins.
    assert!(
        observe(&next, |_, _| Some(Seen {
            state: TaskState::Working,
            version: Some(3),
            reply: None,
            error: None,
            cost_usd: None,
        }))
        .is_none()
    );
}

#[test]
fn the_list_shows_one_row_per_agent_with_its_time_and_cost() {
    let mut chat = chat();
    let mut first = task("a", TaskState::Done, Some(agent("fix-1")));
    first.finished_unix = Some(1_060);
    let mut second = task(
        "b",
        TaskState::Working,
        Some(TaskAgent {
            run: 2,
            ..agent("fix-1")
        }),
    );
    second.started_unix = 2_000;
    let paused = task("c", TaskState::Paused, Some(agent("fix-2")));
    let single = task("d", TaskState::Stopped, None);
    chat.tasks = vec![first, second, paused, single];
    let read = |task: &ChatTask| {
        Some(Live {
            cost_usd: Some(match task.id.as_str() {
                "a" => 0.25,
                "b" => 0.5,
                _ => 1.0,
            }),
            paused_until: (task.id == "c").then_some(50_700),
        })
    };
    let rows = run_rows(&chat, read, true, 2_030);
    assert_eq!(rows.len(), 3, "{rows:?}");
    assert_eq!(rows[0].name, "fix-1");
    assert_eq!(rows[0].target, Target::Run("b".into()));
    assert_eq!(rows[0].elapsed_seconds, 60 + 30, "both runs count");
    assert_eq!(rows[0].cost_usd, Some(0.75), "both runs' costs");
    assert_eq!(rows[0].standing, Standing::Working);
    assert!(rows[0].can_message);
    assert_eq!(
        rows[0].transcript.as_deref(),
        Some("/environments/env-1/runs/b")
    );
    assert_eq!(rows[1].standing, Standing::Paused(Some(50_700)));
    assert_eq!(rows[1].standing.label(), "Paused until 14:05 UTC");
    assert_eq!(rows[2].name, "Fix the login redirect");
    assert!(!rows[2].can_message, "a single run takes no messages");

    let hidden = run_rows(&chat, read, false, 2_030);
    assert!(
        hidden
            .iter()
            .all(|row| row.transcript.is_none() && !row.can_message)
    );
}

#[test]
fn coder_background_agents_for_this_chat_join_the_list() {
    use crate::phone_api::{Agents, Board, Item};
    let item = |id: &str, kind: &str, session: &str, status: &str| Item {
        id: id.into(),
        kind: kind.into(),
        title: id.into(),
        engine: Some("codex".into()),
        status: status.into(),
        started_unix: 100,
        finished_unix: None,
        cost_usd: Some(0.1),
        tokens: None,
        session: Some(session.into()),
        question: None,
        line: None,
    };
    let mut agents = Agents::default();
    agents.boards.insert(
        "studio".into(),
        Board {
            items: vec![
                item("agent-1", "agent", "s1", "working"),
                item("agent-2", "agent", "s2", "working"),
                item("s1", "chat", "s1", "working"),
                item("agent-3", "agent", "s1", "done"),
            ],
            updated_unix: 1,
            digest: String::new(),
        },
    );
    let rows = computer_rows(&agents, "s1", 160);
    assert_eq!(rows.len(), 2);
    assert_eq!(
        rows[0].target,
        Target::Computer {
            computer: "studio".into(),
            item: "agent-1".into()
        }
    );
    assert_eq!(rows[0].place, "studio");
    assert_eq!(rows[0].engine, "codex");
    assert_eq!(rows[0].elapsed_seconds, 60);
    assert!(rows[0].can_stop() && rows[0].can_message);
    assert_eq!(rows[1].standing, Standing::Done);
    assert!(!rows[1].can_stop());
}

#[test]
fn the_panel_offers_stop_message_and_the_transcript_and_reloads_while_working() {
    let row = Row {
        target: Target::Run("claude-env-1-2".into()),
        name: "fix-the-login-2".into(),
        place: "acme/app v3".into(),
        engine: ENGINE.into(),
        standing: Standing::Working,
        elapsed_seconds: 252,
        cost_usd: Some(0.42),
        waiting: 1,
        can_message: true,
        transcript: Some("/environments/env-1/runs/claude-env-1-2".into()),
    };
    let paused = Row {
        standing: Standing::Paused(Some(50_700)),
        target: Target::Computer {
            computer: "studio".into(),
            item: "agent-1".into(),
        },
        transcript: None,
        ..row.clone()
    };
    let html = render("c1", "token", &[row.clone(), paused], Some("Sent.")).into_string();
    for needle in [
        "Agents",
        "fix-the-login-2",
        "Claude Code · acme/app v3 · 4m 12s · $0.42 · 1 message waiting",
        r#"action="/chat/c1/agents/stop""#,
        r#"action="/chat/c1/agents/message""#,
        r#"name="run" value="claude-env-1-2""#,
        r#"name="computer" value="studio""#,
        r#"name="item" value="agent-1""#,
        r#"href="/environments/env-1/runs/claude-env-1-2""#,
        "Open transcript",
        "Paused until 14:05 UTC",
        r#"hx-trigger="every 5s""#,
        "Sent.",
    ] {
        assert!(html.contains(needle), "{needle} in {html}");
    }
    crate::copy_guard::assert_plain("/chat/c1", &html);

    let done = Row {
        standing: Standing::Done,
        waiting: 0,
        ..row
    };
    let still = render("c1", "token", &[done], None).into_string();
    assert!(!still.contains("hx-trigger"), "nothing works, no reloads");
    assert!(
        !still.contains("/agents/stop"),
        "an ended agent has no Stop"
    );
    assert!(still.contains("/agents/message"), "but takes a message");
}

#[test]
fn the_slot_shows_only_for_chats_with_runs_or_a_computer() {
    let mut chat = chat();
    assert_eq!(slot(&chat).into_string(), "");
    chat.tasks = vec![task("a", TaskState::Working, None)];
    let html = slot(&chat).into_string();
    assert!(html.contains(&format!(r#"hx-get="/chat/{}/agents""#, chat.id)));
    assert!(html.contains(r#"hx-trigger="load""#));
}

#[test]
fn a_message_waits_for_a_working_agent_and_restarts_an_ended_one() {
    let mut chat = chat();
    chat.tasks = vec![
        task("a", TaskState::Working, Some(agent("fix-1"))),
        task("b", TaskState::Done, Some(agent("fix-2"))),
        task("c", TaskState::Done, None),
    ];
    assert_eq!(
        queue(&mut chat, "a", "Also a test."),
        Queued::Waiting(false)
    );
    assert_eq!(queue(&mut chat, "b", "Try again."), Queued::Waiting(true));
    assert_eq!(queue(&mut chat, "c", "Hi."), Queued::NotAnAgent);
    assert_eq!(queue(&mut chat, "zzz", "Hi."), Queued::Missing);
    assert_eq!(due(&chat), vec![1], "only the ended one is due");
    for _ in 1..MAX_AGENT_INBOX {
        assert!(matches!(queue(&mut chat, "a", "more"), Queued::Waiting(_)));
    }
    assert_eq!(queue(&mut chat, "a", "too many"), Queued::Full);
    chat.tasks[0].validate().unwrap();

    // Once its next run is recorded, the older run is no longer due.
    chat.tasks.push(task(
        "d",
        TaskState::Working,
        Some(TaskAgent {
            run: 2,
            ..agent("fix-2")
        }),
    ));
    assert!(due(&chat).is_empty());
}

#[test]
fn a_clock_reads_hours_and_minutes_in_utc() {
    assert_eq!(clock(0), "00:00 UTC");
    assert_eq!(clock(50_700), "14:05 UTC");
    assert_eq!(clock(86_400 + 3_660), "01:01 UTC");
}
