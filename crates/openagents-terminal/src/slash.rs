//! The slash commands: a closed list, and the one parse the screen makes
//! of what the person types.
//!
//! A draft is a command only when it is exactly `/word`, a slash and
//! lowercase letters with nothing after them, `/open` and a number, or
//! `/resume` and what follows it. Everything else, including text that
//! merely starts with a slash (`/usr/bin is missing`), goes to the chat
//! router. Nothing here reads the words of a message.

/// One slash command.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Slash {
    New,
    Resume,
    Threads,
    Stop,
    Export,
    Settings,
    Connect,
    Plugins,
    Background,
    Import,
    Expand,
    Run,
    Open,
    Help,
    Quit,
}

impl Slash {
    /// Every command, in the order `/help` lists them.
    pub const ALL: [Slash; 15] = [
        Slash::New,
        Slash::Resume,
        Slash::Threads,
        Slash::Stop,
        Slash::Export,
        Slash::Settings,
        Slash::Connect,
        Slash::Plugins,
        Slash::Background,
        Slash::Import,
        Slash::Expand,
        Slash::Run,
        Slash::Open,
        Slash::Help,
        Slash::Quit,
    ];

    /// The word after the slash.
    pub const fn word(self) -> &'static str {
        match self {
            Slash::New => "new",
            Slash::Resume => "resume",
            Slash::Threads => "threads",
            Slash::Stop => "stop",
            Slash::Export => "export",
            Slash::Settings => "settings",
            Slash::Connect => "connect",
            Slash::Plugins => "plugins",
            Slash::Background => "background",
            Slash::Import => "import",
            Slash::Expand => "expand",
            Slash::Run => "run",
            Slash::Open => "open",
            Slash::Help => "help",
            Slash::Quit => "quit",
        }
    }

    /// The argument it takes after its word, if any.
    pub const fn argument(self) -> Option<&'static str> {
        match self {
            Slash::Resume => Some("[ID or title]"),
            _ => None,
        }
    }

    /// How `/help` writes it: `/word`, and its argument.
    pub fn usage(self) -> String {
        match self.argument() {
            Some(argument) => format!("/{} {argument}", self.word()),
            None => format!("/{}", self.word()),
        }
    }

    /// What it does, for `/help`.
    pub const fn about(self) -> &'static str {
        match self {
            Slash::New => "start a new thread (Ctrl+N)",
            Slash::Resume => "pick a thread to open; /resume ID or title opens that one",
            Slash::Threads => "list threads to open, start, or archive (Ctrl+T)",
            Slash::Stop => "stop the reply or the Coder run (Esc)",
            Slash::Export => "save this thread as an ATIF trajectory file",
            Slash::Settings => "change when Coder starts and which coding agents it may use",
            Slash::Connect => "pair a phone with this computer by QR code",
            Slash::Plugins => "list plugins and run one installed here",
            Slash::Background => {
                "the background rules, such as disk cleanup: show, run, pause, log"
            }
            Slash::Import => "copy this computer's Claude Code and Codex sessions in as threads",
            Slash::Expand => "expand or condense tool calls and a run's changes (Ctrl+O)",
            Slash::Run => {
                "open the Coder run full screen, to watch it and send it messages (Ctrl+R)"
            }
            Slash::Open => "open a Coder run from the rail full screen: /open 2 (Alt+2)",
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
    /// `/open` and a run's number.
    Open(usize),
    /// A slash command with its argument (`/resume ID`).
    With(Slash, String),
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
    // `/open` takes a number.
    if let Some(number) = text.strip_prefix("/open ")
        && let Ok(number) = number.trim().parse::<usize>()
    {
        return Draft::Open(number);
    }
    if let Some(rest) = text.strip_prefix('/')
        && let Some((word, argument)) = rest.split_once(char::is_whitespace)
        && let Some(slash) = Slash::ALL
            .into_iter()
            .find(|slash| slash.argument().is_some() && slash.word() == word)
    {
        return Draft::With(slash, argument.trim().to_owned());
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
        assert_eq!(parse("/open 2"), Draft::Open(2));
        assert_eq!(parse(" /open  12 "), Draft::Open(12));
        assert_eq!(parse("/open two"), Draft::Message("/open two".into()));
        assert_eq!(parse("/resum"), Draft::Unknown("/resum".into()));
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
    fn resume_takes_an_id_or_a_title() {
        assert_eq!(parse("/resume"), Draft::Command(Slash::Resume));
        assert_eq!(
            parse(" /resume  Fix the parser "),
            Draft::With(Slash::Resume, "Fix the parser".into())
        );
        assert_eq!(
            parse("/resume 0a1b"),
            Draft::With(Slash::Resume, "0a1b".into())
        );
        assert_eq!(Slash::Resume.usage(), "/resume [ID or title]");
    }

    #[test]
    fn the_words_are_unique() {
        let mut words: Vec<&str> = Slash::ALL.iter().map(|slash| slash.word()).collect();
        words.sort_unstable();
        words.dedup();
        assert_eq!(words.len(), Slash::ALL.len());
    }
}
