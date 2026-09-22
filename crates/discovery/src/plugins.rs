//! The declarative plugin and skill packages, bundled for serving.
//!
//! Each client package is a directory under `plugins/` an agent copies
//! into its own environment — nothing here installs anything. The
//! `validate` check is the manifest's own self-test: every file a
//! manifest names exists in the bundle, the skill copies agree with
//! the canonical artifact, and the required fields parse.

use serde_json::Value;

/// The canonical skill — what `/.well-known/agent-skills/` serves and
/// what each client package copies verbatim.
pub const CANONICAL_SKILL: &str =
    include_str!("../../../plugins/skills/openagents-decision-api/SKILL.md");
/// The Claude package's plugin manifest.
pub const CLAUDE_MANIFEST: &str =
    include_str!("../../../plugins/claude/.claude-plugin/plugin.json");
/// The Claude package's MCP server declarations.
pub const CLAUDE_MCP: &str = include_str!("../../../plugins/claude/.mcp.json");
/// The Claude package's copy of the skill.
pub const CLAUDE_SKILL: &str =
    include_str!("../../../plugins/claude/skills/openagents-decision-api/SKILL.md");
/// The Codex package's plugin manifest.
pub const CODEX_MANIFEST: &str = include_str!("../../../plugins/codex/.codex-plugin/plugin.json");
/// The Codex package's MCP server declarations.
pub const CODEX_MCP: &str = include_str!("../../../plugins/codex/.mcp.json");
/// The Codex package's copy of the skill.
pub const CODEX_SKILL: &str =
    include_str!("../../../plugins/codex/skills/openagents-decision-api/SKILL.md");
/// The manual-installation guide the packages share.
pub const README: &str = include_str!("../../../plugins/README.md");

/// One bundled plugin file: the path it is served at and the path it
/// occupies inside its package — the same string, so a client can
/// reconstruct the package from the served files alone.
pub struct File {
    /// The repository-relative path, which is also the served suffix
    /// under `/plugins/`.
    pub path: &'static str,
    /// The file's exact bytes.
    pub bytes: &'static str,
    /// The `Content-Type` the route answers with.
    pub content_type: &'static str,
}

/// Every file the packages ship, in served order.
pub const FILES: &[File] = &[
    File {
        path: "README.md",
        bytes: README,
        content_type: "text/markdown; charset=utf-8",
    },
    File {
        path: "skills/openagents-decision-api/SKILL.md",
        bytes: CANONICAL_SKILL,
        content_type: "text/markdown; charset=utf-8",
    },
    File {
        path: "claude/.claude-plugin/plugin.json",
        bytes: CLAUDE_MANIFEST,
        content_type: "application/json; charset=utf-8",
    },
    File {
        path: "claude/.mcp.json",
        bytes: CLAUDE_MCP,
        content_type: "application/json; charset=utf-8",
    },
    File {
        path: "claude/skills/openagents-decision-api/SKILL.md",
        bytes: CLAUDE_SKILL,
        content_type: "text/markdown; charset=utf-8",
    },
    File {
        path: "codex/.codex-plugin/plugin.json",
        bytes: CODEX_MANIFEST,
        content_type: "application/json; charset=utf-8",
    },
    File {
        path: "codex/.mcp.json",
        bytes: CODEX_MCP,
        content_type: "application/json; charset=utf-8",
    },
    File {
        path: "codex/skills/openagents-decision-api/SKILL.md",
        bytes: CODEX_SKILL,
        content_type: "text/markdown; charset=utf-8",
    },
];

/// The checks a manifest must pass before it may be served.
///
/// Returns every violation found — an empty list means the packages
/// are internally consistent: each manifest parses, names a bundled
/// skills directory, carries the required identity fields, and every
/// shipped `SKILL.md` is byte-identical to the canonical artifact the
/// well-known index digests.
#[must_use]
pub fn validate() -> Vec<String> {
    let mut violations = Vec::new();
    for (name, manifest) in [("claude", CLAUDE_MANIFEST), ("codex", CODEX_MANIFEST)] {
        let parsed: Result<Value, _> = serde_json::from_str(manifest);
        let Ok(manifest) = parsed else {
            violations.push(format!("{name}: plugin.json does not parse"));
            continue;
        };
        for field in ["name", "version", "description"] {
            if manifest[field].as_str().is_none_or(str::is_empty) {
                violations.push(format!("{name}: plugin.json needs a `{field}` string"));
            }
        }
        if manifest["skills"].as_str() != Some("./skills/") {
            violations.push(format!("{name}: plugin.json must name `./skills/`"));
        }
        if manifest["name"].as_str() != Some("openagents-decision-api") {
            violations.push(format!(
                "{name}: plugin.json name is not `openagents-decision-api`"
            ));
        }
    }
    for (name, skill) in [("claude", CLAUDE_SKILL), ("codex", CODEX_SKILL)] {
        if skill != CANONICAL_SKILL {
            violations.push(format!(
                "{name}: SKILL.md drifts from the canonical artifact"
            ));
        }
    }
    for (name, mcp) in [("claude", CLAUDE_MCP), ("codex", CODEX_MCP)] {
        let parsed: Result<Value, _> = serde_json::from_str(mcp);
        let Ok(mcp) = parsed else {
            violations.push(format!("{name}: .mcp.json does not parse"));
            continue;
        };
        if mcp["mcpServers"]["oak"].is_null() {
            violations.push(format!(
                "{name}: .mcp.json does not declare the `oak` server"
            ));
        }
    }
    if !CANONICAL_SKILL.starts_with("---\nname: openagents-decision-api\n") {
        violations.push("the canonical SKILL.md lacks its frontmatter name".to_string());
    }
    violations
}
