use super::*;
use openagents_chat::basic_chats::Spawned;

fn row(id: &str, title: &str, updated: u64, project: Option<&str>) -> Summary {
    Summary {
        id: id.repeat(32 / id.len()),
        title: title.into(),
        started: 1,
        updated,
        coder: project.map(|project| Spawned {
            host: "local".into(),
            task: "t1".into(),
            project: Some(project.into()),
            at: None,
        }),
        archived: false,
        pinned: false,
        named: false,
    }
}

fn key(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::NONE)
}

fn ctrl(c: char) -> KeyEvent {
    KeyEvent::new(KeyCode::Char(c), KeyModifiers::CONTROL)
}

fn picker() -> Picker {
    Picker::new(
        vec![
            row("a", "Lunch plans", 50, None),
            row("b", "Fix the parser", 40, Some("demo")),
            row("c", "Zebra docs", 30, Some("zoo")),
            row("d", "Apple docs", 20, Some("apple")),
            row("e", "Parser docs", 60, Some("demo")),
        ],
        "demo",
    )
}

fn labels(picker: &Picker) -> Vec<String> {
    picker
        .entries()
        .iter()
        .map(|entry| match entry {
            Entry::Header(name) => format!("# {name}"),
            Entry::Row(row) => row.title.clone(),
        })
        .collect()
}

#[test]
fn rows_group_by_project_this_folders_first_then_chats_then_alphabetical() {
    let picker = picker();
    assert_eq!(
        labels(&picker),
        [
            "# demo",
            "Parser docs",
            "Fix the parser",
            "# Chats",
            "Lunch plans",
            "# apple",
            "Apple docs",
            "# zoo",
            "Zebra docs",
        ]
    );
    assert_eq!(picker.current().unwrap().title, "Parser docs");
}

#[test]
fn arrows_skip_headers_and_the_edges_reach_search() {
    let mut picker = picker();
    picker.key(&key(KeyCode::Down));
    picker.key(&key(KeyCode::Down));
    assert_eq!(picker.current().unwrap().title, "Lunch plans");
    picker.key(&key(KeyCode::Char('k')));
    assert_eq!(picker.current().unwrap().title, "Fix the parser");
    picker.key(&key(KeyCode::Up));
    picker.key(&key(KeyCode::Up));
    assert!(picker.search && picker.hidden);
    // Down from search goes back to the first row.
    picker.key(&key(KeyCode::Down));
    assert!(!picker.search);
    assert_eq!(picker.current().unwrap().title, "Parser docs");
}

#[test]
fn typing_searches_expands_what_it_finds_and_esc_clears_then_closes() {
    let mut picker = picker();
    for c in "PARSER".chars() {
        assert_eq!(picker.key(&key(KeyCode::Char(c))), Picked::Nothing);
    }
    assert!(picker.search);
    assert_eq!(labels(&picker), ["# demo", "Parser docs", "Fix the parser"]);
    assert_eq!(picker.expanded.len(), 2);
    // Esc leaves search; Esc again clears the query; Esc again closes.
    assert_eq!(picker.key(&key(KeyCode::Esc)), Picked::Nothing);
    assert!(!picker.search);
    assert_eq!(picker.key(&key(KeyCode::Esc)), Picked::Nothing);
    assert!(picker.query.is_empty() && picker.expanded.is_empty());
    assert_eq!(picker.key(&key(KeyCode::Esc)), Picked::Close);
}

#[test]
fn an_id_prefix_finds_its_thread() {
    let picker = picker().with_query("ddd");
    assert_eq!(labels(&picker), ["# apple", "Apple docs"]);
}

#[test]
fn enter_opens_and_the_other_keys_act_on_the_selected_row() {
    let mut picker = picker();
    let first = picker.current().unwrap().id.clone();
    assert_eq!(
        picker.key(&key(KeyCode::Char('y'))),
        Picked::Copy(first.clone())
    );
    assert_eq!(picker.key(&ctrl('a')), Picked::Archive(first.clone()));
    assert_eq!(picker.key(&ctrl('n')), Picked::New);
    picker.key(&key(KeyCode::Char('e')));
    assert!(picker.expanded.contains(&first));
    picker.key(&key(KeyCode::Left));
    assert!(picker.expanded.is_empty());
    picker.key(&key(KeyCode::Right));
    picker.key(&key(KeyCode::Right));
    assert!(picker.expanded.is_empty(), "Right toggles");
    assert_eq!(picker.key(&key(KeyCode::Enter)), Picked::Open(first));
    // `/` enters search, and Enter there picks the selected row.
    picker.key(&key(KeyCode::Char('/')));
    for c in "lunch".chars() {
        picker.key(&key(KeyCode::Char(c)));
    }
    assert_eq!(
        picker.key(&key(KeyCode::Enter)),
        Picked::Open("a".repeat(32))
    );
}

#[test]
fn a_whole_id_with_no_row_still_opens() {
    let id = "f".repeat(32);
    let mut whole = picker().with_query(&id);
    assert_eq!(whole.key(&key(KeyCode::Enter)), Picked::Open(id));
    let mut none = picker().with_query("nothing here");
    assert_eq!(none.key(&key(KeyCode::Enter)), Picked::Nothing);
}

#[test]
fn resume_takes_an_id_a_title_or_an_id_prefix() {
    let mut rows = vec![
        row("a", "Lunch plans", 1, None),
        row("b", "Same", 2, None),
        row("c", "same", 3, None),
        row("ab", "Other", 4, None),
    ];
    let id = |row: Option<&Summary>| row.map(|row| row.id.clone());
    assert_eq!(id(resolve(&rows, &"a".repeat(32))), Some("a".repeat(32)));
    assert_eq!(id(resolve(&rows, &"9".repeat(32))), None);
    assert_eq!(id(resolve(&rows, " LUNCH plans ")), Some("a".repeat(32)));
    // Two titles alike: ambiguous, until one of them was renamed.
    assert_eq!(id(resolve(&rows, "same")), None);
    rows[2].named = true;
    assert_eq!(id(resolve(&rows, "same")), Some("c".repeat(32)));
    assert_eq!(id(resolve(&rows, "bbbb")), Some("b".repeat(32)));
    // "ab" starts both `abab…` and nothing else; "a" starts two IDs.
    assert_eq!(id(resolve(&rows, "aba")), Some("ab".repeat(16)));
    assert_eq!(id(resolve(&rows, "a")), None);
    assert_eq!(id(resolve(&rows, "")), None);
}

#[test]
fn times_read_as_grok_build_writes_them() {
    assert_eq!(time_ago(100, 130), "just now");
    assert_eq!(time_ago(0, 300), "  5m ago");
    assert_eq!(time_ago(0, 3 * 3_600), "  3h ago");
    assert_eq!(time_ago(0, 2 * 86_400), "  2d ago");
    assert_eq!(time_ago(0, 65 * 86_400), " 2mo ago");
    assert_eq!(when(0), "Jan 01, 00:00 UTC");
    assert_eq!(when(1_790_000_000), "Sep 21, 14:13 UTC");
}

fn drawn(picker: &Picker, width: u16, height: u16) -> String {
    let area = Rect::new(0, 0, width, height);
    let mut buf = Buffer::empty(area);
    picker.render(area, &mut buf, Ladder::default(), &"e".repeat(32), 1_000);
    (0..height)
        .map(|y| {
            (0..width)
                .map(|x| buf[(x, y)].symbol().to_owned())
                .collect::<String>()
                .trim_end()
                .to_owned()
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn it_draws_the_title_the_search_hint_the_groups_and_the_rows() {
    let mut picker = picker();
    let screen = drawn(&picker, 90, 20);
    for text in [
        "Resume thread",
        "/ to search",
        " demo ──",
        " Chats ──",
        "› Parser docs · open · Coder",
        "› Lunch plans",
        "15m ago",
        "Enter select",
    ] {
        assert!(screen.contains(text), "{text:?} not in:\n{screen}");
    }
    picker.key(&key(KeyCode::Char('e')));
    let screen = drawn(&picker, 90, 20);
    for text in ["◆ Parser docs", "ID", &"e".repeat(32), "Project", "demo"] {
        assert!(screen.contains(text), "{text:?} not in:\n{screen}");
    }
    picker.key(&key(KeyCode::Char('p')));
    let screen = drawn(&picker, 90, 20);
    assert!(screen.contains(" search: p"), "{screen}");
    let empty = drawn(&Picker::new(Vec::new(), "x"), 90, 20);
    assert!(empty.contains("No threads yet."), "{empty}");
}

#[test]
fn the_selected_row_stays_in_view_and_tiny_areas_never_panic() {
    let rows = (0..40)
        .map(|n| row(&format!("{n:02x}"), &format!("Thread {n}"), n, None))
        .collect();
    let mut picker = Picker::new(rows, "x");
    for _ in 0..30 {
        picker.key(&key(KeyCode::Down));
    }
    let title = picker.current().unwrap().title.clone();
    assert!(drawn(&picker, 80, 16).contains(&title));
    for (width, height) in [(0, 0), (1, 1), (5, 3), (8, 4), (12, 6), (30, 5)] {
        drawn(&picker, width, height);
    }
}
