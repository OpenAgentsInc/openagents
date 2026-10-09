//! The component catalog: every component an answer may use, its
//! arguments in positional order, and what each means. It is written once
//! here and gives the validator ([`crate::parse`]) and the prompt
//! description ([`prompt`]); each surface's renderer draws exactly these.

/// What an argument holds.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    /// Plain text. Bare `https://` URLs in it are drawn as links.
    Text,
    /// A link target: `https://...` or a site path such as `/download`.
    Href,
    /// One of a fixed set of words.
    Choice(&'static [&'static str]),
    /// A list of block components.
    Children,
    /// A list of one component, by name.
    Items(&'static str),
}

/// One argument.
#[derive(Clone, Copy, Debug)]
pub struct Prop {
    pub name: &'static str,
    pub kind: Kind,
    pub required: bool,
    pub about: &'static str,
}

/// One component.
#[derive(Clone, Copy, Debug)]
pub struct Component {
    pub name: &'static str,
    pub about: &'static str,
    /// Arguments in positional order: required ones first.
    pub props: &'static [Prop],
    /// Whether it may appear in a list of children; `Step` and `Tab` only
    /// appear inside `Steps` and `Tabs`.
    pub block: bool,
}

const fn prop(name: &'static str, kind: Kind, required: bool, about: &'static str) -> Prop {
    Prop {
        name,
        kind,
        required,
        about,
    }
}

/// Button styles.
pub const BUTTON_STYLES: &[&str] = &["primary", "secondary"];
/// Who a button is shown to.
pub const AUDIENCES: &[&str] = &["everyone", "signed_in", "signed_out"];

/// The catalog, in the order the prompt lists it.
pub const CATALOG: &[Component] = &[
    Component {
        name: "Stack",
        about: "Blocks one under another.",
        props: &[prop("children", Kind::Children, true, "the blocks")],
        block: true,
    },
    Component {
        name: "Columns",
        about: "Blocks side by side, one under another on a narrow screen.",
        props: &[prop(
            "children",
            Kind::Children,
            true,
            "the blocks, usually Cards",
        )],
        block: true,
    },
    Component {
        name: "Card",
        about: "A titled box holding a few blocks, such as one way to do something.",
        props: &[
            prop("title", Kind::Text, true, "a short heading"),
            prop("children", Kind::Children, false, "what the card holds"),
        ],
        block: true,
    },
    Component {
        name: "Text",
        about: "A short paragraph.",
        props: &[prop(
            "text",
            Kind::Text,
            true,
            "the words; URLs become links",
        )],
        block: true,
    },
    Component {
        name: "Link",
        about: "A text link.",
        props: &[
            prop("label", Kind::Text, true, "the words shown"),
            prop("href", Kind::Href, true, "where it goes"),
        ],
        block: true,
    },
    Component {
        name: "Button",
        about: "A button that opens a page and starts a flow there.",
        props: &[
            prop(
                "label",
                Kind::Text,
                true,
                "a verb phrase, such as \"Connect GitHub\"",
            ),
            prop("href", Kind::Href, true, "the page it opens"),
            prop(
                "style",
                Kind::Choice(BUTTON_STYLES),
                false,
                "primary (default) or secondary",
            ),
            prop(
                "show",
                Kind::Choice(AUDIENCES),
                false,
                "everyone (default), signed_in, or signed_out",
            ),
        ],
        block: true,
    },
    Component {
        name: "CodeBlock",
        about: "Code or a command with a Copy button.",
        props: &[
            prop("code", Kind::Text, true, "the exact text to copy"),
            prop("language", Kind::Text, false, "such as bash"),
        ],
        block: true,
    },
    Component {
        name: "Command",
        about: "One command to run, with a tab for macOS and Linux and one for Windows, each with a Copy button.",
        props: &[
            prop("unix", Kind::Text, true, "the command for macOS and Linux"),
            prop(
                "windows",
                Kind::Text,
                false,
                "the PowerShell command for Windows",
            ),
        ],
        block: true,
    },
    Component {
        name: "Steps",
        about: "Numbered steps.",
        props: &[prop(
            "steps",
            Kind::Items("Step"),
            true,
            "the Steps, in order",
        )],
        block: true,
    },
    Component {
        name: "Step",
        about: "One numbered step, inside Steps.",
        props: &[
            prop("title", Kind::Text, true, "what to do, in a few words"),
            prop(
                "children",
                Kind::Children,
                false,
                "a command, a button, or a line of text",
            ),
        ],
        block: false,
    },
    Component {
        name: "Tabs",
        about: "Tabs, one shown at a time.",
        props: &[prop("tabs", Kind::Items("Tab"), true, "the Tabs")],
        block: true,
    },
    Component {
        name: "Tab",
        about: "One tab, inside Tabs.",
        props: &[
            prop("label", Kind::Text, true, "the tab's name"),
            prop("children", Kind::Children, true, "what the tab shows"),
        ],
        block: false,
    },
    Component {
        name: "LinkCard",
        about: "A card that is one link: a title and a line about the page.",
        props: &[
            prop("title", Kind::Text, true, "the page's name"),
            prop("description", Kind::Text, true, "one line about it"),
            prop("href", Kind::Href, true, "the page"),
        ],
        block: true,
    },
];

/// The component named `name`.
#[must_use]
pub fn component(name: &str) -> Option<&'static Component> {
    CATALOG.iter().find(|c| c.name == name)
}

/// The catalog as a short description for a model's instructions: the
/// syntax, then each component's signature and meaning.
#[must_use]
pub fn prompt() -> String {
    let mut out = String::from(
        "To show interactive parts with an answer, add one fenced ```openui-lang block after a \
         one-line lead. Write one statement per line, `name = Component(args)`; the first is \
         `root`. Arguments go in the order below, or by name (`href=\"/download\"`). A name may be \
         used before the line that defines it. Strings use double quotes. Links are https:// \
         URLs or site paths such as /download. Use only these components:\n",
    );
    for component in CATALOG {
        let args: Vec<String> = component
            .props
            .iter()
            .map(|p| {
                if p.required {
                    p.name.to_owned()
                } else {
                    format!("{}?", p.name)
                }
            })
            .collect();
        out.push_str(&format!(
            "- {}({}): {}",
            component.name,
            args.join(", "),
            component.about
        ));
        let notes: Vec<String> = component
            .props
            .iter()
            .map(|p| format!("{} is {}", p.name, p.about))
            .collect();
        out.push_str(&format!(" ({}).\n", notes.join("; ")));
    }
    out
}
