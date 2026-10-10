//! The command tree the chat router descends, generated from this
//! program's own help text.
//!
//! [`help`] hands `coder::cli_route::tree::build` the top-level table and
//! every group's `USAGE` with the effects its module declares beside it.
//! The router reads the copy bundled in `crates/coder/src/cli_route/tree.json`;
//! the test here fails when that copy differs from what the live help
//! builds. Run it with `OPENAGENTS_WRITE_CLI_TREE=1` to write the copy.

use coder::cli_route::tree::{self, CommandTree, Declared, Effect, GroupHelp};

/// `openagents host`, `task`, `pair`, `doctor`, and `version` have their
/// syntax in other crates or none at all; this module owns their
/// declarations.
const HOST: &[Declared] = &[
    Declared::computer("init", Effect::Grants),
    Declared::computer("public-key", Effect::ReadOnly),
    Declared::screen("invite", Effect::Grants, "account.computers"),
    Declared::screen("request", Effect::Grants, "account.computers"),
    Declared::computer("list", Effect::ReadOnly),
    Declared::screen("revoke", Effect::Grants, "account.computers"),
    Declared::screen("spend request", Effect::Spends, "wallet"),
    Declared::screen("spend list", Effect::ReadOnly, "wallet"),
    Declared::screen("spend show", Effect::ReadOnly, "wallet"),
    Declared::computer("adopt", Effect::LocalWrite),
    Declared::computer("adopt detect", Effect::ReadOnly),
    // Sharing changes this computer's setting, and the host's supervisor
    // then publishes the pylon's beacons (an offline one when it stops).
    Declared::computer("share on", Effect::Publishes),
    Declared::computer("share off", Effect::Publishes),
    Declared::computer("share status", Effect::ReadOnly),
    Declared::computer("serve", Effect::LongRunning),
];

const TASK: &[Declared] = &[
    Declared::computer("submit", Effect::Publishes),
    Declared::computer("cancel", Effect::Publishes),
    Declared::computer("correct", Effect::Publishes),
    Declared::computer("check", Effect::ReadOnly),
    Declared::computer("list", Effect::ReadOnly),
    Declared::computer("show", Effect::ReadOnly),
    Declared::computer("start", Effect::Publishes),
    Declared::computer("execute", Effect::LongRunning),
    Declared::computer("recover", Effect::LocalWrite),
    Declared::computer("view", Effect::ReadOnly),
    Declared::computer("artifact", Effect::ReadOnly),
    Declared::computer("archive", Effect::LocalWrite),
    Declared::computer("restore", Effect::LocalWrite),
    Declared::computer("resume", Effect::LocalWrite),
];

const CODER: &[Declared] = &[
    Declared::computer("remote list", Effect::ReadOnly),
    Declared::computer("remote status", Effect::ReadOnly),
    Declared::computer("remote follow", Effect::LongRunning),
    Declared::computer("remote cancel", Effect::Publishes),
    Declared::computer("remote artifacts", Effect::ReadOnly),
    Declared::computer("remote apply", Effect::LocalWrite),
    Declared::computer("remote continue", Effect::LongRunning),
    Declared::computer("remote steer", Effect::Publishes),
    Declared::computer("status", Effect::ReadOnly),
    Declared::computer("chat", Effect::LongRunning),
    Declared::computer("delegate", Effect::LongRunning),
    Declared::computer("plugins list", Effect::ReadOnly),
    Declared::computer("plugins enable", Effect::LocalWrite),
    Declared::computer("plugins disable", Effect::LocalWrite),
    Declared::computer("plugins configure", Effect::LocalWrite),
    Declared::computer("plugins check", Effect::ReadOnly),
    Declared::computer("models list", Effect::ReadOnly),
    Declared::computer("models set", Effect::LocalWrite),
    Declared::computer("agents list", Effect::ReadOnly),
    Declared::computer("agents enable", Effect::LocalWrite),
    Declared::computer("agents disable", Effect::LocalWrite),
    Declared::computer("agents refresh", Effect::ReadOnly),
    Declared::computer("sessions list", Effect::ReadOnly),
    Declared::computer("sessions read", Effect::ReadOnly),
    Declared::computer("sessions delete", Effect::LocalWrite),
    Declared::computer("memory list", Effect::ReadOnly),
    Declared::computer("memory show", Effect::ReadOnly),
    Declared::computer("memory forget", Effect::LocalWrite),
    Declared::computer("memory instructions", Effect::ReadOnly),
    Declared::computer("export", Effect::LocalWrite),
    Declared::computer("import", Effect::LocalWrite),
    Declared::computer("trace upload", Effect::Publishes),
    Declared::computer("trace list", Effect::ReadOnly),
];

const PAIR: &[Declared] = &[Declared::screen("", Effect::Grants, "account.computers")];
const DOCTOR: &[Declared] = &[Declared::computer("", Effect::ReadOnly)];
const VERSION: &[Declared] = &[Declared::computer("", Effect::ReadOnly)];
const MCP: &[Declared] = &[Declared::computer("serve", Effect::LongRunning)];
const COMPLETIONS: &[Declared] = &[Declared::computer("", Effect::ReadOnly)];
const PYLON: &[Declared] = &[
    Declared::computer("whoami", Effect::LocalWrite),
    Declared::computer("serve", Effect::LongRunning),
    Declared::computer("link", Effect::LocalWrite),
    Declared::computer("route on", Effect::LocalWrite),
    Declared::computer("route off", Effect::LocalWrite),
    Declared::computer("route status", Effect::ReadOnly),
    Declared::computer("ask", Effect::Spends),
    // Sends known-answer or repeated jobs and publishes signed verdicts
    // (and, with --award, NIP-XP).
    Declared::computer("check canary", Effect::Publishes),
    Declared::computer("check redundant", Effect::Publishes),
    Declared::computer("league", Effect::LocalWrite),
    Declared::computer("status", Effect::LocalWrite),
    Declared::computer("pool", Effect::Publishes),
    Declared::computer("pool verify", Effect::LocalWrite),
];

/// Keep Pylon's command rows ahead of its introductory prose for the shared
/// help parser. The command syntax still comes from Pylon's own help.
fn pylon_usage() -> &'static str {
    static USAGE: std::sync::OnceLock<String> = std::sync::OnceLock::new();
    USAGE.get_or_init(|| {
        let header = pylon::cli::USAGE.lines().next().unwrap_or_default();
        let commands = pylon::cli::USAGE
            .split_once("Commands:\n")
            .map_or("", |(_, commands)| commands);
        format!("{header}\n{commands}")
    })
}

/// The `completions` syntax line; the prose under it is not syntax.
fn completions_usage() -> &'static str {
    crate::mcp::COMPLETIONS_USAGE
        .lines()
        .next()
        .unwrap_or("usage: openagents completions SHELL")
}

/// `openagents x402`'s help with `x402 node`'s commands among its own, as
/// `node init`, `node info`, and so on.
fn x402_usage() -> &'static str {
    static USAGE: std::sync::OnceLock<String> = std::sync::OnceLock::new();
    USAGE.get_or_init(|| {
        let rows = |usage: &str| -> (Vec<String>, Vec<String>) {
            let mut lines = usage.lines().skip(1);
            let mut rows = Vec::new();
            let mut notes = Vec::new();
            for line in lines.by_ref() {
                if !line.is_empty() && !line.starts_with(' ') {
                    notes.push(line.to_owned());
                    break;
                }
                rows.push(line.to_owned());
            }
            notes.extend(lines.map(str::to_owned));
            (rows, notes)
        };
        let (own, notes) = rows(crate::x402::USAGE);
        let (node, _) = rows(crate::x402_node::USAGE);
        let mut text = vec![
            crate::x402::USAGE
                .lines()
                .next()
                .unwrap_or_default()
                .to_owned(),
        ];
        text.extend(own);
        text.extend(node.into_iter().map(|line| match line.strip_prefix("  ") {
            Some(rest) if !rest.starts_with(' ') => format!("  node {rest}"),
            _ => line,
        }));
        text.extend(notes);
        text.join("\n")
    })
}

fn x402_effects() -> &'static [Declared] {
    static EFFECTS: std::sync::OnceLock<Vec<Declared>> = std::sync::OnceLock::new();
    EFFECTS.get_or_init(|| {
        let mut all = crate::x402::EFFECTS.to_vec();
        all.extend_from_slice(crate::x402_node::EFFECTS);
        all
    })
}

/// Every group's help, in no particular order; `build` follows the
/// top-level table's.
pub fn help() -> Vec<GroupHelp<'static>> {
    let group = |name, usage, declared| GroupHelp {
        name,
        usage,
        declared,
        alias: false,
    };
    vec![
        group("pylon", Some(pylon_usage()), PYLON),
        group("host", Some(coder_host::cli::USAGE), HOST),
        group(
            "connect",
            Some(crate::connect::USAGE),
            crate::connect::EFFECTS,
        ),
        group("pair", None, PAIR),
        group(
            "computer",
            Some(crate::computer::USAGE),
            crate::computer::EFFECTS,
        ),
        group("study", Some(crate::study::USAGE), crate::study::EFFECTS),
        group("reach", Some(crate::reach::USAGE), crate::reach::EFFECTS),
        group(
            "session",
            Some(crate::session::USAGE),
            crate::session::EFFECTS,
        ),
        group("chat", Some(crate::chat::USAGE), crate::chat::EFFECTS),
        group(
            "terminal",
            Some(crate::screen::USAGE),
            crate::screen::EFFECTS,
        ),
        group("coder", Some(coder_new::programmatic::USAGE), CODER),
        group("task", Some(coder::task::cli::USAGE), TASK),
        group("issue", Some(crate::issue::USAGE), crate::issue::EFFECTS),
        group(
            "project",
            Some(crate::github_verbs::PROJECT_USAGE),
            crate::github_verbs::PROJECT_EFFECTS,
        ),
        group("sales", Some(crate::sales::USAGE), crate::sales::EFFECTS),
        group(
            "customer",
            Some(crate::customer::tree_usage()),
            crate::customer::tree_effects(),
        ),
        group("lease", Some(crate::lease::USAGE), crate::lease::EFFECTS),
        group(
            "scratch",
            Some(crate::scratch::USAGE),
            crate::scratch::EFFECTS,
        ),
        group(
            "browser",
            Some(crate::browser::USAGE),
            crate::browser::EFFECTS,
        ),
        group(
            "capacity",
            Some(crate::capacity::USAGE),
            crate::capacity::EFFECTS,
        ),
        group(
            "artifact",
            Some(crate::artifact::USAGE),
            crate::artifact::EFFECTS,
        ),
        group(
            "settings",
            Some(crate::settings::USAGE),
            crate::settings::EFFECTS,
        ),
        group(
            "service",
            Some(crate::service::USAGE),
            crate::service::EFFECTS,
        ),
        group(
            "background",
            Some(crate::background::USAGE),
            crate::background::EFFECTS,
        ),
        group(
            "worktree",
            Some(crate::worktree::USAGE),
            crate::worktree::EFFECTS,
        ),
        group("ssh", Some(crate::ssh::USAGE), crate::ssh::EFFECTS),
        group(
            "boat",
            Some(crate::boat_run::USAGE),
            crate::boat_run::EFFECTS,
        ),
        group("studio", Some(crate::studio::USAGE), crate::studio::EFFECTS),
        group("agent", Some(crate::agent::USAGE), crate::agent::EFFECTS),
        group("shadow", Some(crate::shadow::USAGE), crate::shadow::EFFECTS),
        group(
            "efficiency",
            Some(crate::efficiency::USAGE),
            crate::efficiency::EFFECTS,
        ),
        group("cloud", Some(crate::cloud::USAGE), crate::cloud::EFFECTS),
        group("deploy", Some(crate::deploy::USAGE), crate::deploy::EFFECTS),
        group("pr", Some(crate::pr::USAGE), crate::pr::EFFECTS),
        group("verse", Some(crate::world::USAGE), crate::world::EFFECTS),
        GroupHelp {
            name: "xp",
            usage: None,
            declared: &[],
            alias: true,
        },
        group("zone", Some(crate::zone::USAGE), crate::zone::EFFECTS),
        group(
            "chamber",
            Some(crate::chamber::USAGE),
            crate::chamber::EFFECTS,
        ),
        group("sov", Some(crate::sov::USAGE), crate::sov::EFFECTS),
        group("eval", Some(crate::eval::USAGE), crate::eval::EFFECTS),
        group("gym", Some(crate::gym::USAGE), crate::gym::EFFECTS),
        group("labor", Some(crate::labor::USAGE), crate::labor::EFFECTS),
        group(
            "inference",
            Some(crate::inference::USAGE),
            crate::inference::EFFECTS,
        ),
        group("key", Some(crate::key::USAGE), crate::key::EFFECTS),
        group("wallet", Some(crate::wallet::USAGE), crate::wallet::EFFECTS),
        group("x402", Some(x402_usage()), x402_effects()),
        group("pay", Some(crate::pay::USAGE), crate::pay::EFFECTS),
        group("kb", Some(crate::kb::USAGE), crate::kb::EFFECTS),
        group("relay", Some(crate::relay::USAGE), crate::relay::EFFECTS),
        group(
            "playtest",
            Some(crate::playtest::USAGE),
            crate::playtest::EFFECTS,
        ),
        group(
            "cap",
            Some(crate::catalog::CAP_USAGE),
            crate::catalog::CAP_EFFECTS,
        ),
        group(
            "prg",
            Some(crate::catalog::PRG_USAGE),
            crate::catalog::PRG_EFFECTS,
        ),
        group(
            "plugin",
            Some(crate::catalog::EXT_USAGE),
            crate::catalog::EXT_EFFECTS,
        ),
        // `ext` is the older name for `plugin`.
        GroupHelp {
            name: "ext",
            usage: None,
            declared: &[],
            alias: true,
        },
        group(
            "discover",
            Some(crate::discover::USAGE),
            crate::discover::EFFECTS,
        ),
        group("mcp", Some(crate::mcp::USAGE), MCP),
        group("completions", Some(completions_usage()), COMPLETIONS),
        group("doctor", None, DOCTOR),
        group("version", None, VERSION),
    ]
}

/// The tree this program's help builds.
///
/// # Errors
///
/// Every problem `tree::build` found, one per line.
pub fn generate() -> Result<CommandTree, String> {
    tree::build(crate::USAGE, &help()).map_err(|errors| errors.join("\n"))
}

#[cfg(test)]
mod tests {
    use super::*;

    const BUNDLED: &str = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../coder/src/cli_route/tree.json"
    );

    #[test]
    fn every_command_parses_and_declares_its_effect() {
        let tree = generate().unwrap_or_else(|errors| panic!("{errors}"));
        for leaf in tree.leaves() {
            assert!(!leaf.forms.is_empty(), "{} has no form", leaf.command());
        }
    }

    #[test]
    fn the_bundled_tree_is_generated_from_this_help() {
        let tree = generate().unwrap_or_else(|errors| panic!("{errors}"));
        let text = tree.to_file();
        if std::env::var_os("OPENAGENTS_WRITE_CLI_TREE").is_some() {
            std::fs::write(BUNDLED, &text).expect("write the bundled tree");
            return;
        }
        let bundled = std::fs::read_to_string(BUNDLED).expect("read the bundled tree");
        assert!(
            bundled == text,
            "crates/coder/src/cli_route/tree.json is stale; run \
             `OPENAGENTS_WRITE_CLI_TREE=1 cargo test -p openagents-cli tree` to regenerate it"
        );
    }

    #[test]
    fn help_names_only_commands_this_program_runs() {
        let texts = help()
            .into_iter()
            .filter_map(|group| group.usage.map(|usage| (group.name, usage)))
            .chain([("host spend", coder_host::spend::cli::USAGE)]);
        for (name, usage) in texts {
            for old in [
                "coder host",
                "coder-service service",
                "coder-service update",
            ] {
                assert!(!usage.contains(old), "{name}'s help names `{old}`");
            }
        }
    }

    #[test]
    fn crew_forms_assemble_valid_all_and_member_controls() {
        use coder::cli_route::params::{self, Filled, Values};

        let tree = generate().unwrap();
        let group = tree.group("agent").unwrap();
        for action in ["stop", "pause", "resume"] {
            let path: Vec<String> = ["agent", "crew", action]
                .into_iter()
                .map(str::to_owned)
                .collect();
            let leaf = tree.leaf(&path).unwrap();
            assert_eq!(leaf.effect, Effect::Publishes);
            assert_eq!(leaf.forms.len(), 2);
            for form in &leaf.forms {
                let mut values = Values::from([("--cohort".into(), Filled::Word("floor".into()))]);
                let options = params::params(form);
                let all = options.iter().any(|p| p.key == "--all");
                if all {
                    values.insert("--all".into(), Filled::On);
                } else {
                    assert!(options.iter().any(|p| p.key == "--members"));
                    values.insert("--members".into(), Filled::Word("paul,erin".into()));
                }
                if action == "resume" {
                    values.insert(
                        "--expected".into(),
                        Filled::Word(format!("sha256:{}", "a".repeat(64))),
                    );
                }
                let argv = params::argv(leaf, form, &values).unwrap();
                params::validate(leaf, group, &argv).unwrap();
                assert_eq!(argv.iter().any(|arg| arg == "--all"), all);
                assert_eq!(argv.iter().any(|arg| arg == "--members"), !all);
                assert!(!argv.iter().any(|arg| arg.contains('|')));
            }
        }
    }

    #[test]
    fn money_and_secrets_are_labeled() {
        let tree = generate().unwrap_or_else(|errors| panic!("{errors}"));
        let effect = |path: &str| {
            let words: Vec<String> = path.split(' ').map(str::to_owned).collect();
            tree.leaf(&words).map(|leaf| leaf.effect)
        };
        for path in [
            "x402 node pay",
            "x402 node send",
            "x402 node channel open",
            "x402 fetch",
            "x402 buy",
            "x402 call",
        ] {
            assert_eq!(effect(path), Some(Effect::Spends), "{path}");
        }
        assert_eq!(effect("x402 node export"), Some(Effect::Secret));
        // The person's wallet is plain and read-only here; the x402 node
        // is never under it.
        assert_eq!(effect("wallet balance"), Some(Effect::ReadOnly));
        assert_eq!(effect("wallet address"), Some(Effect::ReadOnly));
        assert_eq!(effect("wallet history"), Some(Effect::ReadOnly));
        assert_eq!(effect("wallet send"), Some(Effect::Spends));
        assert_eq!(effect("wallet link"), Some(Effect::Secret));
        assert_eq!(effect("wallet restore"), Some(Effect::Secret));
        let wallet = tree.group("wallet").expect("the wallet group");
        for leaf in wallet.leaves() {
            let text = format!("{} {}", leaf.summary, leaf.usage.join(" ")).to_lowercase();
            for word in crate::wallet::TECHNICAL {
                assert!(!text.contains(word), "{} names {word}", leaf.command());
            }
        }
        for path in ["computer approve", "computer invite", "computer revoke"] {
            assert_eq!(effect(path), Some(Effect::Grants), "{path}");
        }
    }
}
