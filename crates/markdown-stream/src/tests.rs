use super::renderable;
use pulldown_cmark::{Event, Options, Parser, html};

fn cut(text: &str) -> String {
    renderable(text).into_owned()
}

/// What the reader sees: HTML as the web chat renders it (raw HTML as
/// text), tags stripped, entities decoded.
fn visible(source: &str) -> String {
    let options = Options::ENABLE_TABLES | Options::ENABLE_STRIKETHROUGH;
    let events = Parser::new_ext(source, options).map(|event| match event {
        Event::Html(raw) | Event::InlineHtml(raw) => Event::Text(raw),
        other => other,
    });
    let mut markup = String::new();
    html::push_html(&mut markup, events);
    let mut text = String::new();
    let mut tag = false;
    for c in markup.chars() {
        match c {
            '<' => tag = true,
            '>' if tag => tag = false,
            c if !tag => text.push(c),
            _ => {}
        }
    }
    text.replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&amp;", "&")
}

// Each case from #11112, with what the stream shows mid-construct.

#[test]
fn an_open_code_fence_closes_and_shows_the_code_so_far() {
    assert_eq!(
        cut("Here:\n\n```rust\nfn main() {\n    let x"),
        "Here:\n\n```rust\nfn main() {\n    let x\n```"
    );
    // A last line that may be the closing fence is held back.
    assert_eq!(cut("```\ncode\n``"), "```\ncode\n```");
    assert_eq!(cut("~~~~\na\n"), "~~~~\na\n~~~~");
    // The fence's opening line waits for its newline.
    assert_eq!(cut("Text\n\n```ru"), "Text\n\n");
    // Inside a list item or quote the close keeps the indentation.
    assert_eq!(cut("- a\n  ```\n  x"), "- a\n  ```\n  x\n  ```");
    assert_eq!(cut("> ```\n> x\n"), "> ```\n> x\n> ```");
    // A finished fence is left alone.
    assert_eq!(cut("```\nx\n```\nafter"), "```\nx\n```\nafter");
}

#[test]
fn a_half_table_row_waits_and_a_header_waits_for_its_delimiter() {
    let table = "| a | b |\n|---|---|\n| 1 | 2 |\n";
    assert_eq!(cut(&format!("{table}| 3 |")), table);
    assert_eq!(cut(table), table);
    assert_eq!(cut("Intro\n\n| a | b |\n"), "Intro\n\n");
    assert_eq!(cut("Intro\n\n| a | b |\n|--"), "Intro\n\n");
    assert_eq!(cut("Intro\n\n| a | b"), "Intro\n\n");
    assert_eq!(
        cut("| a | b |\n|---|---|\n"),
        "| a | b |\n|---|---|\n",
        "a header and its delimiter draw as an empty table"
    );
}

#[test]
fn a_link_waits_for_its_destination() {
    assert_eq!(cut("See [the docs]("), "See ");
    assert_eq!(cut("See [the docs](https://openagents.com/do"), "See ");
    assert_eq!(cut("See [the docs"), "See ");
    assert_eq!(cut("See ![a chart](/static/x"), "See ");
    let done = "See [the docs](https://openagents.com/docs) now";
    assert_eq!(cut(done), done);
    let auto = "Mail <hello@openag";
    assert_eq!(cut(auto), "Mail ");
}

#[test]
fn an_open_list_item_waits_for_its_text() {
    assert_eq!(cut("Steps:\n\n1. One\n2."), "Steps:\n\n1. One\n");
    assert_eq!(cut("- a\n- "), "- a\n");
    assert_eq!(cut("- a\n- b"), "- a\n- b");
    // A bold lead-in waits with its bullet, not as an empty bullet.
    assert_eq!(cut("- a\n- **No"), "- a\n");
    assert_eq!(cut("- a\n- **Note**: x"), "- a\n- **Note**: x");
}

#[test]
fn unclosed_emphasis_waits_until_it_closes() {
    assert_eq!(cut("**bold"), "");
    assert_eq!(cut("Say **bold"), "Say ");
    assert_eq!(cut("Say *it"), "Say ");
    assert_eq!(cut("Say ~~gone"), "Say ");
    assert_eq!(cut("Say **"), "Say ");
    assert_eq!(cut("Say `cargo te"), "Say ");
    for settled in [
        "Say **bold** now",
        "2 * 3 = 6",
        "snake_case and a_b",
        "about ~5 minutes",
    ] {
        assert_eq!(cut(settled), settled);
    }
    // A finished paragraph is settled even with a stray marker.
    assert_eq!(cut("a **b\n\nnext"), "a **b\n\nnext");
}

#[test]
fn a_half_written_heading_waits_for_its_text() {
    assert_eq!(cut("##"), "");
    assert_eq!(cut("Intro\n\n## "), "Intro\n\n");
    assert_eq!(cut("Intro\n\n## Hea"), "Intro\n\n## Hea");
    assert_eq!(cut("Intro\n\n## **Hea"), "Intro\n\n");
    // A setext underline still being written doesn't turn the paragraph
    // into a heading and back.
    assert_eq!(cut("Title\n-"), "Title\n");
    assert_eq!(cut("Title\n==="), "Title\n");
}

#[test]
fn partial_plain_text_and_raw_html_stay() {
    assert_eq!(cut("Hello wor"), "Hello wor");
    assert_eq!(cut("Hi <b>x</b> y"), "Hi <b>x</b> y");
    // A tag still being written waits.
    assert_eq!(cut("Hi <scr"), "Hi ");
}

const FIXTURES: [&str; 3] = [
    "# Setting up the project\n\nTo start, **install the toolchain** and run the *checks*. See [the guide](https://openagents.com/docs/install) or the [FAQ](/docs/faq).\n\n## Steps\n\n1. Clone the repository.\n2. Run `cargo build --release`.\n3. Start the server:\n\n   ```sh\n   ./target/release/server --port 8080\n   ```\n\n4. Open <http://localhost:8080>.\n\n- **Fast**: builds in ~2 minutes.\n- *Safe*: no `unsafe` code.\n- ~~Old~~ New flags.\n\n> Note: on Windows use `wsl`.\n> It works the same.\n\n| Command | What it does | Time |\n| --- | :-: | ---: |\n| `build` | Compiles **everything** | 2 min |\n| `test` | Runs [tests](https://x.example) | 5 min |\n| `fmt` | Formats | 1 s |\n\n---\n\nThat's all. 2 * 3 = 6, snake_case_names stay, and <div>raw html</div> shows as text.\n",
    "Here is the fix:\n\n```rust\nfn main() {\n    let items = vec![1, 2, 3];\n    // `sum` is **not** bold here\n    println!(\"{}\", items.iter().sum::<i32>());\n}\n```\n\nAnd in Python:\n\n~~~python\nprint(sum([1, 2, 3]))\n~~~\n\nWhy it works\n------------\n\nThe iterator *borrows* the vector; `sum` consumes the iterator. Compare:\n\n| Language | Expression |\n|---|---|\n| Rust | `items.iter().sum()` |\n| Python | `sum(items)` |\n\n* one\n* two\n  * nested **deep** item\n  * another [link](https://example.com/a_b_c)\n\n![diagram](/static/verse-grid.jpg)\n\nDone.",
    "### Short answer\n\nYes. ___Really___ yes, with __strong__ and _em_.\n\n```\nplain fence\n```\n\n- [ ] not a task list here\n- item with trailing spaces  \n  continued line\n\n1) first\n2) second\n\nA final line with a [ref][x] and an escaped \\*star\\*.\n",
];

/// The chat's canned answers (`crates/coder/answers`), as the replies the
/// web chat streams.
fn bank() -> Vec<String> {
    include_str!("../../coder/answers/chat-answers-v1.toml")
        .lines()
        .filter_map(|line| line.strip_prefix("text = \"")?.strip_suffix('"'))
        .map(|text| text.replace("\\n", "\n").replace("\\\"", "\""))
        .collect()
}

/// Every cut point of every answer renders without syntax the finished
/// answer doesn't show, and what the reader sees never shrinks as text
/// arrives.
#[test]
fn every_cut_point_renders_without_raw_syntax() {
    let mut answers: Vec<String> = FIXTURES.iter().map(|s| (*s).to_owned()).collect();
    answers.extend(bank());
    assert!(answers.len() > 50, "the answer bank loads");
    for answer in &answers {
        let finished = visible(answer);
        let mut seen = 0;
        let mut shown = String::new();
        for (cut_at, _) in answer.char_indices().chain([(answer.len(), ' ')]) {
            let prefix = renderable(&answer[..cut_at]);
            let now = visible(&prefix);
            for mark in ['*', '_', '`', '[', ']', '|', '#', '~', '<', '>', '!'] {
                let leaked = now.matches(mark).count();
                let allowed = finished.matches(mark).count();
                assert!(
                    leaked <= allowed,
                    "{mark:?} leaks at {cut_at}: {:?}\nshows {now:?}",
                    &answer[..cut_at]
                );
            }
            let size = now.trim_end().chars().count();
            assert!(
                size + 1 >= seen,
                "the reply shrank at {cut_at}: {:?}\nwas {shown:?}\nnow {now:?}",
                &answer[..cut_at]
            );
            seen = seen.max(size);
            shown = now;
        }
    }
}
