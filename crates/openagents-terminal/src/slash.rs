//! The slash commands: a closed list, and the one parse the screen makes
//! of what the person types.
//!
//! A draft is a command only when it is exactly `/word`, a slash and
//! lowercase letters with nothing after them. Everything else, including
//! text that merely starts with a slash (`/usr/bin is missing`), goes to the
//! chat router. Nothing here reads the words of a message.

/// One slash command.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Slash {
    New,
    Threads,
    Stop,
    Export,
    Settings,
    Connect,
    Plugins,
    Expand,
    Help,
    Quit,
}

impl Slash {
    /// Every command, in the order `/help` lists them.
    pub const ALL: [Slash; 10] = [
        Slash::New,
        Slash::Threads,
        Slash::Stop,
        Slash::Export,
        Slash::Settings,
        Slash::Connect,
        Slash::Plugins,
        Slash::Expand,
        Slash::Help,
        Slash::Quit,
    ];

    /// The word after the slash.
    pub const fn word(self) -> &'static str {
        match self {
            Slash::New => "new",
            Slash::Threads => "threads",
            Slash::Stop => "stop",
            Slash::Export => "export",
            Slash::Settings => "settings",
            Slash::Connect => "connect",
            Slash::Plugins => "plugins",
            Slash::Expand => "expand",
            Slash::Help => "help",
            Slash::Quit => "quit",
        }
    }

    /// What it does, for `/help`.
    pub const fn about(self) -> &'static str {
        match self {
            Slash::New => "start a new thread",
            Slash::Threads => "list threads to open, start, or archive (Ctrl+T)",
            Slash::Stop => "stop the reply or the Coder run (Esc)",
            Slash::Export => "save this thread as an ATIF trajectory file",
            Slash::Settings => "show the Coder settings and where the file is",
            Slash::Connect => "pair a phone with this computer by QR code",
            Slash::Plugins => "list published plugins",
            Slash::Expand => "expand or condense tool calls (Ctrl+O)",
            Slash::Help => "show these commands and keys",
            Slash::Quit => "close the screen; a Coder run keeps going",
        }
    }
}

/// What a submitted draft is.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Draft {
    /// A message for the chat.
    Message(String),
    /// A slash command.
    Command(Slash),
    /// `/word` that names no command.
    Unknown(String),
    /// Nothing to send.
    Empty,
}

/// Read a submitted draft.
pub fn parse(draft: &str) -> Draft {
    let text = draft.trim();
    if text.is_empty() {
        return Draft::Empty;
    }
    if let Some(word) = text.strip_prefix('/')
        && !word.is_empty()
        && word.bytes().all(|byte| byte.is_ascii_lowercase())
    {
        return match Slash::ALL.into_iter().find(|slash| slash.word() == word) {
            Some(slash) => Draft::Command(slash),
            None => Draft::Unknown(text.to_owned()),
        };
    }
    Draft::Message(text.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_an_exact_word_from_the_list_is_a_command() {
        for slash in Slash::ALL {
            assert_eq!(parse(&format!("/{}", slash.word())), Draft::Command(slash));
            assert_eq!(
                parse(&format!("  /{} ", slash.word())),
                Draft::Command(slash)
            );
        }
        assert_eq!(parse("/nope"), Draft::Unknown("/nope".into()));
        assert_eq!(parse(""), Draft::Empty);
        assert_eq!(parse("   "), Draft::Empty);
    }

    #[test]
    fn everything_else_goes_to_the_chat_unread() {
        for text in [
            "/usr/bin is missing",
            "/new thread please",
            "/Help",
            "start a new thread",
            "stop",
            "help",
            "please /quit",
        ] {
            assert_eq!(parse(text), Draft::Message(text.into()), "{text}");
        }
    }

    #[test]
    fn the_words_are_unique() {
        let mut words: Vec<&str> = Slash::ALL.iter().map(|slash| slash.word()).collect();
        words.sort_unstable();
        words.dedup();
        assert_eq!(words.len(), Slash::ALL.len());
    }
}
