//! The selector row's states, rendered without a server: what each
//! selector offers for what this person has. The whole-site paths
//! (signed in through GitHub, a Coder computer online, sending) are in
//! `crate::projects::tests::composer_row`.

use super::*;

fn project(id: &str, repository: &str) -> Project {
    Project {
        id: id.into(),
        name: repository.split('/').nth(1).unwrap().into(),
        repository_id: 7,
        repository: repository.into(),
        default_branch: "main".into(),
        private: false,
        created_unix: 1,
        installation_id: None,
    }
}

const APP: &str = "prj_0123456789abcdef";
const OTHER: &str = "prj_fedcba9876543210";

fn environment() -> Environment {
    Environment {
        id: "env-1".into(),
        repository: "acme/app".into(),
        branch: "main".into(),
        version: 3,
    }
}

fn choices() -> Choices {
    Choices {
        projects: vec![project(APP, "acme/app"), project(OTHER, "acme/other")],
        connected: true,
        environments: vec![environment()],
        computers: vec!["studio-mac".into()],
        claude_connect: false,
        claude_subscription: false,
    }
}

fn wanted(project: &str, branch: &str, target: &str) -> Wanted {
    Wanted {
        project: project.into(),
        branch: branch.into(),
        target: target.into(),
        chat: None,
        focus: None,
    }
}

fn render(choices: Option<&Choices>, picked: &Picked) -> String {
    row(choices, picked, None, None, false).into_string()
}

fn nothing() -> Picked {
    Picked {
        project: None,
        branch: None,
        target: Target::Chat,
    }
}

#[test]
fn targets_parse_and_print_back() {
    for target in [
        Target::Chat,
        Target::Claude("env-1".into()),
        Target::Coder("studio mac".into()),
    ] {
        assert_eq!(Target::parse(&target.value()), target);
    }
    assert_eq!(Target::parse("claude:"), Target::Chat);
    assert_eq!(Target::parse("anything"), Target::Chat);
}

#[test]
fn signed_out_visitors_get_an_empty_row() {
    let html = render(None, &nothing());
    assert_eq!(
        html,
        r#"<div class="oa-composer-selector-group" id="composer-row"></div>"#
    );
}

#[test]
fn signed_in_without_projects_offers_only_the_project_selector() {
    let choices = Choices::default();
    let picked = choices.resolve(&wanted("", "", ""));
    let html = render(Some(&choices), &picked);
    assert!(
        html.contains(r#"aria-label="Project: No project""#),
        "{html}"
    );
    assert!(html.contains(r#"hx-get="/composer/row/project""#));
    assert!(
        !html.contains("Branch:") && !html.contains("Where it runs"),
        "{html}"
    );
    for name in ["project", "branch", "target"] {
        assert!(
            html.contains(&format!(r#"name="{name}" value="" form="chat-form""#)),
            "{name}: {html}"
        );
    }
    // The panel's one item connects a repository.
    let panel = project_panel(&choices, &picked, &|_: &dyn Fn(&mut Picked), _: &str| {
        String::new()
    })
    .into_string();
    assert!(panel.contains("Connect a GitHub repository"), "{panel}");
    assert!(panel.contains(r#"href="/projects""#), "{panel}");
    crate::copy_guard::assert_plain("/composer/row/project", &panel);
}

#[test]
fn a_project_brings_its_default_branch_and_its_environment() {
    let choices = choices();
    let picked = choices.resolve(&wanted(APP, "", ""));
    assert_eq!(picked.branch.as_deref(), Some("main"));
    assert_eq!(picked.target, Target::Chat);
    let html = render(Some(&choices), &picked);
    assert!(html.contains(r#"aria-label="Project: app""#), "{html}");
    assert!(html.contains(r#"aria-label="Branch: main""#), "{html}");
    assert!(
        html.contains(r#"aria-label="Where it runs: Chat""#),
        "{html}"
    );
    assert!(html.contains(&format!(r#"name="project" value="{APP}""#)));

    let picked = choices.resolve(&wanted(APP, "fix-login", "claude:env-1"));
    assert_eq!(picked.branch.as_deref(), Some("fix-login"));
    assert_eq!(picked.target, Target::Claude("env-1".into()));
    let html = render(Some(&choices), &picked);
    assert!(
        html.contains(r#"aria-label="Where it runs: Claude Code v3""#),
        "{html}"
    );
    assert!(html.contains(r#"name="target" value="claude:env-1""#));
}

#[test]
fn claude_code_is_offered_only_for_the_project_with_the_environment() {
    let choices = choices();
    // Another project: its repository has no environment.
    let picked = choices.resolve(&wanted(OTHER, "", "claude:env-1"));
    assert_eq!(picked.target, Target::Chat);
    let labels: Vec<String> = choices
        .targets(picked.project.as_ref())
        .into_iter()
        .map(|(_, label)| label)
        .collect();
    assert_eq!(labels, ["Chat", "Coder on studio-mac"]);
    // No project: no Claude Code.
    let picked = choices.resolve(&wanted("", "", "claude:env-1"));
    assert_eq!(picked.target, Target::Chat);
    assert_eq!(picked.branch, None);
    // An unknown project is no project.
    let picked = choices.resolve(&wanted("prj_1111111111111111", "dev", ""));
    assert_eq!(picked.project, None);
    assert_eq!(picked.branch, None);
}

#[test]
fn coder_is_offered_while_a_computer_is_online() {
    let mut choices = choices();
    let picked = choices.resolve(&wanted("", "", "coder:studio-mac"));
    assert_eq!(picked.target, Target::Coder("studio-mac".into()));
    let html = render(Some(&choices), &picked);
    assert!(
        html.contains(r#"aria-label="Where it runs: Coder on studio-mac""#),
        "{html}"
    );
    // Offline: back to Chat, and with nothing else, no selector.
    choices.computers.clear();
    choices.environments.clear();
    let picked = choices.resolve(&wanted("", "", "coder:studio-mac"));
    assert_eq!(picked.target, Target::Chat);
    let html = render(Some(&choices), &picked);
    assert!(!html.contains("Where it runs"), "{html}");
}

#[test]
fn the_target_panel_says_what_each_place_does() {
    let choices = choices();
    let picked = choices.resolve(&wanted(APP, "", ""));
    let panel = target_panel(
        &choices,
        &picked,
        &|change: &dyn Fn(&mut Picked), focus: &str| {
            let mut next = picked.clone();
            change(&mut next);
            let mut pairs = query(&next, None);
            pairs.push(("focus", focus.to_owned()));
            href(ROW, &pairs)
        },
    )
    .into_string();
    for text in [
        "Answers here. Nothing runs.",
        "Claude Code in app v3",
        "Its answer shows in this chat.",
        "Coder on studio-mac",
    ] {
        assert!(panel.contains(text), "{text}: {panel}");
    }
    // Each choice reloads the row with its pick, focus back on the
    // dropdown; the current one is marked and takes focus.
    assert!(panel.contains("target=claude%3Aenv-1"), "{panel}");
    assert!(panel.contains("target=coder%3Astudio-mac"), "{panel}");
    assert!(panel.contains("focus=target"), "{panel}");
    assert!(panel.contains(r##"hx-target="#composer-row""##));
    assert_eq!(panel.matches(r#"aria-current="true""#).count(), 1);
    crate::copy_guard::assert_plain("/composer/row/target", &panel);
}

#[test]
fn a_reloaded_row_focuses_the_dropdown_that_changed() {
    let choices = choices();
    let picked = choices.resolve(&wanted(APP, "", ""));
    let html = row(
        Some(&choices),
        &picked,
        Some("chat-1"),
        Some("branch"),
        false,
    )
    .into_string();
    assert_eq!(html.matches("autofocus").count(), 1, "{html}");
    assert!(html.contains(r#"title="Branch: main" autofocus"#), "{html}");
}

#[test]
fn a_chat_row_keeps_its_project_branch_and_claude_code() {
    use crate::chat_store::{ChatEnvironment, ChatTask, Message, Role, TaskKind, TaskState};
    let mut chat = crate::chat_store::Conversation {
        id: "11111111-1111-4111-8111-111111111111".into(),
        owner: "v_owner".into(),
        revision: 1,
        title: "Fix it".into(),
        messages: vec![Message {
            role: Role::User,
            text: "Fix the login".into(),
            request_id: None,
        }],
        pending: None,
        requests: Vec::new(),
        selection: None,
        updated_unix: 1,
        pinned_unix: None,
        archived_unix: None,
        project: Some(APP.into()),
        terminal: None,
        environment: Some(ChatEnvironment {
            id: "env-1".into(),
            repository: "acme/app".into(),
            version: Some(3),
            removed: false,
        }),
        tasks: vec![ChatTask {
            id: "run-1".into(),
            kind: TaskKind::Claude,
            environment: "env-1".into(),
            title: "Fix the login".into(),
            state: TaskState::Working,
            started_unix: 1,
            after_message: 1,
            version: Some(3),
            finished_unix: None,
            agent: None,
        }],
        opened_unix: None,
        branch: Some("fix-login".into()),
    };
    let wanted = wanted_for(&chat);
    assert_eq!(wanted.project, APP);
    assert_eq!(wanted.branch, "fix-login");
    assert_eq!(wanted.target, "claude:env-1");
    // A message answered here since: Chat again.
    chat.messages.push(Message {
        role: Role::User,
        text: "Thanks".into(),
        request_id: None,
    });
    assert_eq!(wanted_for(&chat).target, "");
}

/// Picking Claude Code before Claude is connected shows the connect card
/// in the row, with the one control that works from here (#11234).
#[test]
fn claude_code_before_claude_is_connected_shows_the_connect_card() {
    let mut choices = choices();
    choices.claude_connect = true;
    // Offered, so it can be picked.
    let picked = choices.resolve(&wanted(APP, "", "claude:env-1"));
    assert_eq!(picked.target, Target::Claude("env-1".into()));
    let html = render(Some(&choices), &picked);
    assert!(html.contains(r#"id="composer-connect""#), "{html}");
    assert!(html.contains("Connect Claude to run Claude Code"), "{html}");
    assert!(html.contains(r#"href="/settings/claude#key""#), "{html}");
    assert!(html.contains("Use a key"), "{html}");
    // No button that can't work from here: one link.
    let card = connect_card(false, None).into_string();
    assert!(!card.contains("<button"), "{card}");
    assert_eq!(card.matches("<a ").count(), 1, "{card}");
    assert!(!card.contains("subscription"), "{card}");
    crate::copy_guard::assert_plain("/composer/row", &card);
    // On the subscription allowlist the card leads with the subscription:
    // the same dialog as Settings, Claude, loaded over the page and back
    // to this chat; without script, the same steps as a page.
    let card = connect_card(true, Some("chat-1")).into_string();
    assert!(
        !card.contains("<button") && !card.contains("<form"),
        "{card}"
    );
    assert_eq!(card.matches("<a ").count(), 2, "{card}");
    let lead = card.find("Use your Claude subscription").unwrap();
    assert!(lead < card.find("Use a key").unwrap(), "{card}");
    assert!(
        card.contains(r#"href="/settings/claude/subscription?back=%2Fchat%2Fchat-1""#),
        "{card}"
    );
    assert!(
        card.contains(
            r#"hx-get="/settings/claude/subscription?part=dialog&amp;back=%2Fchat%2Fchat-1" hx-target="body" hx-swap="beforeend""#
        ),
        "{card}"
    );
    crate::copy_guard::assert_plain("/composer/row", &card);
    choices.claude_subscription = true;
    let picked = choices.resolve(&wanted(APP, "", "claude:env-1"));
    assert!(render(Some(&choices), &picked).contains("Use your Claude subscription"));
    choices.claude_subscription = false;
    // Chat, or Claude Code once connected: no card.
    let chat = choices.resolve(&wanted(APP, "", ""));
    assert!(!render(Some(&choices), &chat).contains("composer-connect"));
    choices.claude_connect = false;
    let ready = choices.resolve(&wanted(APP, "", "claude:env-1"));
    assert!(!render(Some(&choices), &ready).contains("composer-connect"));
    // The refusal a send gets says the same, plainly.
    assert!(CONNECT.contains("your message stays in the box"));
    crate::copy_guard::assert_plain("/composer/row", CONNECT);
}
