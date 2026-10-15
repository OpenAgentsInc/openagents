use super::*;

/// This computer's library for `cwd`, as a real chat would read it.
fn live_library(cwd: &Path) -> Library {
    let home = user_home::home_os().map(PathBuf::from).unwrap();
    let memory = crate::memory::Memory::at(
        home.join(".openagents/memory"),
        Some(home.join(".openagents/AGENTS.md")),
        Some(home),
        cwd,
    );
    let dirs = skill_dirs(memory.checkout(), memory.home());
    gather(&memory, &dirs)
}

/// Asks the real doors to choose for a message and prints the choice:
/// `CODER_CONTEXT_LIVE="fix the gym router" cargo test -p coder-new
/// context::tests::live_choice -- --ignored --nocapture`.
#[test]
#[ignore = "asks the real System One doors"]
fn live_choice() {
    let message =
        std::env::var("CODER_CONTEXT_LIVE").unwrap_or_else(|_| "fix the gym router".into());
    let cwd = std::env::current_dir().unwrap();
    let library = live_library(&cwd);
    println!(
        "{} candidates, {} files",
        library.candidates.len(),
        library.files.len()
    );
    let started = Instant::now();
    let answer = if std::env::var("CODER_CONTEXT_DOOR").is_ok_and(|door| door == "gateway") {
        let runtime = tokio::runtime::Runtime::new().unwrap();
        let mut scored = Vec::new();
        let mut door = String::new();
        for (offset, request) in requests(&library, &[], &message) {
            let (response, by) = runtime
                .block_on(crate::issue_run::decide::ask(
                    crate::issue_run::Decider::Gateway,
                    request,
                ))
                .unwrap();
            let end = (offset + BATCH).min(library.candidates.len());
            scored.extend(read(&response, &library.candidates[offset..end]).unwrap());
            door = by;
        }
        Ok(Answered {
            scored,
            model: String::new(),
            door,
            ms: 0,
        })
    } else {
        ask_now(requests(&library, &[], &message), &library.candidates)
    };
    if let Ok(answered) = &answer {
        let mut all = answered.scored.clone();
        all.sort_by(|a, b| b.position.total_cmp(&a.position));
        for s in &all {
            println!("{} {:.2} {}", s.level, s.position, s.id);
        }
    } else {
        println!("{answer:?}");
    }
    println!("answered in {:?}", started.elapsed());
    let (briefing, trace) = decide(answer, &library, started.elapsed());
    println!("{}", serde_json::to_string_pretty(&trace).unwrap());
    if let Some(briefing) = briefing {
        println!("{}", briefing.render(true));
    }
}

/// Lists this repository's instruction-file candidates:
/// `cargo test -p coder-new --lib context::tests::list_repo_candidates -- --ignored --nocapture`.
#[test]
#[ignore = "reads this checkout"]
fn list_repo_candidates() {
    for candidate in repo_library().candidates {
        println!(
            "{}\t{}\t{}\t{}",
            candidate.id,
            candidate.size(),
            candidate.title,
            gist(&candidate.text, 160).replace('\n', " / ")
        );
    }
}

/// The repository root this test binary was built from.
fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap()
}

/// The candidates of this repository's instruction-file tree, the
/// fixture's library: every tracked `AGENTS.md`, split into parts.
fn repo_library() -> Library {
    let root = repo_root();
    let mut library = Library::default();
    for file in ["AGENTS.md", "docs/research/unreal/AGENTS.md"] {
        let path = root.join(file);
        let text = std::fs::read_to_string(&path).unwrap();
        library.candidates.extend(sections(file, &path, &text));
        library.files.push(path);
    }
    library
}

/// The labelled fixture: about twenty requests against this repository's
/// instruction-file tree, each with the parts a good briefing carries
/// marked, and System One's recorded judgments.
const FIXTURE: &str = include_str!("../fixtures/context/context-v1.json");

#[derive(Deserialize, Serialize)]
struct Fixture {
    schema: String,
    question_set: String,
    model: String,
    door: String,
    library: Vec<Sized>,
    requests: Vec<Labelled>,
}

#[derive(Deserialize, Serialize)]
struct Sized {
    id: String,
    size: usize,
}

#[derive(Deserialize, Serialize)]
struct Labelled {
    message: String,
    relevant: Vec<String>,
    #[serde(default)]
    judgments: Vec<Scored>,
}

/// Records System One's judgments for the labelled requests into the
/// fixture: `cargo test -p coder-new --lib context::tests::record_fixture
/// -- --ignored --nocapture`.
#[test]
#[ignore = "asks the real System One doors and rewrites the fixture"]
fn record_fixture() {
    let labels: Vec<Labelled> =
        serde_json::from_str(include_str!("../fixtures/context/labels.json")).unwrap();
    let library = repo_library();
    let mut fixture = Fixture {
        schema: "openagents.coder.context-fixture.v1".into(),
        question_set: QUESTION_SET.into(),
        model: String::new(),
        door: String::new(),
        library: library
            .candidates
            .iter()
            .map(|c| Sized {
                id: c.id.clone(),
                size: c.size(),
            })
            .collect(),
        requests: Vec::new(),
    };
    for mut labelled in labels {
        for id in &labelled.relevant {
            assert!(
                library.candidates.iter().any(|c| &c.id == id),
                "no candidate {id}"
            );
        }
        let answered = ask_now(
            requests(&library, &[], &labelled.message),
            &library.candidates,
        )
        .unwrap();
        println!(
            "{} ms {} — {}",
            answered.ms, answered.door, labelled.message
        );
        fixture.model = answered.model;
        fixture.door = answered.door;
        labelled.judgments = answered.scored;
        fixture.requests.push(labelled);
    }
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures/context/context-v1.json");
    std::fs::write(path, serde_json::to_string_pretty(&fixture).unwrap() + "\n").unwrap();
}

/// Recall and size of the briefings the recorded judgments make, under
/// `select`'s rule; no network.
fn replay(fixture: &Fixture) -> (usize, usize, usize, usize) {
    let candidates: Vec<Candidate> = fixture
        .library
        .iter()
        .map(|sized| Candidate {
            id: sized.id.clone(),
            origin: Origin::Instructions,
            path: PathBuf::from("AGENTS.md"),
            title: String::new(),
            text: " ".repeat(sized.size),
        })
        .collect();
    let (mut found, mut wanted, mut spare, mut answers) = (0, 0, 0, 0);
    for request in &fixture.requests {
        let chosen: Vec<&str> = select(&candidates, &request.judgments, BRIEFING_BYTES)
            .into_iter()
            .map(|index| candidates[index].id.as_str())
            .collect();
        println!("{:?} -> {chosen:?}", request.message);
        if request.relevant.is_empty() {
            answers += 1;
            spare += chosen.len();
        }
        wanted += request.relevant.len();
        found += request
            .relevant
            .iter()
            .filter(|id| chosen.contains(&id.as_str()))
            .count();
    }
    (found, wanted, spare, answers)
}

/// The labelled fixture's selection recall from recorded judgments, with no
/// network: the marked parts a briefing carries, and how little it carries
/// for a question that needs none of them.
#[test]
fn the_fixture_briefings_carry_the_marked_parts() {
    let fixture: Fixture = serde_json::from_str(FIXTURE).unwrap();
    assert_eq!(fixture.question_set, QUESTION_SET);
    assert!(fixture.requests.len() >= 20);
    let (found, wanted, spare, answers) = replay(&fixture);
    println!("recall {found}/{wanted}; {spare} parts over {answers} plain questions");
    // Measured 2026-10-12 on Clef-Flash through the gateway door (24 of 30,
    // 2 parts over 3 plain questions). A lower number is a regression.
    assert!(
        found * 100 >= wanted * RECALL_FLOOR,
        "recall {found}/{wanted}"
    );
    assert!(
        spare <= answers,
        "{spare} parts over {answers} plain questions"
    );
}

/// The least share of marked parts, in percent, the fixture briefings carry.
const RECALL_FLOOR: usize = 80;

const FILE: &str = "# Guide\n\nOpening words.\n\n## Testing\n\nRun `cargo test -p` for touched crates.\n\n```sh\n# not a heading\ncargo fmt\n```\n\n## Copy\n\n### Errors\n\nPlain words in errors.\n\n## Empty\n\n## Copy\n\nA second Copy section.\n";

#[test]
fn an_instruction_file_splits_at_its_headings_outside_code() {
    let parts = sections("AGENTS.md", Path::new("AGENTS.md"), FILE);
    let ids: Vec<&str> = parts.iter().map(|part| part.id.as_str()).collect();
    assert_eq!(
        ids,
        [
            "AGENTS.md#guide",
            "AGENTS.md#testing",
            "AGENTS.md#copy-errors",
            "AGENTS.md#copy",
        ]
    );
    assert_eq!(parts[0].title, "Guide");
    assert!(
        parts[1].text.contains("# not a heading"),
        "fences stay whole"
    );
    assert_eq!(parts[2].title, "Copy › Errors");
    assert_eq!(parts[3].text, "A second Copy section.");
    // The same file splits the same way.
    assert_eq!(parts, sections("AGENTS.md", Path::new("AGENTS.md"), FILE));
}

#[test]
fn a_long_part_is_cut_at_its_list_items_and_named_by_them() {
    let item = |name: &str| format!("- `crates/{name}` — {}\n", "words ".repeat(300));
    let text = format!("## Crates\n\n{}{}{}", item("a"), item("b"), item("c"));
    let parts = sections("AGENTS.md", Path::new("AGENTS.md"), &text);
    assert!(parts.len() >= 2, "{}", parts.len());
    assert!(parts.iter().all(|part| part.text.len() <= PART_MAX));
    assert_eq!(parts[0].id, "AGENTS.md#crates-crates-a");
    assert!(parts[0].title.starts_with("Crates (part 1)"));
    // System One reads each item's first line.
    let excerpt = parts[0].excerpt();
    assert!(excerpt.contains("`crates/a`"));
    assert!(excerpt.chars().count() < EXCERPT_CHARS + 200);
}

fn part(id: &str, size: usize) -> Candidate {
    Candidate {
        id: id.into(),
        origin: Origin::Instructions,
        path: PathBuf::from("/repo/AGENTS.md"),
        title: String::new(),
        text: "x".repeat(size),
    }
}

fn scored(id: &str, position: f64) -> Scored {
    Scored {
        id: id.into(),
        level: 0,
        position,
    }
}

#[test]
fn the_most_relevant_parts_that_fit_are_chosen_in_file_order() {
    let candidates = [
        part("a", 100),
        part("b", 300),
        part("c", 100),
        part("d", 50),
    ];
    let judged = [
        scored("a", 1.2),
        scored("b", 2.9),
        scored("c", 0.4),
        scored("d", 2.0),
    ];
    assert_eq!(select(&candidates, &judged, 10_000), [0, 1, 3]);
    // Under a tight budget the highest go first; one that does not fit
    // leaves room for a smaller one below it.
    assert_eq!(select(&candidates, &judged, 360), [1, 3]);
    assert!(select(&candidates, &[], 10_000).is_empty());
}

fn library() -> Library {
    let mut candidates = sections("AGENTS.md", Path::new("/repo/AGENTS.md"), FILE);
    candidates.push(Candidate {
        id: "memory:user/prefers-small-commits".into(),
        origin: Origin::Memory,
        path: PathBuf::from("/memory/user/prefers.md"),
        title: "Prefers small commits (feedback)".into(),
        text: "Small commits\nOne change per commit.".into(),
    });
    candidates.push(Candidate {
        id: "memory:user/likes-tea".into(),
        origin: Origin::Memory,
        path: PathBuf::from("/memory/user/tea.md"),
        title: "Likes tea (user)".into(),
        text: "Tea\nGreen.".into(),
    });
    candidates.push(Candidate {
        id: "skill:release".into(),
        origin: Origin::Skill,
        path: PathBuf::from("/repo/.agents/skills/release/SKILL.md"),
        title: "release".into(),
        text: "Ship a build to TestFlight.".into(),
    });
    Library {
        candidates,
        files: vec![PathBuf::from("/repo/AGENTS.md")],
        nested: vec![PathBuf::from("/repo/docs/AGENTS.md")],
    }
}

/// The judgments a test's System One gives: `chosen` placed relevant,
/// everything else unrelated.
pub(crate) fn judged(library: &Library, chosen: &[&str]) -> Vec<Scored> {
    library
        .candidates
        .iter()
        .map(|c| Scored {
            id: c.id.clone(),
            level: if chosen.contains(&c.id.as_str()) {
                2
            } else {
                0
            },
            position: if chosen.contains(&c.id.as_str()) {
                2.1
            } else {
                0.2
            },
        })
        .collect()
}

fn answered(library: &Library, chosen: &[&str]) -> Answered {
    Answered {
        scored: judged(library, chosen),
        model: "clef-flash".into(),
        door: "gateway".into(),
        ms: 40,
    }
}

#[test]
fn the_briefing_carries_the_chosen_parts_and_names_the_rest() {
    let library = library();
    let (briefing, trace) = decide(
        Ok(answered(
            &library,
            &[
                "AGENTS.md#testing",
                "memory:user/prefers-small-commits",
                "skill:release",
            ],
        )),
        &library,
        Duration::ZERO,
    );
    let briefing = briefing.unwrap();
    let text = briefing.render(true);
    assert!(text.contains("### Testing\n\nRun `cargo test -p`"));
    assert!(
        !text.contains("Plain words in errors"),
        "unrelated parts stay out"
    );
    assert!(text.contains("Read a file when something in it may matter: /repo/AGENTS.md"));
    assert!(text.contains("/repo/docs/AGENTS.md"));
    assert!(text.contains(
        "- Prefers small commits (feedback):\n  Small commits\n  One change per commit."
    ));
    assert!(text.contains("left out as unrelated to this work: Likes tea (user). Use recall"));
    assert!(text.contains(crate::memory::TOOLS_GUIDE));
    assert!(text.contains(
        "- release: Ship a build to TestFlight. (/repo/.agents/skills/release/SKILL.md)"
    ));
    assert!(!briefing.render(false).contains("Use recall"));
    assert_eq!(trace.question_set, QUESTION_SET);
    assert_eq!(
        trace
            .selected
            .iter()
            .map(|s| s.id.as_str())
            .collect::<Vec<_>>(),
        [
            "AGENTS.md#testing",
            "memory:user/prefers-small-commits",
            "skill:release"
        ]
    );
    assert_eq!(trace.model.as_deref(), Some("clef-flash"));
    assert_eq!(trace.door.as_deref(), Some("gateway"));
    // No answer: no briefing, and the trace says why.
    let (none, missing) = decide(
        Err("timed out".into()),
        &library,
        Duration::from_millis(2000),
    );
    assert!(none.is_none());
    assert_eq!(missing.ms, 2000);
    assert!(missing.missing.is_some());
}

#[test]
fn a_library_gathers_instruction_parts_notes_and_skills() {
    let dir = tempfile::tempdir().unwrap();
    let home = dir.path().join("home");
    let repo = home.join("repo");
    std::fs::create_dir_all(repo.join(".git")).unwrap();
    std::fs::create_dir_all(repo.join(".agents/skills/release")).unwrap();
    std::fs::write(repo.join("AGENTS.md"), FILE).unwrap();
    std::fs::write(
        repo.join(".agents/skills/release/SKILL.md"),
        "---\nname: release\ndescription: Ship a build.\n---\n\nSteps.\n",
    )
    .unwrap();
    let memory = crate::memory::Memory::at(home.join("memory"), None, Some(home.clone()), &repo);
    memory
        .remember(&crate::memory::Note {
            name: "Prefers small commits".into(),
            kind: "feedback".into(),
            body: "One change per commit.".into(),
            description: Some("Small commits".into()),
            scope: None,
        })
        .unwrap();
    let library = gather(&memory, &skill_dirs(memory.checkout(), memory.home()));
    let ids: Vec<&str> = library.candidates.iter().map(|c| c.id.as_str()).collect();
    assert_eq!(
        ids,
        [
            "AGENTS.md#guide",
            "AGENTS.md#testing",
            "AGENTS.md#copy-errors",
            "AGENTS.md#copy",
            "memory:user/prefers-small-commits",
            "skill:release",
        ]
    );
    assert_eq!(library.files.len(), 1);
    let skill = &library.candidates[5];
    assert_eq!(skill.text, "Ship a build.");
    // The questions: one Score per candidate, in order.
    let questions = questions(&library.candidates);
    assert_eq!(questions.len(), 6);
    assert!(questions.get("c5").is_some());
}

/// A gateway door asks at most 64 questions; a large library goes in
/// batches whose answers line up with their candidates.
#[test]
fn a_large_library_is_asked_in_batches() {
    let library = Library {
        candidates: (0..BATCH + 5).map(|n| part(&format!("p{n}"), 10)).collect(),
        ..Library::default()
    };
    let batches = requests(&library, &[], "hello");
    assert_eq!(batches.len(), 2);
    assert_eq!(batches[1].0, BATCH);
}

fn record_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "coder-context-test-{name}-{}-{}",
        std::process::id(),
        atif::now_ms()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    dir
}

fn facts(claude: bool) -> crate::harness_plugin::Facts {
    let health = crate::harness_plugin::Health {
        signed_in: claude,
        at_limit: false,
    };
    crate::harness_plugin::Facts {
        local: true,
        claude_plugin_on: claude,
        claude: health,
        codex_on: false,
        codex: health,
        opencode_on: false,
        opencode: crate::harness_plugin::Health {
            signed_in: false,
            at_limit: false,
        },
        openrouter_key: false,
        leaning: crate::routing_preferences::Leaning::default(),
        failed: Default::default(),
    }
}

fn finish(app: &mut crate::App) {
    let id = app.request_id;
    app.apply_update(crate::live::Update::Finished {
        id,
        result: Ok(openrouter::Streamed {
            text: "done".into(),
            model: "m".into(),
            ..Default::default()
        }),
    });
    app.request = None;
}

fn route_answer(new_work: Option<f64>) -> crate::router::Recorded {
    let map = |pairs: &[(&str, f64)]| {
        pairs
            .iter()
            .map(|(name, p)| ((*name).to_owned(), *p))
            .collect()
    };
    crate::router::Recorded {
        kind: map(&[("repo_change", 0.9), ("answer", 0.1)]),
        size: map(&[("medium", 0.9), ("small", 0.1)]),
        needs_repo: 0.9,
        own_tools: 0.0,
        risk: map(&[("low", 0.9), ("medium", 0.1)]),
        new_work,
    }
}

fn chosen_lines(dir: &Path) -> Vec<crate::routing_record::ChoseLine> {
    crate::routing_record::read(dir)
        .into_iter()
        .filter_map(|line| match line {
            crate::routing_record::Line::Chose(chose) => Some(chose),
            _ => None,
        })
        .collect()
}

/// The Coder loop's path: the first message waits for its briefing, the
/// turn reads it in place of every file and note, a continuation keeps
/// it, new work chooses again, and every record carries the choice.
#[test]
fn the_coder_loop_reads_the_briefing_chosen_per_piece_of_work() {
    let dir = record_dir("loop");
    crate::harness_plugin::sign_in_claude(false);
    crate::routing_record::record_to(Some((dir.clone(), facts(false))));
    let library = library();
    set_test_library(Some(library.clone()));
    set_test_judge(Some(Some(judged(&library, &["AGENTS.md#testing"]))));
    let mut app = crate::App::default();
    app.set_mode(crate::Mode::Live);
    app.draft.text = "fix the failing test".into();
    app.submit_live();
    assert!(
        app.routing_pending(),
        "the first message waits for its briefing"
    );
    app.poll_route();
    assert!(!app.routing_pending());
    let briefing = app.live.briefing.clone().expect("a briefing was chosen");
    assert_eq!(briefing.key(), "AGENTS.md#testing");
    let Some(crate::live::Work::Microcoder {
        messages,
        execution,
        ..
    }) = app.request.take().map(|request| request.kind)
    else {
        panic!("a Coder loop turn");
    };
    assert_eq!(execution.briefing.as_ref(), Some(&briefing));
    let task = crate::live::local_task(&messages, &execution).unwrap();
    assert!(task.contains("### Testing\n\nRun `cargo test -p`"));
    assert!(!task.contains("Plain words in errors"));
    finish(&mut app);

    // A continuation keeps the briefing: the router asks only whether it
    // is new work, and no selection runs.
    crate::router::set_test_judge(Some(Some(route_answer(Some(0.1)))));
    set_test_judge(Some(Some(judged(&library, &["AGENTS.md#copy-errors"]))));
    app.draft.text = "also run it twice".into();
    app.submit_live();
    app.poll_route();
    assert_eq!(
        app.live.briefing.as_ref().map(|b| b.key()).as_deref(),
        Some("AGENTS.md#testing")
    );
    finish(&mut app);

    // New work chooses again.
    crate::router::set_test_judge(Some(Some(route_answer(Some(0.9)))));
    app.draft.text = "now reword the error messages".into();
    app.submit_live();
    app.poll_route();
    assert_eq!(
        app.live.briefing.as_ref().map(|b| b.key()).as_deref(),
        Some("AGENTS.md#copy-errors")
    );
    finish(&mut app);

    crate::router::set_test_judge(None);
    set_test_judge(None);
    set_test_library(None);
    crate::routing_record::record_to(None);
    let chose = chosen_lines(&dir);
    let _ = std::fs::remove_dir_all(&dir);
    let contexts: Vec<(Vec<String>, bool)> = chose
        .iter()
        .map(|line| {
            let context = line
                .inputs
                .context
                .clone()
                .expect("the record keeps the choice");
            (
                context.selected.into_iter().map(|s| s.id).collect(),
                context.kept,
            )
        })
        .collect();
    assert_eq!(
        contexts,
        [
            (vec!["AGENTS.md#testing".to_owned()], false),
            (vec!["AGENTS.md#testing".to_owned()], true),
            (vec!["AGENTS.md#copy-errors".to_owned()], false),
        ]
    );
    let first = chose[0].inputs.context.as_ref().unwrap();
    assert_eq!(first.question_set, QUESTION_SET);
    assert_eq!(first.model.as_deref(), Some("test"));
    assert_eq!(first.door.as_deref(), Some("test"));
}

/// No answer in time: no briefing, and the loop loads what it loaded
/// before; the record says why.
#[test]
fn without_an_answer_each_harness_loads_what_it_loaded_before() {
    let dir = record_dir("silent");
    crate::harness_plugin::sign_in_claude(false);
    crate::routing_record::record_to(Some((dir.clone(), facts(false))));
    set_test_library(Some(library()));
    set_test_judge(Some(None));
    let mut app = crate::App::default();
    app.set_mode(crate::Mode::Live);
    app.draft.text = "hello".into();
    app.submit_live();
    app.settle_route();
    assert!(app.live.briefing.is_none());
    assert!(matches!(
        app.request.as_ref().map(|request| &request.kind),
        Some(crate::live::Work::Microcoder { execution, .. }) if execution.briefing.is_none()
    ));
    set_test_judge(None);
    set_test_library(None);
    crate::routing_record::record_to(None);
    let chose = chosen_lines(&dir);
    let _ = std::fs::remove_dir_all(&dir);
    let context = chose[0].inputs.context.as_ref().unwrap();
    assert!(context.selected.is_empty());
    assert!(context.missing.is_some());
}

/// Claude Code's turn carries the same briefing: appended to its system
/// prompt, with its own loading of those instruction files off.
#[test]
fn claude_code_reads_the_same_briefing() {
    crate::harness_plugin::sign_in_claude(true);
    let library = library();
    set_test_library(Some(library.clone()));
    set_test_judge(Some(Some(judged(
        &library,
        &["AGENTS.md#testing", "skill:release"],
    ))));
    crate::router::set_test_judge(Some(Some(route_answer(None))));
    let mut app = crate::App::default();
    app.set_mode(crate::Mode::Live);
    app.plugins.bundled.claude = true;
    app.draft.text = "fix the failing test".into();
    app.submit_live();
    app.poll_route();
    crate::router::set_test_judge(None);
    set_test_judge(None);
    set_test_library(None);
    crate::harness_plugin::sign_in_claude(false);
    assert_eq!(
        app.live.runs_on,
        Some(crate::harness_plugin::Harness::ClaudeCode)
    );
    let Some(crate::live::Work::Claude { briefing, .. }) =
        app.request.take().map(|request| request.kind)
    else {
        panic!("a Claude Code turn");
    };
    let briefing = *briefing.expect("the turn carries the briefing");
    assert_eq!(Some(&briefing), app.live.briefing.as_ref());
    let options = crate::claude::briefed(claude_agent_sdk::QueryOptions::new(), Some(&briefing));
    let Some(claude_agent_sdk::SystemPromptConfig::Preset {
        append: Some(append),
    }) = &options.system_prompt
    else {
        panic!("an appended system prompt");
    };
    assert_eq!(append, &briefing.render(false));
    assert!(append.contains("### Testing") && append.contains("- release:"));
    assert_eq!(
        options.settings,
        Some(serde_json::json!({"claudeMdExcludes": ["/repo/AGENTS.md", "/repo/CLAUDE.md"]}))
    );
    // A resumed session keeps Coder's note first.
    let mut resumed = claude_agent_sdk::QueryOptions::new();
    resumed.system_prompt = Some(claude_agent_sdk::SystemPromptConfig::Preset {
        append: Some("NOTE".into()),
    });
    let resumed = crate::claude::briefed(resumed, Some(&briefing));
    assert!(matches!(
        &resumed.system_prompt,
        Some(claude_agent_sdk::SystemPromptConfig::Preset { append: Some(text) })
            if text.starts_with("NOTE\n\n# Project instructions")
    ));
    // New work in a running process: the briefing rides on the message.
    let message = crate::claude::with_briefing(&briefing, "next");
    assert!(message.starts_with("<briefing>") && message.ends_with("</briefing>\n\nnext"));
    // No briefing: Claude Code loads its own files as before.
    let plain = crate::claude::briefed(claude_agent_sdk::QueryOptions::new(), None);
    assert!(plain.system_prompt.is_none() && plain.settings.is_none());
}
