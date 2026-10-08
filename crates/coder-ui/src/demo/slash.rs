//! Original demo slash command identity and disclosure.

/// A terminal command.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Command {
    Demo,
    Plugins,
    Models,
    Export,
    Resume,
    Brainstorm,
    Help,
}

/// Commands in the order shown by the picker.
pub const ALL: &[Command] = &[
    Command::Demo,
    Command::Plugins,
    Command::Models,
    Command::Export,
    Command::Resume,
    Command::Brainstorm,
    Command::Help,
];

impl Command {
    pub const ALL: &'static [Self] = ALL;

    /// The command word without its leading slash.
    pub const fn word(self) -> &'static str {
        match self {
            Self::Demo => "demo",
            Self::Plugins => "plugins",
            Self::Models => "models",
            Self::Export => "export",
            Self::Resume => "resume",
            Self::Brainstorm => "brainstorm",
            Self::Help => "help",
        }
    }

    /// The action described by the picker in the current demo mode.
    pub const fn about(self, demo: bool) -> &'static str {
        match self {
            Self::Demo if demo => "Turn demo off",
            Self::Demo => "Turn demo on",
            Self::Plugins => "Manage plugins",
            Self::Models => "Choose model and reasoning level",
            Self::Export => "Export this conversation as ATIF",
            Self::Resume => "Resume a saved conversation",
            Self::Brainstorm => "Explicit public profile or reputation lookup",
            Self::Help => "Show commands and keys",
        }
    }
}

/// Commands matching a leading slash and an unfinished lowercase word.
pub fn matches(text: &str) -> Vec<Command> {
    let Some(word) = command_prefix(text) else {
        return Vec::new();
    };
    ALL.iter()
        .copied()
        .filter(|command| command.word().starts_with(word))
        .collect()
}

/// A known command written as an exact `/word`.
pub fn parse(text: &str) -> Option<Command> {
    if !is_command_word(text) {
        return None;
    }
    let word = &text[1..];
    ALL.iter().copied().find(|command| command.word() == word)
}

/// Help for the commands available in this build.
pub fn help() -> String {
    let mut text = String::new();
    text.push_str("/demo  Toggle demo/live\n");
    text.push_str("/plugins  Manage plugins\n/models  Choose a model for an enabled provider\n/export [path]  Export the selected conversation as ATIF\n/resume [number|id]  Resume a saved conversation\n/help  Show commands\nTab  Complete a command\nEsc  Close suggestions or stop a reply\nCtrl+C  Quit");
    text.push_str("\n/brainstorm search <public query>  Search public profiles\n/brainstorm rank <hex-or-npub>  Look up raw influence\nBrainstorm sends only explicit queries and public keys to its configured HTTPS origin.");
    text
}

/// Whether text is a complete command word, including an unknown one.
pub fn is_command_word(text: &str) -> bool {
    command_prefix(text).is_some_and(|word| !word.is_empty())
}

fn command_prefix(text: &str) -> Option<&str> {
    let word = text.strip_prefix('/')?;
    word.bytes()
        .all(|byte| byte.is_ascii_lowercase())
        .then_some(word)
}
