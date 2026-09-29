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
];

const PAIR: &[Declared] = &[Declared::screen("", Effect::Grants, "account.computers")];
const DOCTOR: &[Declared] = &[Declared::computer("", Effect::ReadOnly)];
const VERSION: &[Declared] = &[Declared::computer("", Effect::ReadOnly)];
const MCP: &[Declared] = &[Declared::computer("serve", Effect::LongRunning)];
const COMPLETIONS: &[Declared] = &[Declared::computer("", Effect::ReadOnly)];

/// The `completions` syntax line, which `mcp::USAGE` carries after
/// `mcp`'s; the prose under it is not syntax.
fn completions_usage() -> &'static str {
    let usage = crate::mcp::USAGE;
    usage
        .find("usage: openagents completions")
        .map(|at| &usage[at..])
        .and_then(|text| text.lines().next())
        .unwrap_or("usage: openagents completions SHELL")
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
        group("task", Some(coder::task::cli::USAGE), TASK),
        group(
            "service",
            Some(crate::service::USAGE),
            crate::service::EFFECTS,
        ),
        group("ssh", Some(crate::ssh::USAGE), crate::ssh::EFFECTS),
        group("verse", Some(crate::world::USAGE), crate::world::EFFECTS),
        GroupHelp {
            name: "xp",
            usage: None,
            declared: &[],
            alias: true,
        },
        group("zone", Some(crate::zone::USAGE), crate::zone::EFFECTS),
        group("sov", Some(crate::sov::USAGE), crate::sov::EFFECTS),
        group("eval", Some(crate::eval::USAGE), crate::eval::EFFECTS),
        group("gym", Some(crate::gym::USAGE), crate::gym::EFFECTS),
        group("labor", Some(crate::labor::USAGE), crate::labor::EFFECTS),
        group("key", Some(crate::key::USAGE), crate::key::EFFECTS),
        group("wallet", Some(crate::wallet::USAGE), crate::wallet::EFFECTS),
        group("x402", Some(crate::x402::USAGE), crate::x402::EFFECTS),
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
            "ext",
            Some(crate::catalog::EXT_USAGE),
            crate::catalog::EXT_EFFECTS,
        ),
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
    fn money_and_secrets_are_labeled() {
        let tree = generate().unwrap_or_else(|errors| panic!("{errors}"));
        let effect = |path: &str| {
            let words: Vec<String> = path.split(' ').map(str::to_owned).collect();
            tree.leaf(&words).map(|leaf| leaf.effect)
        };
        for path in [
            "wallet pay",
            "wallet send",
            "wallet channel open",
            "x402 fetch",
            "x402 buy",
            "x402 call",
        ] {
            assert_eq!(effect(path), Some(Effect::Spends), "{path}");
        }
        assert_eq!(effect("wallet export"), Some(Effect::Secret));
        for path in ["computer approve", "computer invite", "computer revoke"] {
            assert_eq!(effect(path), Some(Effect::Grants), "{path}");
        }
    }
}
