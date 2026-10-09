//! The Gym's plugins come from one catalog (#10090).
//!
//! `deploy/eval-runner/catalog` lists the plugin directories the hosted
//! runner tests, and each directory's `package.json` names the plugin and
//! says what it does in one line. Everything the chat says about which
//! plugins there are follows from it:
//!
//! - the worker's tool catalog ([`coder::gym_kb::tools`]): one product note
//!   per plugin, in the catalog's order, titled with the package's name and
//!   summarized with its summary, so the `tool` question, the Gym card,
//!   and `eval.run.choose`'s list read the package;
//! - `knowledge/openagents/openagents.plugin-list.md`, the note that
//!   answers "which plugins are in the Gym?", generated from the catalog;
//! - the app's Gym chips (`openagents_chat_app::eval_cards::CATALOG`) and
//!   the authoring interview's starter catalog, in the same order;
//! - the table of plugins in `docs/plugins/README.md`.
//!
//! Adding a plugin to the catalog fails these tests until each follows.
//! Rewrite the generated note with
//!
//! ```text
//! PLUGIN_LIST_WRITE=1 cargo test -p coder --test plugin_catalog
//! ```
//!
//! and then the route map's snapshot (`ROUTE_MAP_WRITE=1 cargo test -p
//! coder --test route_map_sources`).

use std::path::{Path, PathBuf};

use coder::gym_kb::{self, CATALOG_PATH};
use knowledge::product::Corpus;
use serde_json::Value;

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// One catalog plugin, from its package record.
struct Plugin {
    dir: String,
    name: String,
    summary: String,
}

fn catalog() -> Vec<Plugin> {
    gym_kb::catalog_dirs()
        .into_iter()
        .map(|dir| {
            let path = root().join(dir).join("package.json");
            let package: Value = serde_json::from_str(
                &std::fs::read_to_string(&path)
                    .unwrap_or_else(|e| panic!("{}: {e}", path.display())),
            )
            .unwrap_or_else(|e| panic!("{}: {e}", path.display()));
            let text = |field: &str| {
                package[field]
                    .as_str()
                    .filter(|value| !value.trim().is_empty())
                    .unwrap_or_else(|| panic!("{dir}/package.json has no {field}"))
                    .to_string()
            };
            Plugin {
                dir: dir.to_string(),
                name: text("name"),
                summary: text("summary"),
            }
        })
        .collect()
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

fn list_path() -> PathBuf {
    root()
        .join("knowledge/openagents")
        .join(format!("{LIST_ID}.md"))
}

/// The plugin list note for `plugins`, at `version`.
fn plugin_list(plugins: &[Plugin], version: u64) -> String {
    let names: Vec<&str> = plugins.iter().map(|p| p.name.as_str()).collect();
    let answer = format!(
        "The Gym has {} plugins you can test on Coder: {}. Each has its own test set, run with \
         the plugin and without it, so you can see whether it makes Coder better. Ask us what \
         one does, or ask to test one.",
        count(plugins.len()),
        series(&names)
    );
    let mut cites = vec![CATALOG_PATH.to_string()];
    cites.extend(plugins.iter().map(|p| format!("{}/package.json", p.dir)));
    let mut out = String::new();
    out.push_str(&format!(
        "---\nid: {LIST_ID}\nversion: {version}\nkind: product\ntitle: \"Which plugins there are\"\n"
    ));
    out.push_str(&format!(
        "summary: >-\n  The plugins in the Gym, which you can test on Coder: {}.\n",
        series(&names)
    ));
    out.push_str("tags: [gym, plugins, catalog, extension]\n");
    out.push_str(
        "applies_when: >-\n  The user asks which plugins there are, which plugins are in the \
         Gym, or which plugins they can test or use with Coder.\n",
    );
    out.push_str(&format!("answer: >-\n  {answer}\n"));
    out.push_str("status: admitted\nauthor: openagents\nprovenance:\n  written_from: [reference]\n  cites:\n");
    for cite in &cites {
        out.push_str(&format!("    - {cite}\n"));
    }
    out.push_str(
        "evidence:\n  - \"Generated from the hosted runner's catalog and each plugin's package.json \
         by crates/coder/tests/plugin_catalog.rs (#10090); PLUGIN_LIST_WRITE=1 rewrites it, and \
         its version moves when its words do.\"\n---\n\n",
    );
    out.push_str(&format!("## Answer\n\n{answer}\n\n## Details\n\n"));
    for plugin in plugins {
        out.push_str(&format!(
            "- **{}** (`{}`): {}\n",
            plugin.name, plugin.dir, plugin.summary
        ));
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

/// The committed plugin list is what the catalog says now.
#[test]
fn the_plugin_list_note_is_the_catalog() {
    let plugins = catalog();
    assert!(!plugins.is_empty(), "the catalog lists no plugin");
    let path = list_path();
    let committed = std::fs::read_to_string(&path).unwrap_or_default();
    let version = version_of(&committed).unwrap_or(0);
    let same = plugin_list(&plugins, version);
    if std::env::var_os("PLUGIN_LIST_WRITE").is_some() {
        let next = if unversioned(&same) == unversioned(&committed) {
            same
        } else {
            plugin_list(&plugins, version + 1)
        };
        std::fs::write(&path, next).expect("the plugin list is written");
        return;
    }
    assert!(
        committed == same,
        "knowledge/openagents/{LIST_ID}.md is stale for {CATALOG_PATH}; rewrite it with \
         PLUGIN_LIST_WRITE=1 cargo test -p coder --test plugin_catalog"
    );
}

/// The package records compiled into the worker and the website (the plugin
/// cards, `docs/web/plugin-card.md`) are the catalog's, in its order, and
/// each reads its slug, name, and summary.
#[test]
fn the_compiled_in_packages_are_the_catalog() {
    let dirs: Vec<&str> = gym_kb::CATALOG_PACKAGES
        .iter()
        .map(|(dir, _)| *dir)
        .collect();
    assert_eq!(
        dirs,
        gym_kb::catalog_dirs(),
        "gym_kb::CATALOG_PACKAGES needs one include_str! per directory in {CATALOG_PATH}, \
         in its order"
    );
    let compiled = gym_kb::catalog_plugins();
    let plugins = catalog();
    assert_eq!(
        compiled.len(),
        plugins.len(),
        "a package record failed to read"
    );
    for (compiled, plugin) in compiled.iter().zip(&plugins) {
        assert_eq!(compiled.name, plugin.name);
        assert_eq!(compiled.summary, plugin.summary);
        assert!(!compiled.slug.is_empty());
        assert_eq!(
            gym_kb::catalog_plugin(&compiled.slug).as_ref(),
            Some(compiled)
        );
    }
}

/// The worker's tool catalog is the runner's catalog: one note per
/// plugin, in its order, named and summarized by its package.
#[test]
fn the_chat_tool_catalog_is_the_runner_catalog() {
    let plugins = catalog();
    let corpus = Corpus::load(&root().join("knowledge/openagents"), Some(&root()))
        .expect("the product corpus loads");
    let tools = gym_kb::tools(&corpus);
    let names: Vec<&str> = tools.iter().map(|t| t.name.as_str()).collect();
    let wanted: Vec<&str> = plugins.iter().map(|p| p.name.as_str()).collect();
    assert_eq!(
        names, wanted,
        "every plugin in {CATALOG_PATH} needs one product note tagged `tool` and its \
         component slug (knowledge/openagents/openagents.tool-*.md), titled with its \
         package's name, and no other note may be tagged `tool`"
    );
    for (tool, plugin) in tools.iter().zip(&plugins) {
        assert_eq!(
            tool.line, plugin.summary,
            "{}: the note's summary is the plugin's package.json summary",
            tool.source
        );
        assert!(
            tool.slugs
                .iter()
                .any(|slug| slug == gym_kb::catalog_slug(&plugin.dir)),
            "{} carries {}'s component slug",
            tool.source,
            plugin.dir
        );
    }
    assert_eq!(tools[0].id, gym_kb::DEFAULT_TOOL);
}

/// The app's Gym chips and the authoring interview's starter catalog name
/// the same plugins in the same order.
#[test]
fn the_app_and_the_interview_list_the_catalog() {
    let wanted: Vec<String> = catalog().into_iter().map(|p| p.name).collect();
    let chips: Vec<String> = openagents_chat_app::eval_cards::CATALOG
        .iter()
        .map(|name| (*name).to_string())
        .collect();
    assert_eq!(chips, wanted, "openagents_chat_app::eval_cards::CATALOG");
    assert_eq!(
        openagents_chat_app::eval_cards::catalog_dirs(),
        gym_kb::catalog_dirs(),
        "the app reads the same catalog file"
    );
    let starter: Vec<String> = ext_eval::author::catalog::Catalog::starter()
        .tools
        .into_iter()
        .map(|tool| tool.name)
        .collect();
    assert_eq!(
        starter, wanted,
        "ext_eval::author::catalog::Catalog::starter"
    );
}

/// `docs/plugins/README.md`'s table of plugins you can use now is the
/// catalog: each plugin's name and summary, in its order.
#[test]
fn the_plugins_page_lists_the_catalog() {
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
    let wanted: Vec<(String, String)> = catalog()
        .into_iter()
        .map(|plugin| (plugin.name, plugin.summary))
        .collect();
    assert_eq!(
        rows, wanted,
        "docs/plugins/README.md: one row per catalog plugin, its package's name and summary"
    );
}
