//! Command parsing and effects shared by routing and task execution.

pub mod params;
pub mod tree;
pub mod usage;

/// A command's effect class, declared next to its usage string.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Effect {
    /// Reads only.
    ReadOnly,
    /// Writes this device's stores.
    LocalWrite,
    /// Signs and sends an event.
    Publishes,
    /// Changes who may do what.
    Grants,
    /// Moves money.
    Spends,
    /// Shows or exports a secret.
    Secret,
    /// Runs until stopped.
    LongRunning,
}

impl Effect {
    /// The word the wire carries.
    #[must_use]
    pub fn word(self) -> &'static str {
        match self {
            Effect::ReadOnly => "read_only",
            Effect::LocalWrite => "local_write",
            Effect::Publishes => "publishes",
            Effect::Grants => "grants",
            Effect::Spends => "spends",
            Effect::Secret => "secret",
            Effect::LongRunning => "long_running",
        }
    }
}

/// Where a command can run for a surface.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RunsOn {
    /// The device's own client.
    ThisDevice,
    /// Through a connected computer.
    ConnectedComputer,
    /// A screen on the device does it better.
    Screen,
}

impl RunsOn {
    /// The word the wire carries.
    #[must_use]
    pub fn word(self) -> &'static str {
        match self {
            RunsOn::ThisDevice => "this_device",
            RunsOn::ConnectedComputer => "connected_computer",
            RunsOn::Screen => "screen",
        }
    }
}

/// The declared effect of the command `argv` (group first, without the
/// program's name) from this build's own command tree, when the tree
/// knows the command and its own argument parser accepts the words
/// (#10170). A computer reads this, never the effect a worker sent,
/// before it runs a command a chat reply proposed.
#[must_use]
pub fn effect_here(argv: &[String]) -> Option<tree::Effect> {
    let tree = tree::bundled();
    let words = tree::tree_argv(argv);
    let group = tree.group(words.first()?)?;
    // The longest path of the tree's words that names a command.
    let leaf = (1..=words.len())
        .rev()
        .find_map(|end| tree.leaf(&words[..end]))?;
    params::validate(leaf, group, argv).ok()?;
    Some(leaf.effect)
}
