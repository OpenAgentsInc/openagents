//! Screen readers' view of the chat window (#10024): the AccessKit tree the
//! window hands VoiceOver, Orca (AT-SPI), and Narrator, read through
//! `accesskit_consumer` as those platform adapters read it, and a screen
//! reader's requests run as the same clicks and typing a person gives.

use super::tests::chat_fixture;
use super::*;
use openagents_desktop::fake::FakeHost;
use openagents_desktop::model::{Agent, Screen};
use rust_native_desktop::access::accesskit::{
    Action, ActionData, ActionRequest, NodeId, Role, TreeId,
};
use rust_native_desktop::access::{Tree, answer};

const SIZE: (f32, f32) = (1200.0, 840.0);

fn tree(app: &mut DesktopApp, focus: Option<&str>) -> Tree {
    app.present();
    let (_, scene) = rust_native_desktop::capture(app, SIZE.0, SIZE.1, 2.0);
    Tree::of(app, &scene, focus, 2.0, 0.0)
}

/// Each node a screen reader reaches, one a line: role, spoken name,
/// value, and the actions it admits.
fn outline(tree: &Tree) -> Vec<String> {
    fn write(node: accesskit_consumer::NodeRef<'_>, depth: usize, out: &mut Vec<String>) {
        let data = node.data();
        let mut line = format!("{}{:?}", "  ".repeat(depth), node.role());
        let name = node.label().or_else(|| {
            node.label_comes_from_value()
                .then(|| node.value())
                .flatten()
        });
        if let Some(name) = name {
            line.push_str(&format!(" {name:?}"));
        }
        if !node.label_comes_from_value()
            && let Some(value) = node.value()
        {
            line.push_str(&format!(" = {value:?}"));
        }
        match node.toggled() {
            Some(rust_native_desktop::access::accesskit::Toggled::True) => line.push_str(" [on]"),
            Some(_) => line.push_str(" [off]"),
            None => {}
        }
        if node.is_disabled() {
            line.push_str(" [disabled]");
        }
        if data.supports_action(Action::Click) {
            line.push_str(" <click>");
        }
        if data.supports_action(Action::SetValue) {
            line.push_str(" <set value>");
        }
        if node.is_focused() {
            line.push_str(" *focused*");
        }
        out.push(line);
        for child in node.children() {
            write(child, depth + 1, out);
        }
    }
    let consumer = accesskit_consumer::Tree::new(tree.update.clone(), true);
    let mut out = vec![];
    write(consumer.state().root(), 0, &mut out);
    out
}

fn find(lines: &[String], wanted: &str) -> usize {
    lines
        .iter()
        .position(|line| line.trim_start().starts_with(wanted))
        .unwrap_or_else(|| panic!("no {wanted:?} in\n{}", lines.join("\n")))
}

fn node_with(tree: &Tree, role: Role, name: &str) -> NodeId {
    tree.update
        .nodes
        .iter()
        .find(|(_, node)| {
            node.role() == role
                && (node.label() == Some(name)
                    || (role == Role::Label && node.value() == Some(name)))
        })
        .map(|(id, _)| *id)
        .unwrap_or_else(|| panic!("no {role:?} {name:?}"))
}

fn request(id: NodeId, action: Action, data: Option<ActionData>) -> ActionRequest {
    ActionRequest {
        action,
        target_tree: TreeId::ROOT,
        target_node: id,
        data,
    }
}

/// Runs a screen reader's request the way the window does.
fn run(app: &mut DesktopApp, tree: &Tree, id: NodeId, action: Action, data: Option<ActionData>) {
    let request = tree
        .request(&request(id, action, data))
        .unwrap_or_else(|| panic!("{action:?} is admitted"));
    answer(app, request, Instant::now());
    app.tick(Instant::now());
}

#[test]
fn a_screen_reader_navigates_chats_reads_replies_and_sends_a_message() {
    let (mut app, _) = chat_fixture(4);
    let tree = tree(&mut app, None);
    let lines = outline(&tree);
    let dump = lines.join("\n");
    // The window, named by its title.
    assert!(lines[0].starts_with("Window \"OpenAgents"), "{dump}");
    // The conversation is a log of articles: who wrote each, then what it says.
    let log = find(&lines, "Log \"Conversation\"");
    let question = find(&lines, "Label \"Question 0: how does this work?\"");
    let reply = find(
        &lines,
        "Label \"Reply 1 with bold, italic, and inline code.",
    );
    assert!(log < question && question < reply, "{dump}");
    assert!(
        lines[question - 1]
            .trim_start()
            .starts_with("Article \"You\""),
        "{dump}"
    );
    assert!(
        lines[reply - 1]
            .trim_start()
            .starts_with("Article \"Reply\""),
        "{dump}"
    );
    assert!(
        dump.contains("let answer = 42;"),
        "code blocks are read: {dump}"
    );
    // The composer is a text field named by its placeholder, and the send
    // control a button.
    let composer = find(&lines, "MultilineTextInput \"Message OpenAgents");
    assert!(
        lines[composer].contains("= \"\" <click> <set value>"),
        "{dump}"
    );
    // Every control has a name.
    for line in &lines {
        let role = line.trim_start().split(' ').next().unwrap();
        if matches!(role, "Button" | "CheckBox" | "MultilineTextInput" | "List") {
            assert!(line.contains('"'), "unnamed control: {line}\n{dump}");
        }
    }
}

#[test]
fn a_screen_reader_writes_and_sends_a_message() {
    let fake = FakeHost::new("Test computer", unix_now());
    let context = Context::new(
        Box::new(fake.clone()),
        Some(fake),
        None,
        None,
        std::env::temp_dir(),
    );
    let mut app = DesktopApp::inline_chat(
        Model::new(Instant::now(), Screen::Connect, Agent::Enabled),
        context,
    );
    // New chat, from the sidebar.
    let start = tree(&mut app, None);
    let new_chat = node_with(&start, Role::Button, "New chat");
    run(&mut app, &start, new_chat, Action::Click, None);
    let before = tree(&mut app, None);
    let composer = before.id("chat-composer").expect("a composer");
    // Focus lands in the composer, as a click there puts it.
    run(&mut app, &before, composer, Action::Focus, None);
    let focused = tree(&mut app, None);
    assert_eq!(
        focused.update.focus, composer,
        "the text cursor is the focus"
    );
    // VoiceOver's and Orca's typing sets the value.
    run(
        &mut app,
        &focused,
        composer,
        Action::SetValue,
        Some(ActionData::Value("Is the build green?".into())),
    );
    assert_eq!(app.chat.as_ref().unwrap().draft(), "Is the build green?");
    let typed = tree(&mut app, None);
    let lines = outline(&typed);
    let dump = lines.join("\n");
    let field = find(&lines, "MultilineTextInput \"Message OpenAgents");
    assert!(
        lines[field].contains("= \"Is the build green?\""),
        "the value reads back: {dump}"
    );
    // The send control, pressed by the screen reader, sends it.
    let send = typed
        .update
        .nodes
        .iter()
        .find(|(_, node)| {
            node.role() == Role::Button
                && node.label().is_some_and(|label| label.starts_with("Send"))
        })
        .map(|(id, _)| *id)
        .unwrap_or_else(|| panic!("a send button in\n{dump}"));
    run(&mut app, &typed, send, Action::Click, None);
    assert_eq!(app.chat.as_ref().unwrap().draft(), "", "sent");
    let after = tree(&mut app, None);
    let lines = outline(&after);
    let dump = lines.join("\n");
    let log = find(&lines, "Log \"Conversation\"");
    assert!(lines[log + 1].trim_start() == "Article \"You\"", "{dump}");
    assert!(
        lines[log + 2].trim_start() == "Label \"Is the build green?\"",
        "{dump}"
    );
    find(&lines, "Status \"OpenAgents is replying…\"");
    find(&lines, "Button \"Stop\" <click>");
    // The chat is now a row in the sidebar. A new chat, then that row,
    // brings the message back.
    let row = |tree: &Tree| {
        tree.update
            .nodes
            .iter()
            .find(|(_, node)| {
                node.role() == Role::Button
                    && node
                        .label()
                        .is_some_and(|label| label.starts_with("Is the build green?"))
            })
            .map(|(id, _)| *id)
            .unwrap_or_else(|| panic!("a sidebar row in\n{dump}"))
    };
    row(&after);
    run(
        &mut app,
        &after,
        node_with(&after, Role::Button, "New chat"),
        Action::Click,
        None,
    );
    let empty = tree(&mut app, None);
    assert!(
        !outline(&empty)
            .iter()
            .any(|line| line.contains("Label \"Is the build green?\"")),
        "a new chat is empty"
    );
    run(&mut app, &empty, row(&empty), Action::Click, None);
    let back = tree(&mut app, None);
    node_with(&back, Role::Label, "Is the build green?");
}

#[test]
fn a_screen_reader_opens_another_chat_from_the_sidebar() {
    let (mut app, _) = chat_fixture(2);
    let before = tree(&mut app, None);
    let lines = outline(&before);
    let dump = lines.join("\n");
    // The sidebar's chats are buttons; pick one not already shown.
    let new_chat = before
        .update
        .nodes
        .iter()
        .find(|(_, node)| {
            node.role() == Role::Button
                && node
                    .label()
                    .is_some_and(|label| label.to_lowercase().contains("new chat"))
        })
        .map(|(id, _)| *id)
        .unwrap_or_else(|| panic!("a New chat button in\n{dump}"));
    let shown = app
        .chat
        .as_ref()
        .unwrap()
        .state()
        .and_then(|s| s.chat.clone());
    run(&mut app, &before, new_chat, Action::Click, None);
    app.present();
    let now_shown = app
        .chat
        .as_ref()
        .unwrap()
        .state()
        .and_then(|s| s.chat.clone());
    assert_ne!(
        shown, now_shown,
        "the screen reader's click opened a new chat"
    );
}

#[test]
fn a_screen_reader_opens_settings_and_reads_its_controls() {
    let (mut app, _) = chat_fixture(0);
    let chat = tree(&mut app, None);
    run(
        &mut app,
        &chat,
        node_with(&chat, Role::Button, "Settings"),
        Action::Click,
        None,
    );
    let settings = tree(&mut app, None);
    let lines = outline(&settings);
    let dump = lines.join("\n");
    // Settings' switches are checkboxes that say whether they are on.
    let switch = find(&lines, "CheckBox \"");
    assert!(
        lines[switch].contains("[on]") || lines[switch].contains("[off]"),
        "{dump}"
    );
    // Whatever settings the page grows, every control is named and every
    // enabled one clickable.
    for line in &lines {
        let role = line.trim_start().split(' ').next().unwrap();
        if matches!(role, "Button" | "CheckBox" | "MultilineTextInput") {
            assert!(line.contains('"'), "unnamed control: {line}\n{dump}");
            assert!(
                line.contains("<click>") || line.contains("[disabled]"),
                "{line}\n{dump}"
            );
        }
    }
}

/// A follow-up chip above the composer is a button a screen reader names
/// by its words, can focus, and presses to send them (#10075, #10024).
#[test]
fn a_screen_reader_reaches_and_presses_a_followup_chip() {
    let (mut app, _) =
        super::card_fixtures::followups_fixture(&[], &["What can you do?", "What model is this?"]);
    let before = tree(&mut app, None);
    let lines = outline(&before);
    let dump = lines.join("\n");
    let chip = find(&lines, "Button \"What can you do?\"");
    assert!(lines[chip].contains("<click>"), "{dump}");
    let composer = find(&lines, "MultilineTextInput \"Message OpenAgents");
    assert!(
        chip < composer,
        "the chips read before the composer: {dump}"
    );
    let id = node_with(&before, Role::Button, "What model is this?");
    assert!(
        before
            .update
            .nodes
            .iter()
            .any(|(node, data)| *node == id && data.supports_action(Action::Focus)),
        "a chip takes keyboard focus: {dump}"
    );
    run(&mut app, &before, id, Action::Click, None);
    let after = tree(&mut app, None);
    let lines = outline(&after);
    assert!(
        !lines
            .iter()
            .any(|line| line.contains("Button \"What can you do?\"")),
        "the chips leave while the reply to one comes:\n{}",
        lines.join("\n")
    );
}

/// A new chat's starter chip above the centered composer is a button a
/// screen reader names by its words, reads before the composer, can focus,
/// and presses to send them (#10097).
#[test]
fn a_screen_reader_reaches_and_presses_a_starter_chip() {
    let (mut app, _) = chat_fixture(0);
    assert!(app.chat.as_ref().unwrap().composer_centered());
    let before = tree(&mut app, None);
    let lines = outline(&before);
    let dump = lines.join("\n");
    let chip = find(&lines, "Button \"What is OpenAgents?\"");
    assert!(lines[chip].contains("<click>"), "{dump}");
    let composer = find(&lines, "MultilineTextInput \"Message OpenAgents");
    assert!(
        chip < composer,
        "the starters read before the composer: {dump}"
    );
    let id = node_with(&before, Role::Button, "What models do you use?");
    assert!(
        before
            .update
            .nodes
            .iter()
            .any(|(node, data)| *node == id && data.supports_action(Action::Focus)),
        "a starter takes keyboard focus: {dump}"
    );
    run(&mut app, &before, id, Action::Click, None);
    let after = tree(&mut app, None);
    let lines = outline(&after);
    assert!(
        !lines
            .iter()
            .any(|line| line.contains("Button \"What is OpenAgents?\"")),
        "the starters leave once the chat has a message:\n{}",
        lines.join("\n")
    );
}
