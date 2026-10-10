//! Which commands a chat may offer, on which surface.
//!
//! The router's effect gate ([`crate::router::gate`]) decides by effect:
//! money and secrets are never proposed from chat, grants open a screen on
//! the phone, and the desktop and terminal take the rest with a confirm.
//! On top of it the owner decided on 2026-09-28 that the phone's chat
//! offers only the read-only commands in [`PHONE_COMMANDS`]; every other
//! command is not proposed there. The CLI route proposes a command only
//! when [`offered`] says so, and the router applies its own gate again to
//! whatever comes back.

use super::tree::{Leaf, RunsOn};
use crate::router::{CliGate, Surface, gate};

/// The commands the phone's chat offers: read-only, by the owner's
/// decision of 2026-09-28. A test checks each is in the tree and
/// declared read-only there.
pub const PHONE_COMMANDS: &[&str] = &[
    "computer list",
    "computer show",
    "computer workspaces",
    "verse who",
    "verse quests",
    "verse board",
    "verse xp",
    "kb search",
    "cap list",
    "prg list",
    "plugin list",
    "session list",
];

/// The commands the website's chat offers (#11167): GitHub changes, which
/// the website makes itself with the person's GitHub connection after a
/// signed confirm card (`github_actions::Action::from_argv`), never by
/// running the program. A test checks each the tree knows is declared
/// `publishes` there.
pub const WEB_COMMANDS: [&str; 6] = github_actions::COMMANDS;

/// Whether `leaf` may be proposed on `surface`: on the website, one of
/// [`WEB_COMMANDS`]; elsewhere the router's effect gate offers it, and on
/// the phone it is one of [`PHONE_COMMANDS`].
#[must_use]
pub fn offered(leaf: &Leaf, surface: Surface) -> bool {
    let path = leaf.path.join(" ");
    if surface == Surface::Web {
        return WEB_COMMANDS.contains(&path.as_str())
            && leaf.effect == super::tree::Effect::Publishes;
    }
    gate(leaf.effect, surface) == CliGate::Offer
        && (surface != Surface::Phone || PHONE_COMMANDS.contains(&path.as_str()))
}

/// The declared effect of the command `argv` (group first, without the
/// program's name) from this build's own command tree, when the tree
/// knows the command and its own argument parser accepts the words
/// (#10170). A computer reads this, never the effect a worker sent,
/// before it runs a command a chat reply proposed.
#[must_use]
pub fn effect_here(argv: &[String]) -> Option<super::tree::Effect> {
    let tree = super::tree::bundled();
    let words = super::tree::tree_argv(argv);
    let group = tree.group(words.first()?)?;
    // The longest path of the tree's words that names a command.
    let leaf = (1..=words.len())
        .rev()
        .find_map(|end| tree.leaf(&words[..end]))?;
    super::params::validate(leaf, group, argv).ok()?;
    Some(leaf.effect)
}

/// Where the command runs for `surface`: the declared place on the phone,
/// this device (where the program is) on the desktop and in the terminal.
#[must_use]
pub fn runs_on(leaf: &Leaf, surface: Surface) -> RunsOn {
    match surface {
        // The website proposes no command (`gate`); its place is the declared one.
        Surface::Phone | Surface::Web => leaf.runs_on,
        Surface::Desktop | Surface::Terminal => RunsOn::ThisDevice,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli_route::tree::{Effect, bundled};

    fn words(path: &str) -> Vec<String> {
        path.split(' ').map(str::to_owned).collect()
    }

    /// #11167: the website is offered the GitHub changes it makes itself
    /// and nothing else; every one the tree knows publishes, and the
    /// router's gate offers it there by its words.
    #[test]
    fn the_website_is_offered_only_github_changes() {
        let tree = bundled();
        for leaf in tree.leaves() {
            let path = leaf.path.join(" ");
            let github = WEB_COMMANDS.contains(&path.as_str());
            assert_eq!(
                offered(leaf, Surface::Web),
                github && leaf.effect == Effect::Publishes,
                "{path}"
            );
            if github {
                assert_eq!(leaf.effect, Effect::Publishes, "{path}");
            }
        }
        let argv = words("issue create --title T --project 22 --status Todo");
        assert_eq!(
            crate::router::gate_command(&argv, Effect::Publishes, Surface::Web),
            CliGate::Offer
        );
        assert_eq!(
            crate::router::gate_command(&words("issue claim 3"), Effect::Publishes, Surface::Web),
            CliGate::Withhold
        );
        assert_eq!(
            crate::router::gate_command(&argv, Effect::Publishes, Surface::Phone),
            crate::router::gate(Effect::Publishes, Surface::Phone)
        );
    }

    #[test]
    fn a_command_here_has_its_tree_effect_only_when_its_words_parse() {
        assert_eq!(
            effect_here(&words("wallet balance")),
            Some(Effect::ReadOnly)
        );
        assert_eq!(effect_here(&words("computer list")), Some(Effect::ReadOnly));
        assert_eq!(
            effect_here(&words("x402 node send bc1qexample --sats 1000")),
            Some(Effect::Spends)
        );
        assert_eq!(
            effect_here(&words("x402 node export --reveal")),
            Some(Effect::Secret)
        );
        assert_eq!(effect_here(&words("wallet balance --bogus")), None);
        assert_eq!(effect_here(&words("wallet")), None);
        assert_eq!(effect_here(&words("rm -rf")), None);
    }

    #[test]
    fn the_phone_list_is_read_only_commands_in_the_tree() {
        for path in PHONE_COMMANDS {
            let leaf = bundled()
                .leaf(&words(path))
                .unwrap_or_else(|| panic!("{path} is not in the tree"));
            assert_eq!(leaf.effect, Effect::ReadOnly, "{path}");
            assert!(offered(leaf, Surface::Phone), "{path}");
        }
    }

    /// The phone's read-only table in every build up to TestFlight 40,
    /// before #10087 (`crates/openagents-chat/src/router.rs` `READ_ONLY`
    /// at d8f7c3db4f). Those phones set aside any card not in it.
    const SHIPPED_PHONE_READ_ONLY: &[(&str, &[&str])] = &[
        ("computer", &["list", "show", "workspaces"]),
        ("verse", &["who", "quests", "board", "xp"]),
        ("kb", &["search"]),
        ("cap", &["list"]),
        ("prg", &["list"]),
        ("ext", &["list"]),
        ("session", &["list"]),
    ];

    /// Every phone command goes out as argv a shipped phone accepts
    /// (#10089): `plugin list` is offered as `ext list`.
    #[test]
    fn every_phone_command_goes_out_as_a_shipped_phone_accepts() {
        for path in PHONE_COMMANDS {
            let argv = crate::cli_route::tree::wire_argv(&words(path));
            assert!(
                SHIPPED_PHONE_READ_ONLY
                    .iter()
                    .any(|(group, leaves)| argv[0] == *group && leaves.contains(&argv[1].as_str())),
                "{path} goes out as {argv:?}"
            );
        }
    }

    #[test]
    fn no_surface_offers_money_or_secrets() {
        for leaf in bundled().leaves() {
            if matches!(leaf.effect, Effect::Spends | Effect::Secret) {
                for surface in [Surface::Phone, Surface::Desktop, Surface::Terminal] {
                    assert!(!offered(leaf, surface), "{} on {surface:?}", leaf.command());
                }
            }
        }
    }

    #[test]
    fn the_phone_offers_nothing_but_its_list() {
        let mut offered_on_phone = 0;
        for leaf in bundled().leaves() {
            if offered(leaf, Surface::Phone) {
                offered_on_phone += 1;
                assert!(PHONE_COMMANDS.contains(&leaf.path.join(" ").as_str()));
            }
        }
        assert_eq!(offered_on_phone, PHONE_COMMANDS.len());
        let approve = bundled().leaf(&words("computer approve")).unwrap();
        assert!(!offered(approve, Surface::Phone));
        assert!(offered(approve, Surface::Desktop));
        let who = bundled().leaf(&words("verse who")).unwrap();
        assert_eq!(runs_on(who, Surface::Phone), RunsOn::ConnectedComputer);
        assert_eq!(runs_on(who, Surface::Terminal), RunsOn::ThisDevice);
        let list = bundled().leaf(&words("computer list")).unwrap();
        assert_eq!(runs_on(list, Surface::Phone), RunsOn::ThisDevice);
    }
}
