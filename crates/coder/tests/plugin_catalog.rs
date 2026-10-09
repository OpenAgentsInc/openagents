//! The plugins people see are Coder's built-in plugins
//! (`coder::builtin_plugins`), and nothing else.
//!
//! The hosted runner's catalog (`deploy/eval-runner/catalog`) lists the
//! sample packages under `crates/plugin-*`. They are the runner's test
//! fixtures and are never shown. Everything the chat says about which
//! plugins there are follows the built-in list:
//!
//! - `knowledge/openagents/openagents.plugin-list.md`, the note that
//!   answers "which plugins are there?", generated from the list;
//! - the table of plugins in `docs/plugins/README.md`;
//! - the website's plugin cards (`docs/web/plugin-card.md`).
//!
//! No product note is tagged `tool`, so the Gym offers no sample plugin to
//! test. Rewrite the generated note with
//!
//! ```text
//! PLUGIN_LIST_WRITE=1 cargo test -p coder --test plugin_catalog
//! ```
//!
//! and then the route map's snapshot (`ROUTE_MAP_WRITE=1 cargo test -p
//! coder --test route_map_sources`).

use std::path::{Path, PathBuf};

use coder::builtin_plugins::{BUILTIN_PLUGINS, BuiltinPlugin};
use coder::gym_kb;
use knowledge::product::Corpus;

/// The sample plugins' names, which no surface may show.
const SAMPLES: [&str; 6] = [
    "Project map",
    "Code finder",
    "Test reader",
    "Explain this error",
    "Release notes",
    "Dependency check",
];

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// `a`, `a and b`, or `a, b, and c`.
fn series(names: &[&str]) -> String {
    match names {
        [] => String::new(),
        [one] => (*one).to_string(),
        [one, two] => format!("{one} and {two}"),
        [rest @ .., last] => format!("{}, and {last}", rest.join(", ")),
    }
}

fn count(n: usize) -> String {
    const WORDS: [&str; 13] = [
        "no", "one", "two", "three", "four", "five", "six", "seven", "eight", "nine", "ten",
        "eleven", "twelve",
    ];
    WORDS
        .get(n)
        .map_or_else(|| n.to_string(), |word| (*word).to_string())
}

const LIST_ID: &str = "openagents.plugin-list";
const LIST_SOURCE: &str = "crates/coder/src/builtin_plugins.rs";

fn list_path() -> PathBuf {
    root()
        .join("knowledge/openagents")
        .join(format!("{LIST_ID}.md"))
}

/// The plugin list note for `plugins`, at `version`.
fn plugin_list(plugins: &[BuiltinPlugin], version: u64) -> String {
    let names: Vec<&str> = plugins.iter().map(|p| p.name).collect();
    let answer = format!(
        "Coder comes with {} built-in plugins: {}. With the coding agents, Coder hands a task to \
         Claude Code, Codex, Cursor, or Grok Build on your computer, when you have it, and shows \
         its progress as it works. With OpenRouter, Coder uses OpenRouter models with your own \
         API key. In the openagents terminal, /plugins turns each on or off.",
        count(plugins.len()),
        series(&names)
    );
    let mut cites = vec![LIST_SOURCE.to_string()];
    for plugin in plugins {
        for path in plugin.evidence {
            if !cites.iter().any(|cite| cite == path) {
                cites.push((*path).to_string());
            }
        }
    }
    let mut out = String::new();
    out.push_str(&format!(
        "---\nid: {LIST_ID}\nversion: {version}\nkind: product\ntitle: \"Which plugins there are\"\n"
    ));
    out.push_str(&format!(
        "summary: >-\n  Coder's built-in plugins: {}.\n",
        series(&names)
    ));
    out.push_str("tags: [plugins, catalog, coder]\n");
    out.push_str(
        "applies_when: >-\n  The user asks which plugins there are, which plugins Coder has, \
         or which plugins they can use or test with Coder.\n",
    );
    out.push_str(&format!("answer: >-\n  {answer}\n"));
    out.push_str("status: admitted\nauthor: openagents\nprovenance:\n  written_from: [reference]\n  cites:\n");
    for cite in &cites {
        out.push_str(&format!("    - {cite}\n"));
    }
    out.push_str(
        "evidence:\n  - \"Generated from Coder's built-in plugins (crates/coder/src/builtin_plugins.rs) \
         by crates/coder/tests/plugin_catalog.rs; PLUGIN_LIST_WRITE=1 rewrites it, and its version \
         moves when its words do. The hosted runner's sample plugins are test fixtures and are \
         not listed.\"\n---\n\n",
    );
    out.push_str(&format!("## Answer\n\n{answer}\n\n## Details\n\n"));
    for plugin in plugins {
        out.push_str(&format!("- **{}**: {}\n", plugin.name, plugin.summary));
    }
    out.push_str("\n## Sources\n\n");
    for cite in &cites {
        out.push_str(&format!("- `{cite}`\n"));
    }
    out
}

/// The note's text without its version line, to tell whether its words
/// moved.
fn unversioned(text: &str) -> String {
    text.lines()
        .filter(|line| !line.starts_with("version: "))
        .collect::<Vec<_>>()
        .join("\n")
}

fn version_of(text: &str) -> Option<u64> {
    text.lines()
        .find_map(|line| line.strip_prefix("version: "))
        .and_then(|v| v.trim().parse().ok())
}

/// The committed plugin list is what the built-in list says now, and it
/// names every built-in plugin and no sample.
#[test]
fn the_plugin_list_note_is_the_builtin_list() {
    let path = list_path();
    let committed = std::fs::read_to_string(&path).unwrap_or_default();
    let version = version_of(&committed).unwrap_or(0);
    let same = plugin_list(BUILTIN_PLUGINS, version);
    if std::env::var_os("PLUGIN_LIST_WRITE").is_some() {
        let next = if unversioned(&same) == unversioned(&committed) {
            same
        } else {
            plugin_list(BUILTIN_PLUGINS, version + 1)
        };
        std::fs::write(&path, next).expect("the plugin list is written");
        return;
    }
    assert!(
        committed == same,
        "knowledge/openagents/{LIST_ID}.md is stale for {LIST_SOURCE}; rewrite it with \
         PLUGIN_LIST_WRITE=1 cargo test -p coder --test plugin_catalog"
    );
    for plugin in BUILTIN_PLUGINS {
        assert!(committed.contains(plugin.name), "{}", plugin.name);
    }
    for sample in SAMPLES {
        assert!(!committed.contains(sample), "{sample}");
    }
}

/// No product note is a sample plugin's, and none is tagged `tool`: the
/// chat's tool catalog is empty, so the Gym offers nothing fake to test.
#[test]
fn no_sample_plugin_reaches_the_chat() {
    let corpus = Corpus::load(&root().join("knowledge/openagents"), Some(&root()))
        .expect("the product corpus loads");
    assert!(
        gym_kb::tools(&corpus).is_empty(),
        "no product note may be tagged `tool` while the Gym has no plugin to test"
    );
    for entry in &corpus.base.entries {
        if entry.id.starts_with("openagents.ttc-") || entry.id.starts_with("openagents.gen-") {
            // Essay notes describe what an essay measured, by date.
            continue;
        }
        for sample in SAMPLES {
            assert!(
                !entry.answer.as_deref().unwrap_or_default().contains(sample)
                    && !entry.summary.contains(sample),
                "{} names {sample}",
                entry.id
            );
        }
    }
}

/// The runner's catalog still resolves for its tests, and the app's
/// internal list and the authoring interview's starter catalog name the
/// same packages in the same order.
#[test]
fn the_runner_fixtures_stay_in_step() {
    let dirs = gym_kb::catalog_dirs();
    assert_eq!(dirs.len(), SAMPLES.len());
    for dir in &dirs {
        assert!(root().join(dir).join("package.json").is_file(), "{dir}");
    }
    assert_eq!(
        openagents_chat_app::eval_cards::catalog_dirs(),
        dirs,
        "the app reads the same catalog file"
    );
    let chips: Vec<&str> = openagents_chat_app::eval_cards::CATALOG.to_vec();
    assert_eq!(chips, SAMPLES, "openagents_chat_app::eval_cards::CATALOG");
    let starter: Vec<String> = ext_eval::author::catalog::Catalog::starter()
        .tools
        .into_iter()
        .map(|tool| tool.name)
        .collect();
    assert_eq!(
        starter, SAMPLES,
        "ext_eval::author::catalog::Catalog::starter"
    );
}

/// `docs/plugins/README.md`'s table of plugins you can use now is the
/// built-in list: each plugin's name and summary, in its order.
#[test]
fn the_plugins_page_lists_the_builtins() {
    let page = std::fs::read_to_string(root().join("docs/plugins/README.md"))
        .expect("docs/plugins/README.md");
    let rows: Vec<(String, String)> = page
        .split("## Plugins you can use now")
        .nth(1)
        .expect("the page has its table of plugins")
        .lines()
        .skip_while(|line| !line.starts_with('|'))
        .take_while(|line| line.starts_with('|'))
        .skip(2)
        .map(|line| {
            let cells: Vec<&str> = line.split(" | ").collect();
            (
                cells[0].trim_start_matches("| ").to_string(),
                cells[1].to_string(),
            )
        })
        .collect();
    let wanted: Vec<(String, String)> = BUILTIN_PLUGINS
        .iter()
        .map(|plugin| (plugin.name.to_string(), plugin.summary.to_string()))
        .collect();
    assert_eq!(
        rows, wanted,
        "docs/plugins/README.md: one row per built-in plugin, its name and summary"
    );
}
