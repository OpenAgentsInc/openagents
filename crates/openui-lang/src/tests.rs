use super::embed::{self, Segment};
use super::*;

const CONNECT: &str = r#"root = Columns([web, computer])
web = Card("On the web", [Text("Connect GitHub and pick a repository."), Button("Connect GitHub", href="/auth/github/repos?access=private", show="signed_in"), Button("Log in to connect", href="/login?return_to=/projects", show="signed_out")])
computer = Card("On your computer", [Steps([install, login, sync])])
install = Step("Install Coder", [Command("curl -fsSL https://openagents.com/cli/install.sh | bash", windows="irm https://openagents.com/cli/install.ps1 | iex")])
login = Step("Sign in", [CodeBlock("coder login", "bash"), Button("Approve sign-in", href="/device", style="secondary")])
sync = Step("Save its chats to your account", [CodeBlock("/sync on")])
"#;

fn texts(node: &Node) -> Vec<String> {
    let mut out = Vec::new();
    embed::walk(node, &mut |node| match node {
        Node::Card { title, .. } => out.push(title.clone()),
        Node::Text { text } => out.push(text.clone()),
        Node::Link { label, href } | Node::Button { label, href, .. } => {
            out.push(label.clone());
            out.push(href.clone());
        }
        Node::CodeBlock { code, .. } => out.push(code.clone()),
        Node::Command { unix, windows } => {
            out.push(unix.clone());
            out.extend(windows.clone());
        }
        Node::Steps { steps } => out.extend(steps.iter().map(|s| s.title.clone())),
        _ => {}
    });
    out
}

#[test]
fn the_connect_answer_parses_clean() {
    let document = parse(CONNECT);
    assert!(
        document.diagnostics.is_empty(),
        "{:?}",
        document.diagnostics
    );
    let Some(Node::Columns { children }) = &document.root else {
        panic!("{:?}", document.root);
    };
    assert_eq!(children.len(), 2);
    let Node::Card {
        title,
        children: web,
    } = &children[0]
    else {
        panic!()
    };
    assert_eq!(title, "On the web");
    assert!(matches!(
        &web[1],
        Node::Button {
            show: Audience::SignedIn,
            style: ButtonStyle::Primary,
            ..
        }
    ));
    assert!(matches!(
        &web[2],
        Node::Button {
            show: Audience::SignedOut,
            ..
        }
    ));
    let root = document.root.clone().unwrap();
    assert_eq!(
        embed::links(&root),
        [
            "https://openagents.com/auth/github/repos?access=private",
            "https://openagents.com/login?return_to=/projects",
            "https://openagents.com/device"
        ]
    );
    assert_eq!(
        embed::commands(&root),
        [
            "curl -fsSL https://openagents.com/cli/install.sh | bash",
            "irm https://openagents.com/cli/install.ps1 | iex",
            "coder login",
            "/sync on"
        ]
    );
    // The tree round-trips through JSON, as the native apps receive it.
    let json = serde_json::to_string(&root).unwrap();
    let back: Node = serde_json::from_str(&json).unwrap();
    assert_eq!(back, root);
}

/// Every prefix of the program, cut anywhere, parses without a panic into
/// a tree whose every string is one the finished tree has whole: no
/// half-written text or link ever shows. The streaming parser, fed the
/// prefixes in order, agrees with the one-shot parse at every step and at
/// the end.
#[test]
fn every_cut_point_shows_only_whole_values() {
    let whole = parse(CONNECT).root.unwrap();
    let allowed = texts(&whole);
    let mut stream = Stream::default();
    let mut seen_root = false;
    for (at, _) in CONNECT.char_indices().chain([(CONNECT.len(), ' ')]) {
        let prefix = &CONNECT[..at];
        let fresh = parse_partial(prefix);
        let streamed = stream.update(prefix);
        assert_eq!(fresh.root, streamed.root, "at {at}");
        if let Some(root) = &fresh.root {
            seen_root = true;
            for text in texts(root) {
                assert!(allowed.contains(&text), "at {at}: `{text}` is not whole");
            }
        }
        // While streaming, nothing is reported as missing.
        assert!(
            fresh
                .diagnostics
                .iter()
                .all(|d| !d.message.contains("never defined") && !d.message.contains("needs")),
            "at {at}: {:?}",
            fresh.diagnostics
        );
    }
    assert!(seen_root);
    assert_eq!(stream.finish(CONNECT).root, Some(whole));
}

#[test]
fn a_finished_statement_is_not_replaced_by_one_still_being_written() {
    let source = "root = Stack([a])\na = Text(\"first\")\na = Stack([Text(\"sec";
    let document = parse_partial(source);
    assert_eq!(
        document.root,
        Some(Node::Stack {
            children: vec![Node::Text {
                text: "first".into()
            }]
        })
    );
    // Once whole, the later statement replaces it (merge by name).
    let document = parse("root = Stack([a])\na = Text(\"first\")\na = Text(\"second\")");
    assert_eq!(
        document.root,
        Some(Node::Stack {
            children: vec![Node::Text {
                text: "second".into()
            }]
        })
    );
}

#[test]
fn forward_references_draw_nothing_until_defined() {
    let partial = parse_partial("root = Stack([a, b])\na = Text(\"one\")\n");
    assert_eq!(
        partial.root,
        Some(Node::Stack {
            children: vec![Node::Text { text: "one".into() }]
        })
    );
    assert!(partial.diagnostics.is_empty());
    let whole = parse("root = Stack([a, b])\na = Text(\"one\")\n");
    assert!(
        whole
            .diagnostics
            .iter()
            .any(|d| d.message.contains("`b` is never defined")),
        "{:?}",
        whole.diagnostics
    );
}

#[test]
fn invalid_parts_are_dropped_and_reported() {
    let document = parse(
        r#"root = Stack([Text("ok"), Chart("x"), Button("Go", href="javascript:alert(1)"), Button("Fine", "/download", style="loud", color="red"), Step("loose"), Text(42), Link("x")])"#,
    );
    let Some(Node::Stack { children }) = &document.root else {
        panic!("{:?}", document.root)
    };
    assert_eq!(
        children,
        &[
            Node::Text { text: "ok".into() },
            Node::Button {
                label: "Fine".into(),
                href: "/download".into(),
                style: ButtonStyle::Primary,
                show: Audience::Everyone
            },
            Node::Text { text: "42".into() },
        ]
    );
    let messages: Vec<&str> = document
        .diagnostics
        .iter()
        .map(|d| d.message.as_str())
        .collect();
    for expected in [
        "`Chart` is not in the catalog",
        "is not an https:// URL or a site path",
        "`loud` is not one of",
        "has no argument `color`",
        "holds blocks, so an item was dropped",
        "Link needs href",
    ] {
        assert!(
            messages.iter().any(|m| m.contains(expected)),
            "{expected}: {messages:?}"
        );
    }
}

#[test]
fn syntax_variants_parse() {
    // Named arguments with `:`, single quotes, escapes, comments, a call
    // over several lines, a trailing comma, and one component where a list
    // belongs.
    let document = parse(
        "// a comment\nroot = Card(title: 'Hi \\'you\\'', children: Text(\"a\\nb\"))\n\nx = Steps([\n  Step(\"one\"),\n  Step(\"two\"),\n])\n",
    );
    assert_eq!(
        document.root,
        Some(Node::Card {
            title: "Hi 'you'".into(),
            children: vec![Node::Text {
                text: "a\nb".into()
            }]
        })
    );
    assert!(
        document.diagnostics.is_empty(),
        "{:?}",
        document.diagnostics
    );
    let broken = parse("root = Text(\"a\")\nthis is not a statement\ny = = 3\n");
    assert_eq!(broken.root, Some(Node::Text { text: "a".into() }));
    assert_eq!(broken.diagnostics.len(), 2, "{:?}", broken.diagnostics);
    // A cycle is refused, not followed forever.
    let cycle = parse("root = Stack([a])\na = Stack([root])");
    assert!(
        cycle
            .diagnostics
            .iter()
            .any(|d| d.message.contains("refers to itself"))
    );
    // No root: nothing draws.
    assert_eq!(parse("a = Text(\"x\")").root, None);
}

#[test]
fn safe_links_only() {
    for good in [
        "https://openagents.com/download",
        "/device",
        "/login?return_to=/projects",
    ] {
        assert!(safe_href(good), "{good}");
    }
    for bad in [
        "http://example.com",
        "javascript:alert(1)",
        "//evil.example",
        "https://",
        "https://a b",
        "data:text/html,x",
        "mailto:a@b",
        "/x\"onclick",
    ] {
        assert!(!safe_href(bad), "{bad}");
    }
}

#[test]
fn blocks_are_found_in_markdown() {
    let reply =
        format!("Two ways to connect your code.\n\n```openui-lang\n{CONNECT}```\n\nThat's all.\n");
    let segments = embed::segments(&reply);
    assert_eq!(segments.len(), 3, "{segments:?}");
    assert_eq!(
        segments[0],
        Segment::Markdown("Two ways to connect your code.\n\n")
    );
    assert_eq!(
        segments[1],
        Segment::Ui {
            source: CONNECT,
            closed: true
        }
    );
    assert_eq!(
        embed::prose(&reply),
        "Two ways to connect your code.\n\n\nThat's all."
    );
    // Still streaming: the open block is read with the streaming rules.
    let cut = &reply[..reply.find("login = ").unwrap()];
    assert!(matches!(
        embed::segments(cut).last(),
        Some(Segment::Ui { closed: false, .. })
    ));
    assert_eq!(embed::trees(cut).len(), 1);
    // A block inside another code block is code.
    let quoted = "````md\n```openui-lang\nroot = Text(\"x\")\n```\n````\n";
    assert!(!embed::has_ui(quoted));
}

#[test]
fn the_fallback_is_markdown_with_links_and_commands() {
    let reply = format!("Two ways to connect your code.\n\n```openui-lang\n{CONNECT}```\n");
    let fallback = embed::fallback(&reply);
    assert!(
        fallback.starts_with("Two ways to connect your code.\n\n**On the web**"),
        "{fallback}"
    );
    for expected in [
        "[Connect GitHub](https://openagents.com/auth/github/repos?access=private)",
        "1. Install Coder",
        "   macOS and Linux:",
        "   ```bash\n   curl -fsSL https://openagents.com/cli/install.sh | bash\n   ```",
        "   Windows (PowerShell):",
        "irm https://openagents.com/cli/install.ps1 | iex",
        "2. Sign in",
        "[Approve sign-in](https://openagents.com/device)",
        "3. Save its chats to your account",
        "/sync on",
    ] {
        assert!(fallback.contains(expected), "{expected}:\n{fallback}");
    }
    // The signed-out button is not in the fallback.
    assert!(!fallback.contains("Log in to connect"), "{fallback}");
    assert!(!fallback.contains("openui-lang"), "{fallback}");
    // Text without a block is untouched.
    assert_eq!(embed::fallback("Just text."), "Just text.");
    let plain = embed::plain(&parse(CONNECT).root.unwrap());
    assert!(
        plain.contains("Approve sign-in (https://openagents.com/device)"),
        "{plain}"
    );
}

#[test]
fn the_prompt_describes_the_catalog() {
    let prompt = prompt();
    for component in catalog::CATALOG {
        assert!(
            prompt.contains(&format!("- {}(", component.name)),
            "{}",
            component.name
        );
    }
    assert!(
        prompt.contains("Button(label, href, style?, show?)"),
        "{prompt}"
    );
    assert!(prompt.contains("```openui-lang"));
}
