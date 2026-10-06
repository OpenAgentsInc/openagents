//! The `openagents` command tree, generated from its help text.
//!
//! [`build`] reads the top-level help table and each group's `USAGE`
//! string with [`super::usage`] and joins every command it finds with the
//! [`Declared`] effect and `runs_on` its owning module states next to that
//! `USAGE`. A command in the help with no declaration, or a declaration
//! with no command, is an error, so a new command cannot reach the router
//! without saying what it does.
//!
//! `openagents-cli` builds the tree from its live strings and checks that
//! the copy bundled here ([`bundled`], `tree.json`) is the same; set
//! `OPENAGENTS_WRITE_CLI_TREE=1` on its test to write a new one. Nothing
//! in the file is written by hand.

use std::sync::OnceLock;

use serde::{Deserialize, Serialize};

use super::usage::{self, Form, Token};

/// The tree's schema, recorded in the file and on every proposal.
pub const SCHEMA: &str = "openagents.cli-tree.v1";

pub use crate::router::{Effect, RunsOn};

/// Every effect class, in the order of consequence.
pub const EFFECTS: [Effect; 7] = [
    Effect::ReadOnly,
    Effect::LocalWrite,
    Effect::Publishes,
    Effect::Grants,
    Effect::Spends,
    Effect::Secret,
    Effect::LongRunning,
];

/// Every place a command runs.
pub const PLACES: [RunsOn; 3] = [
    RunsOn::ThisDevice,
    RunsOn::ConnectedComputer,
    RunsOn::Screen,
];

/// The router's [`Effect`] and [`RunsOn`] as their wire words in the
/// tree file.
mod word {
    use serde::{Deserialize, Deserializer, Serializer, de::Error};

    use super::{EFFECTS, Effect, PLACES, RunsOn};

    #[allow(clippy::trivially_copy_pass_by_ref)]
    pub fn effect_out<S: Serializer>(effect: &Effect, out: S) -> Result<S::Ok, S::Error> {
        out.serialize_str(effect.word())
    }

    pub fn effect_in<'de, D: Deserializer<'de>>(input: D) -> Result<Effect, D::Error> {
        let word = String::deserialize(input)?;
        EFFECTS
            .into_iter()
            .find(|effect| effect.word() == word)
            .ok_or_else(|| D::Error::custom(format!("unknown effect `{word}`")))
    }

    #[allow(clippy::trivially_copy_pass_by_ref)]
    pub fn place_out<S: Serializer>(place: &RunsOn, out: S) -> Result<S::Ok, S::Error> {
        out.serialize_str(place.word())
    }

    pub fn place_in<'de, D: Deserializer<'de>>(input: D) -> Result<RunsOn, D::Error> {
        let word = String::deserialize(input)?;
        PLACES
            .into_iter()
            .find(|place| place.word() == word)
            .ok_or_else(|| D::Error::custom(format!("unknown runs_on `{word}`")))
    }
}

/// Each renamed command's words in the tree and the older words a
/// proposal's argv carries on the wire, longest first. `plugin` was `ext`
/// and `plugin test` was `ext eval` until #10087: phones up to TestFlight
/// 40 accept only `ext list` as a read-only card, and an older
/// `openagents` on a connected computer knows only `ext`, while every
/// `openagents` since keeps `ext` and `eval` as aliases. So the tree, its
/// labels, and the descent say `plugin`, and the offer runs `ext` (#10089).
pub const WIRE_NAMES: &[(&[&str], &[&str])] = &[
    (&["plugin", "test"], &["ext", "eval"]),
    (&["plugin"], &["ext"]),
];

/// `argv` (group first) with its leading words renamed by the first
/// [`WIRE_NAMES`] pair that matches: to the wire's names, or back.
fn renamed(argv: &[String], to_wire: bool) -> Vec<String> {
    for &(tree, wire) in WIRE_NAMES {
        let (from, to) = if to_wire { (tree, wire) } else { (wire, tree) };
        if argv.len() >= from.len() && argv.iter().zip(from).all(|(word, name)| word == name) {
            let mut out: Vec<String> = to.iter().map(|word| (*word).to_string()).collect();
            out.extend(argv[from.len()..].iter().cloned());
            return out;
        }
    }
    argv.to_vec()
}

/// `argv` (group first) under the names the wire carries.
#[must_use]
pub fn wire_argv(argv: &[String]) -> Vec<String> {
    renamed(argv, true)
}

/// `argv` (group first) under the tree's names.
#[must_use]
pub fn tree_argv(argv: &[String]) -> Vec<String> {
    renamed(argv, false)
}

/// A command's effect and place, declared by the module that owns its
/// `USAGE`. `path` is the command's words after the group, space-joined
/// (`"list"`, `"channel open"`, `"control move"`), or empty for a group
/// that is itself the command.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Declared {
    pub path: &'static str,
    pub effect: Effect,
    pub runs_on: RunsOn,
    /// The phone screen that does this instead, for a command the phone
    /// chat does not run.
    pub screen: Option<&'static str>,
}

impl Declared {
    /// A command the phone's own core runs.
    #[must_use]
    pub const fn device(path: &'static str, effect: Effect) -> Self {
        Self {
            path,
            effect,
            runs_on: RunsOn::ThisDevice,
            screen: None,
        }
    }

    /// A command a connected computer runs for the phone.
    #[must_use]
    pub const fn computer(path: &'static str, effect: Effect) -> Self {
        Self {
            path,
            effect,
            runs_on: RunsOn::ConnectedComputer,
            screen: None,
        }
    }

    /// A command whose phone equivalent is `screen`.
    #[must_use]
    pub const fn screen(path: &'static str, effect: Effect, screen: &'static str) -> Self {
        Self {
            path,
            effect,
            runs_on: RunsOn::Screen,
            screen: Some(screen),
        }
    }
}

/// One group's help, as `openagents-cli` hands it over.
#[derive(Clone, Copy, Debug)]
pub struct GroupHelp<'a> {
    /// The group's name in the top-level table.
    pub name: &'a str,
    /// Its `USAGE` text, or `None` for a group with no syntax of its own
    /// (`doctor`, `version`), whose summary line is its whole help.
    pub usage: Option<&'a str>,
    pub declared: &'a [Declared],
    /// Another name for a group already in the tree (`xp` is `verse xp`);
    /// the tree lists the command once.
    pub alias: bool,
}

/// A runnable command.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Leaf {
    /// The full command words: group first (`["computer", "list"]`).
    pub path: Vec<String>,
    /// The usage text of each form, as the help prints it.
    pub usage: Vec<String>,
    pub forms: Vec<Form>,
    pub summary: String,
    #[serde(
        serialize_with = "word::effect_out",
        deserialize_with = "word::effect_in"
    )]
    pub effect: Effect,
    /// Where it runs for the phone; on the desktop and in the terminal
    /// every command runs on this device, where the program is.
    #[serde(
        serialize_with = "word::place_out",
        deserialize_with = "word::place_in"
    )]
    pub runs_on: RunsOn,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub screen: Option<String>,
}

impl Leaf {
    /// `openagents computer list`.
    #[must_use]
    pub fn command(&self) -> String {
        format!("openagents {}", self.path.join(" "))
    }
}

/// One level of the tree: a group, an intermediate word (`channel`), or a
/// command. A node can be both a command and a parent (`verse xp` and
/// `verse xp verify-card`).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Node {
    pub name: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub summary: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub leaf: Option<Leaf>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub children: Vec<Node>,
    /// Options the group's notes name for every command (`--relay URL`);
    /// only a group node carries them.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub options: Vec<Token>,
}

impl Node {
    fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            summary: String::new(),
            leaf: None,
            children: Vec::new(),
            options: Vec::new(),
        }
    }

    /// The child named `name`.
    #[must_use]
    pub fn child(&self, name: &str) -> Option<&Node> {
        self.children.iter().find(|child| child.name == name)
    }

    /// Every command at or under this node.
    #[must_use]
    pub fn leaves(&self) -> Vec<&Leaf> {
        let mut out: Vec<&Leaf> = self.leaf.iter().collect();
        for child in &self.children {
            out.extend(child.leaves());
        }
        out
    }

    fn insert(&mut self, words: &[String], leaf: Leaf) -> Result<(), String> {
        let Some((first, rest)) = words.split_first() else {
            if self.leaf.is_some() {
                return Err(format!("`{}` is listed twice", leaf.command()));
            }
            self.leaf = Some(leaf);
            return Ok(());
        };
        let index = match self.children.iter().position(|child| &child.name == first) {
            Some(index) => index,
            None => {
                self.children.push(Node::new(first));
                self.children.len() - 1
            }
        };
        self.children[index].insert(rest, leaf)
    }
}

/// The whole command tree.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CommandTree {
    pub schema: String,
    pub groups: Vec<Node>,
}

impl CommandTree {
    /// The group named `name`.
    #[must_use]
    pub fn group(&self, name: &str) -> Option<&Node> {
        self.groups.iter().find(|group| group.name == name)
    }

    /// The node at `path` (group first).
    #[must_use]
    pub fn node(&self, path: &[String]) -> Option<&Node> {
        let (first, rest) = path.split_first()?;
        let mut node = self.group(first)?;
        for word in rest {
            node = node.child(word)?;
        }
        Some(node)
    }

    /// The command at `path`.
    #[must_use]
    pub fn leaf(&self, path: &[String]) -> Option<&Leaf> {
        self.node(path)?.leaf.as_ref()
    }

    /// Every command in the tree.
    #[must_use]
    pub fn leaves(&self) -> Vec<&Leaf> {
        self.groups.iter().flat_map(Node::leaves).collect()
    }

    /// The tree as the checked-in file holds it.
    #[must_use]
    pub fn to_file(&self) -> String {
        let mut text = serde_json::to_string_pretty(self).expect("the tree serializes");
        text.push('\n');
        text
    }
}

/// Build the tree from the top-level help table and each group's help.
///
/// # Errors
///
/// Lists every problem at once: a group in the table with no help handed
/// over, a row that does not parse, a command with no declaration, and a
/// declaration with no command.
pub fn build(top: &str, help: &[GroupHelp<'_>]) -> Result<CommandTree, Vec<String>> {
    let mut errors = Vec::new();
    let mut groups = Vec::new();
    for row in usage::groups(top) {
        let Some(group) = help.iter().find(|group| group.name == row.name) else {
            errors.push(format!("`{}` has no help registered", row.name));
            continue;
        };
        if group.alias {
            continue;
        }
        match group_node(&row.name, &row.summary, group) {
            Ok(node) => groups.push(node),
            Err(mut more) => errors.append(&mut more),
        }
    }
    for group in help {
        if !usage::groups(top).iter().any(|row| row.name == group.name) {
            errors.push(format!("`{}` is not in the top-level help", group.name));
        }
    }
    if errors.is_empty() {
        Ok(CommandTree {
            schema: SCHEMA.to_string(),
            groups,
        })
    } else {
        Err(errors)
    }
}

fn group_node(name: &str, summary: &str, help: &GroupHelp<'_>) -> Result<Node, Vec<String>> {
    let mut errors = Vec::new();
    let mut node = Node::new(name);
    node.summary = summary.to_string();
    let mut used = vec![false; help.declared.len()];
    let mut declared = |path: &str, errors: &mut Vec<String>| match help
        .declared
        .iter()
        .position(|d| d.path == path)
    {
        Some(index) => {
            used[index] = true;
            Some(help.declared[index])
        }
        None => {
            errors.push(format!("`openagents {name} {path}` has no declared effect"));
            None
        }
    };
    let rows = match help.usage {
        Some(text) => usage::rows(name, text).map_err(|error| vec![format!("{name}: {error}")])?,
        None => Vec::new(),
    };
    if let Some(text) = help.usage {
        node.options = usage::options(text);
    }
    if rows.is_empty() || help.declared.iter().any(|command| command.path.is_empty()) {
        let form = match help.usage {
            Some(text) => {
                usage::bare(name, text).map_err(|error| vec![format!("{name}: {error}")])?
            }
            None => Form::default(),
        };
        if let Some(declared) = declared("", &mut errors) {
            node.leaf = Some(Leaf {
                path: vec![name.to_string()],
                usage: vec![help.usage.and_then(|text| text.lines().next()).map_or_else(
                    || format!("openagents {name}"),
                    |line| line.trim_start_matches("usage: ").to_string(),
                )],
                forms: vec![form],
                summary: summary.to_string(),
                effect: declared.effect,
                runs_on: declared.runs_on,
                screen: declared.screen.map(str::to_string),
            });
        }
    }
    for row in rows {
        let path = row.path.join(" ");
        let Some(declared) = declared(&path, &mut errors) else {
            continue;
        };
        let mut full = vec![name.to_string()];
        full.extend(row.path.iter().cloned());
        let leaf = Leaf {
            path: full,
            usage: row.usage,
            forms: row.forms,
            summary: row.summary,
            effect: declared.effect,
            runs_on: declared.runs_on,
            screen: declared.screen.map(str::to_string),
        };
        if let Err(error) = node.insert(&row.path, leaf) {
            errors.push(error);
        }
    }
    for (index, used) in used.into_iter().enumerate() {
        if !used {
            errors.push(format!(
                "`openagents {name} {}` is declared but not in the help",
                help.declared[index].path
            ));
        }
    }
    if errors.is_empty() {
        Ok(node)
    } else {
        Err(errors)
    }
}

/// The tree bundled with this crate, generated by `openagents-cli`.
///
/// # Panics
///
/// Never in a build whose `tree.json` came from `openagents-cli`'s
/// generator, which a test in that crate checks.
#[must_use]
pub fn bundled() -> &'static CommandTree {
    static TREE: OnceLock<CommandTree> = OnceLock::new();
    TREE.get_or_init(|| {
        serde_json::from_str(include_str!("tree.json")).expect("the bundled command tree parses")
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const TOP: &str = "usage: openagents COMMAND
  computer     Enroll with hosts.
  doctor       Show the identities.
  xp           This identity's XP.";
    const COMPUTER: &str = "usage: openagents computer COMMAND [OPTIONS]
  list [--wait SECONDS]     Every host this device knows.
  show HOST                 One host.
Options: --store DIR.";

    #[test]
    fn builds_from_help_and_declarations() {
        let declared = [
            Declared::device("list", Effect::ReadOnly),
            Declared::device("show", Effect::ReadOnly),
        ];
        let tree = build(
            TOP,
            &[
                GroupHelp {
                    name: "computer",
                    usage: Some(COMPUTER),
                    declared: &declared,
                    alias: false,
                },
                GroupHelp {
                    name: "doctor",
                    usage: None,
                    declared: &[Declared::computer("", Effect::ReadOnly)],
                    alias: false,
                },
                GroupHelp {
                    name: "xp",
                    usage: None,
                    declared: &[],
                    alias: true,
                },
            ],
        )
        .unwrap();
        assert_eq!(tree.groups.len(), 2);
        let show = tree
            .leaf(&["computer".to_string(), "show".to_string()])
            .unwrap();
        assert_eq!(show.summary, "One host.");
        assert_eq!(show.effect, Effect::ReadOnly);
        assert!(
            tree.group("computer")
                .unwrap()
                .options
                .iter()
                .any(|t| matches!(t, Token::Option { name, .. } if name == "store"))
        );
        assert_eq!(
            tree.leaf(&["doctor".to_string()]).unwrap().command(),
            "openagents doctor"
        );
    }

    #[test]
    fn an_explicit_bare_command_keeps_its_subcommands() {
        let tree = build(
            "usage: openagents COMMAND\n  terminal     Open a chat or shell.",
            &[GroupHelp {
                name: "terminal",
                usage: Some(
                    "usage: openagents terminal [--thread ID]\n  shell [--root DIR]     Open a shell.",
                ),
                declared: &[
                    Declared::computer("", Effect::LongRunning),
                    Declared::computer("shell", Effect::LongRunning),
                ],
                alias: false,
            }],
        )
        .unwrap();
        let chat = tree.leaf(&["terminal".into()]).unwrap();
        assert_eq!(chat.usage, ["openagents terminal [--thread ID]"]);
        assert_eq!(chat.effect, Effect::LongRunning);
        let shell = tree.leaf(&["terminal".into(), "shell".into()]).unwrap();
        assert_eq!(shell.usage, ["shell [--root DIR]"]);
        assert_eq!(shell.effect, Effect::LongRunning);
    }

    #[test]
    fn an_undeclared_command_or_a_stray_declaration_fails() {
        let errors = build(
            TOP,
            &[
                GroupHelp {
                    name: "computer",
                    usage: Some(COMPUTER),
                    declared: &[
                        Declared::device("list", Effect::ReadOnly),
                        Declared::device("forget", Effect::LocalWrite),
                    ],
                    alias: false,
                },
                GroupHelp {
                    name: "doctor",
                    usage: None,
                    declared: &[Declared::computer("", Effect::ReadOnly)],
                    alias: false,
                },
            ],
        )
        .unwrap_err();
        assert!(
            errors
                .iter()
                .any(|e| e.contains("computer show` has no declared effect"))
        );
        assert!(
            errors
                .iter()
                .any(|e| e.contains("forget` is declared but not in the help"))
        );
        assert!(
            errors
                .iter()
                .any(|e| e.contains("`xp` has no help registered"))
        );
    }

    #[test]
    fn a_renamed_command_goes_out_under_its_older_name() {
        let words = |text: &str| text.split(' ').map(str::to_owned).collect::<Vec<_>>();
        for (tree, wire) in [
            ("plugin list --limit 5", "ext list --limit 5"),
            ("plugin test run DIR --trust", "ext eval run DIR --trust"),
            ("plugin defaults show", "ext defaults show"),
            ("computer list", "computer list"),
        ] {
            assert_eq!(wire_argv(&words(tree)), words(wire), "{tree}");
            assert_eq!(tree_argv(&words(wire)), words(tree), "{wire}");
        }
        // Only the leading command words are names.
        assert_eq!(
            wire_argv(&words("plugin list --type test")),
            words("ext list --type test")
        );
        for (tree, wire) in WIRE_NAMES {
            let tree: Vec<String> = tree.iter().map(|w| (*w).to_string()).collect();
            assert!(bundled().node(&tree).is_some(), "{tree:?} is in the tree");
            assert!(
                bundled().group(wire[0]).is_none(),
                "{wire:?} is only a wire name"
            );
        }
    }

    #[test]
    fn the_bundled_tree_parses() {
        let tree = bundled();
        assert_eq!(tree.schema, SCHEMA);
        assert!(!tree.leaves().is_empty());
    }
}
