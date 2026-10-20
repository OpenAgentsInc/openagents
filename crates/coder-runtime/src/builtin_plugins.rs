//! The plugins Coder ships with that work today, as the chat shows them
//! (`docs/web/plugin-card.md`): the coding agents the `openagents`
//! terminal hands work to over ACP (`crates/coder-new`'s ACP Subagents
//! plugin) and the bring-your-own-key model plugin.
//!
//! Each entry names the code and the tests that show it works, so a card
//! never offers something that doesn't. The Gym's sample packages
//! (`crates/plugin-*`) are test fixtures for the hosted eval runner and are
//! not shown to people.

/// One built-in plugin as a card shows it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BuiltinPlugin {
    /// The id a reply's `plugins` field carries.
    pub slug: &'static str,
    /// The name people see.
    pub name: &'static str,
    /// One plain line on what it does.
    pub summary: &'static str,
    /// The code that does it and the tests that check it.
    pub evidence: &'static [&'static str],
}

/// The ACP delegation code and its tests, shared by the four agents.
const ACP: [&str; 4] = [
    "crates/coder-new/src/acp_discovery.rs",
    "crates/coder-new/src/bundled_runtime.rs",
    "crates/coder-new/src/delegation_events.rs",
    "crates/coder-new/tests/bundled_plugins.rs",
];

/// The built-in plugins, in the order cards show them.
pub const BUILTIN_PLUGINS: &[BuiltinPlugin] = &[
    BuiltinPlugin {
        slug: "claude-code",
        name: "Claude Code",
        summary: "Coder hands a task to Claude Code on your computer and shows its progress as it works.",
        evidence: &ACP,
    },
    BuiltinPlugin {
        slug: "codex",
        name: "Codex",
        summary: "Coder hands a task to Codex on your computer and shows its progress as it works.",
        evidence: &ACP,
    },
    BuiltinPlugin {
        slug: "cursor",
        name: "Cursor",
        summary: "Coder hands a task to Cursor's agent on your computer and shows its progress as it works.",
        evidence: &ACP,
    },
    BuiltinPlugin {
        slug: "grok-build",
        name: "Grok Build",
        summary: "Coder hands a task to Grok Build on your computer and shows its progress as it works.",
        evidence: &ACP,
    },
    BuiltinPlugin {
        slug: "openrouter",
        name: "OpenRouter",
        summary: "Use OpenRouter models in Coder with your own API key.",
        evidence: &[
            "crates/coder-new/src/plugin_definition.rs",
            "crates/coder-new/tests/bundled_plugins.rs",
        ],
    },
];

/// The built-in plugin whose slug is `slug`.
#[must_use]
pub fn builtin_plugin(slug: &str) -> Option<&'static BuiltinPlugin> {
    BUILTIN_PLUGINS.iter().find(|plugin| plugin.slug == slug)
}

/// Every built-in plugin's name, in order.
#[must_use]
pub fn names() -> Vec<String> {
    BUILTIN_PLUGINS
        .iter()
        .map(|plugin| plugin.name.to_owned())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every card names code and tests that exist, and no sample plugin
    /// is among them.
    #[test]
    fn every_builtin_names_real_code_and_no_sample() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        for plugin in BUILTIN_PLUGINS {
            assert!(!plugin.evidence.is_empty(), "{}", plugin.slug);
            for path in plugin.evidence {
                assert!(root.join(path).is_file(), "{}: {path}", plugin.slug);
            }
            assert!(
                oa_copy::violations(plugin.summary, &[]).is_empty(),
                "{}",
                plugin.slug
            );
        }
        let acp =
            std::fs::read_to_string(root.join("crates/coder-new/src/acp_discovery.rs")).unwrap();
        for id in ["claude-code", "codex", "cursor", "grok-build"] {
            assert!(acp.contains(&format!("\"{id}\"")), "{id} is discovered");
            assert!(builtin_plugin(id).is_some());
        }
        let shown = names().join(" ");
        for sample in [
            "Project map",
            "Code finder",
            "Test reader",
            "Explain this error",
            "Release notes",
            "Dependency check",
        ] {
            assert!(!shown.contains(sample), "{sample}");
        }
    }
}
