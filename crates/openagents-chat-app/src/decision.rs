//! The decision panel: a question or an approval that a waiting Coder task
//! asked, answered one page at a time.
//!
//! This reimplements the paged question flow of Zeron's composer (public
//! MIT zeronsh/zeron at `9e1a1115`, `crates/ui/src/composer.rs`, `Wizard`
//! and `QuestionFlow`) in Rust Native: one page per question with a "1 of
//! 3" counter, number keys 1 to 9 pick an option, picking an option moves
//! to the next page, and typed text answers the page instead. No Zeron code
//! is copied.
//!
//! The host's questions are text (`coder::task::interaction`): a turn's
//! reply is the question. [`Flow::question`] reads that text the way a
//! person would. A numbered list of two to nine items is the options of the
//! question before it, and a later paragraph that asks something is a page
//! of its own. Any other text is one page with free text only.
//!
//! Zeron has no approval interface, so approvals are ours ([Agent Studio,
//! "Approvals are ours"]). An approval uses the same panel with the options
//! **Allow once** and **Deny**. Its answer is data for the engine: the next
//! turn runs under a fresh grant with every usual check, so an answer never
//! widens the task's grant.
//!
//! An approval whose step the host named ([`Prompt`]) shows the tool, the
//! exact command, a risk chip, the reason, and the working directory, after
//! AgentCraft's permission body (`PermissionBody.java`; reimplemented, not
//! copied). When the host offers a standing rule for the step, the panel
//! adds **Always allow** and shows the rule's exact text: the
//! host records that rule and applies it, never the panel
//! ([`Flow::always`]).
//!
//! [Agent Studio, "Approvals are ours"]: ../../../docs/verse/agent-studio.md

use rust_native::style::{Color, Space, Style, TextWeight};
use rust_native::{Axis, Element, Node, TextRole, markdown};

/// The most options one page offers: the number keys 1 to 9.
pub const MAX_OPTIONS: usize = 9;
/// The most pages one decision has; text that reads as more is one page.
pub const MAX_PAGES: usize = 8;
/// The longest option label, in characters.
const MAX_LABEL: usize = 200;
/// The answer an approval's **Allow once** sends.
pub const ALLOWED: &str = "Approved.";
/// The answer an approval's **Deny** sends.
pub const DENIED: &str = "Denied.";
/// The option that keeps a standing rule for the seat.
pub const ALWAYS: &str = "Always allow";
/// What a page the person passed over answers.
const NO_ANSWER: &str = "No answer.";
/// The panel's card color, the transcript's card.
const CARD: Color = Color::rgb(26, 29, 34);

/// What the waiting task asked for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    /// An open question, with options when the text lists them.
    Question,
    /// Approval of a step the engine named.
    Approval,
}

/// How much harm a step can do, as the host classified it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Risk {
    Low,
    Medium,
    High,
}

impl Risk {
    /// The chip's words.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Low => "Low risk",
            Self::Medium => "Medium risk",
            Self::High => "High risk",
        }
    }

    /// The chip's color.
    #[must_use]
    pub const fn color(self) -> Color {
        match self {
            Self::Low => Color::rgb(46, 92, 64),
            Self::Medium => Color::rgb(122, 92, 28),
            Self::High => Color::rgb(128, 40, 40),
        }
    }
}

/// The step an approval asks to take, as the host showed it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Prompt {
    /// The tool, such as `shell`.
    pub tool: String,
    /// The exact command.
    pub command: String,
    /// The absolute working directory.
    pub cwd: String,
    /// Why the engine asks; may be empty.
    pub reason: String,
    pub risk: Risk,
    /// The exact standing rule **Always allow** records, or
    /// `None` when the host offers none, as for a high-risk step.
    pub always: Option<String>,
}

impl Prompt {
    /// The prompt as Markdown, for a surface that draws text: the tool and
    /// its risk, the command in a code block, the reason, the directory,
    /// and what **Always allow** covers.
    #[must_use]
    pub fn markdown(&self) -> String {
        let fence = "`".repeat(longest_run(&self.command, '`').max(2) + 1);
        let mut out = format!(
            "**{}** · {}\n\n{fence}\n{}\n{fence}",
            self.tool,
            self.risk.label(),
            self.command
        );
        if !self.reason.trim().is_empty() {
            out.push_str(&format!("\n\n{}", self.reason.trim()));
        }
        out.push_str(&format!("\n\nIn `{}`", self.cwd));
        if let Some(rule) = &self.always {
            out.push_str(&format!("\n\n\"{ALWAYS}\" covers: {rule}"));
        }
        out
    }
}

/// The longest run of `ch` in `text`.
fn longest_run(text: &str, ch: char) -> usize {
    let mut longest = 0;
    let mut run = 0;
    for c in text.chars() {
        if c == ch {
            run += 1;
            longest = longest.max(run);
        } else {
            run = 0;
        }
    }
    longest
}

/// One question.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Page {
    /// The question, as Markdown.
    pub prompt: String,
    /// The options the question lists, at most [`MAX_OPTIONS`].
    pub options: Vec<String>,
}

/// What an interaction did.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Step {
    /// Nothing changed.
    Stay,
    /// The panel moved to another page.
    Moved,
    /// Every page is answered: send this text as the answer.
    Done(String),
}

/// What a panel control does.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Control {
    /// Pick the option at this index on the current page.
    Pick(usize),
    /// Go to the previous page.
    Back,
    /// Go to the next page, or submit on the last one.
    Next,
}

/// A decision in progress: its pages, the current page, and each page's
/// answer so far.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Flow {
    kind: Kind,
    /// An approval's named step.
    prompt: Option<Prompt>,
    pages: Vec<Page>,
    page: usize,
    picked: Vec<Option<usize>>,
    typed: Vec<String>,
}

/// A panel's view, and what each of its control keys does.
#[derive(Clone, Debug, PartialEq)]
pub struct Panel {
    pub node: Node<()>,
    pub controls: Vec<(String, Control)>,
}

impl Flow {
    /// The decision for a question whose text is `text`.
    #[must_use]
    pub fn question(text: &str) -> Self {
        Self::new(Kind::Question, parse(text))
    }

    /// The decision for an approval of the step `text` names.
    #[must_use]
    pub fn approval(text: &str) -> Self {
        Self::new(
            Kind::Approval,
            vec![Page {
                prompt: text.trim().into(),
                options: vec!["Allow once".into(), "Deny".into()],
            }],
        )
    }

    /// The decision for an approval whose step the host named: **Allow
    /// once**, **Always allow** when the host offers a
    /// standing rule, and **Deny**.
    #[must_use]
    pub fn approval_step(text: &str, prompt: Prompt) -> Self {
        let mut options = vec!["Allow once".to_owned()];
        if prompt.always.is_some() {
            options.push(ALWAYS.into());
        }
        options.push("Deny".into());
        let mut flow = Self::new(
            Kind::Approval,
            vec![Page {
                prompt: text.trim().into(),
                options,
            }],
        );
        flow.prompt = Some(prompt);
        flow
    }

    fn new(kind: Kind, pages: Vec<Page>) -> Self {
        let count = pages.len();
        Self {
            kind,
            prompt: None,
            pages,
            page: 0,
            picked: vec![None; count],
            typed: vec![String::new(); count],
        }
    }

    #[must_use]
    pub const fn kind(&self) -> Kind {
        self.kind
    }

    /// An approval's named step, if the host named one.
    #[must_use]
    pub fn prompt(&self) -> Option<&Prompt> {
        self.prompt.as_ref()
    }

    /// The standing rule the person chose with **Always allow for this
    /// seat**: the exact text the host offered, which the client sends
    /// back for the host to record. `None` for any other answer.
    #[must_use]
    pub fn always(&self) -> Option<&str> {
        if self.kind != Kind::Approval || self.picked_label(0) != Some(ALWAYS) {
            return None;
        }
        self.prompt.as_ref()?.always.as_deref()
    }

    /// The label of the option picked on page `page`.
    fn picked_label(&self, page: usize) -> Option<&str> {
        let index = self.picked[page]?;
        self.pages[page].options.get(index).map(String::as_str)
    }

    #[must_use]
    pub fn pages(&self) -> &[Page] {
        &self.pages
    }

    /// The current page's index.
    #[must_use]
    pub const fn page(&self) -> usize {
        self.page
    }

    /// The current page.
    #[must_use]
    pub fn current(&self) -> &Page {
        &self.pages[self.page]
    }

    /// Where the panel is, as "2 of 3".
    #[must_use]
    pub fn counter(&self) -> String {
        format!("{} of {}", self.page + 1, self.pages.len())
    }

    /// The option picked on the current page, if any.
    #[must_use]
    pub fn picked(&self) -> Option<usize> {
        self.picked[self.page]
    }

    /// Whether number key `number` picks an option on the current page.
    #[must_use]
    pub fn takes_number(&self, number: usize) -> bool {
        (1..=self.current().options.len()).contains(&number)
    }

    /// Pick option `index` on the current page. A pick replaces the page's
    /// typed answer and moves on: to the next page, or, on the last one, to
    /// the answer.
    pub fn select(&mut self, index: usize) -> Step {
        if index >= self.current().options.len() {
            return Step::Stay;
        }
        self.picked[self.page] = Some(index);
        self.typed[self.page].clear();
        self.advance()
    }

    /// Number key `number`, 1 to 9.
    pub fn press_number(&mut self, number: usize) -> Step {
        if self.takes_number(number) {
            self.select(number - 1)
        } else {
            Step::Stay
        }
    }

    /// Answer the current page with typed `text` and move on. Typed text
    /// replaces a picked option; blank text keeps the page's pick, and is
    /// no answer on a page without one.
    pub fn answer_typed(&mut self, text: &str) -> Step {
        let text = text.trim();
        if text.is_empty() && self.picked[self.page].is_none() {
            return Step::Stay;
        }
        if !text.is_empty() {
            text.clone_into(&mut self.typed[self.page]);
            self.picked[self.page] = None;
        }
        self.advance()
    }

    /// Go to the next page, or, on the last page, finish with the answer.
    pub fn advance(&mut self) -> Step {
        if self.page + 1 < self.pages.len() {
            self.page += 1;
            Step::Moved
        } else {
            Step::Done(self.answer())
        }
    }

    /// Go to the previous page; `false` on the first.
    pub fn back(&mut self) -> bool {
        if self.page == 0 {
            return false;
        }
        self.page -= 1;
        true
    }

    /// Carry out a panel control.
    pub fn control(&mut self, control: Control) -> Step {
        match control {
            Control::Pick(index) => self.select(index),
            Control::Back => {
                if self.back() {
                    Step::Moved
                } else {
                    Step::Stay
                }
            }
            Control::Next => self.advance(),
        }
    }

    /// The answer text the task receives. An approval sends
    /// [`ALLOWED`] or [`DENIED`]. A one-page question sends its answer; a
    /// longer one sends one numbered line per page, in page order.
    #[must_use]
    pub fn answer(&self) -> String {
        if self.kind == Kind::Approval {
            return match self.picked_label(0) {
                Some("Allow once" | ALWAYS) => ALLOWED,
                _ => DENIED,
            }
            .into();
        }
        let answers: Vec<String> = (0..self.pages.len())
            .map(|page| self.page_answer(page))
            .collect();
        if let [only] = answers.as_slice() {
            return only.clone();
        }
        answers
            .iter()
            .enumerate()
            .map(|(index, answer)| format!("{}. {answer}", index + 1))
            .collect::<Vec<_>>()
            .join("\n")
    }

    fn page_answer(&self, page: usize) -> String {
        if !self.typed[page].is_empty() {
            return self.typed[page].clone();
        }
        self.picked[page]
            .and_then(|index| self.pages[page].options.get(index))
            .cloned()
            .unwrap_or_else(|| NO_ANSWER.into())
    }

    /// The panel as a Rust Native card. Its keys start with `prefix`: the
    /// card is `{prefix}-decision`, an approval's options are
    /// `{prefix}-approve` and `{prefix}-deny`, a question's are
    /// `{prefix}-option-{n}` from 1, and the paging controls are
    /// `{prefix}-decision-back` and `{prefix}-decision-next`. `enabled` is
    /// whether the controls take presses now.
    #[must_use]
    pub fn view(&self, prefix: &str, enabled: bool) -> Panel {
        let key = format!("{prefix}-decision");
        let mut controls = Vec::new();
        let page = self.current();
        let mut title = match self.kind {
            Kind::Question => "Coder asks".to_owned(),
            Kind::Approval => "Coder asks to go ahead".to_owned(),
        };
        if self.pages.len() > 1 {
            title = format!("{title} · {}", self.counter());
        }
        let mut children = vec![Node {
            key: format!("{key}-title"),
            style: Style {
                weight: Some(TextWeight::Bold),
                ..Style::default()
            },
            element: Element::Text {
                value: title,
                role: TextRole::Body,
            },
        }];
        if !page.prompt.is_empty() {
            children.push(Node {
                key: format!("{key}-prompt"),
                style: Style::default(),
                element: Element::Markdown {
                    blocks: markdown::parse(&page.prompt),
                },
            });
        }
        if let Some(prompt) = &self.prompt {
            children.extend(step_nodes(&key, prompt));
        }
        let mut options = Vec::new();
        for (index, label) in page.options.iter().enumerate() {
            let option_key = match (self.kind, label.as_str()) {
                (Kind::Approval, "Allow once") => format!("{prefix}-approve"),
                (Kind::Approval, ALWAYS) => format!("{prefix}-always"),
                (Kind::Approval, _) => format!("{prefix}-deny"),
                (Kind::Question, _) => format!("{prefix}-option-{}", index + 1),
            };
            controls.push((option_key.clone(), Control::Pick(index)));
            options.push(button(
                &option_key,
                label,
                Some((index + 1).to_string()),
                enabled,
            ));
        }
        if !options.is_empty() {
            children.push(Node {
                key: format!("{key}-options"),
                style: Style {
                    gap: Some(Space::Sm),
                    ..Style::default()
                },
                element: Element::Stack {
                    axis: match self.kind {
                        Kind::Approval => Axis::Horizontal,
                        Kind::Question => Axis::Vertical,
                    },
                    children: options,
                },
            });
        }
        if self.kind == Kind::Question
            && let Some(answer) = self.answered(self.page)
        {
            children.push(status(
                &format!("{key}-answer"),
                &format!("Your answer: {answer}"),
            ));
        }
        children.push(status(
            &format!("{key}-hint"),
            match self.kind {
                Kind::Approval if self.always_offered() => {
                    "Always allow saves the rule above on your computer. Risky steps still ask every time."
                }
                Kind::Approval => {
                    "You can also answer in your own words below."
                }
                Kind::Question if page.options.is_empty() => "Answer below.",
                Kind::Question => "Pick an option or press its number, or type your own answer below.",
            },
        ));
        if self.pages.len() > 1 {
            let mut paging = Vec::new();
            if self.page > 0 {
                let back = format!("{key}-back");
                controls.push((back.clone(), Control::Back));
                paging.push(button(&back, "Back", None, enabled));
            }
            let next = format!("{key}-next");
            controls.push((next.clone(), Control::Next));
            paging.push(button(
                &next,
                if self.page + 1 < self.pages.len() {
                    "Next"
                } else {
                    "Send answers"
                },
                None,
                enabled,
            ));
            children.push(Node {
                key: format!("{key}-paging"),
                style: Style {
                    gap: Some(Space::Sm),
                    ..Style::default()
                },
                element: Element::Stack {
                    axis: Axis::Horizontal,
                    children: paging,
                },
            });
        }
        Panel {
            node: Node {
                key,
                style: Style {
                    background: Some(CARD),
                    padding_top: Some(Space::Md),
                    padding_bottom: Some(Space::Md),
                    padding_start: Some(Space::Md),
                    padding_end: Some(Space::Md),
                    gap: Some(Space::Xs),
                    ..Style::default()
                },
                element: Element::Stack {
                    axis: Axis::Vertical,
                    children,
                },
            },
            controls,
        }
    }

    /// Whether the host offered a standing rule for this approval.
    fn always_offered(&self) -> bool {
        self.prompt
            .as_ref()
            .is_some_and(|prompt| prompt.always.is_some())
    }

    /// The page's answer so far, if it has one.
    fn answered(&self, page: usize) -> Option<String> {
        (!self.typed[page].is_empty() || self.picked[page].is_some())
            .then(|| self.page_answer(page))
    }
}

/// A control as wide as its words, as the transcript draws Coder's
/// controls.
fn button(key: &str, label: &str, shortcut: Option<String>, enabled: bool) -> Node<()> {
    Node {
        key: key.into(),
        style: Style {
            intrinsic_width: Some(true),
            ..Style::default()
        },
        element: Element::Button {
            label: label.into(),
            enabled,
            icon: None,
            shortcut,
            intent: (),
        },
    }
}

/// An approval's named step: the tool with its risk chip, the command,
/// the reason, the directory, and what a standing rule covers.
fn step_nodes(key: &str, prompt: &Prompt) -> Vec<Node<()>> {
    let tool = Node {
        key: format!("{key}-tool"),
        style: Style {
            weight: Some(TextWeight::Bold),
            ..Style::default()
        },
        element: Element::Text {
            value: prompt.tool.clone(),
            role: TextRole::Body,
        },
    };
    let chip = Node {
        key: format!("{key}-risk"),
        style: Style {
            background: Some(prompt.risk.color()),
            radius: Some(6),
            padding_start: Some(Space::Xs),
            padding_end: Some(Space::Xs),
            intrinsic_width: Some(true),
            ..Style::default()
        },
        element: Element::Text {
            value: prompt.risk.label().into(),
            role: TextRole::Status,
        },
    };
    let mut nodes = vec![
        Node {
            key: format!("{key}-step"),
            style: Style {
                gap: Some(Space::Sm),
                ..Style::default()
            },
            element: Element::Stack {
                axis: Axis::Horizontal,
                children: vec![tool, chip],
            },
        },
        Node {
            key: format!("{key}-command"),
            style: Style {
                border: Some(prompt.risk.color()),
                padding_start: Some(Space::Sm),
                ..Style::default()
            },
            element: Element::Text {
                value: prompt.command.clone(),
                role: TextRole::Code,
            },
        },
    ];
    if !prompt.reason.trim().is_empty() {
        nodes.push(Node {
            key: format!("{key}-reason"),
            style: Style::default(),
            element: Element::Text {
                value: prompt.reason.trim().into(),
                role: TextRole::Body,
            },
        });
    }
    nodes.push(status(&format!("{key}-cwd"), &format!("In {}", prompt.cwd)));
    if let Some(rule) = &prompt.always {
        nodes.push(status(
            &format!("{key}-covers"),
            &format!("\"{ALWAYS}\" covers: {rule}"),
        ));
    }
    nodes
}

fn status(key: &str, text: &str) -> Node<()> {
    Node {
        key: key.into(),
        style: Style::default(),
        element: Element::Text {
            value: text.into(),
            role: TextRole::Status,
        },
    }
}

/// The pages a question's text asks. A numbered list, `1.` or `1)` from 1
/// in order, of two to [`MAX_OPTIONS`] items is the options of the text
/// before it. After a list, a paragraph that asks something (it holds a
/// `?`) starts the next page, and any other text stays with the page.
/// Text that reads as no page or as more than [`MAX_PAGES`] is one page,
/// with no options.
#[must_use]
pub fn parse(text: &str) -> Vec<Page> {
    let whole = || {
        vec![Page {
            prompt: text.trim().into(),
            options: vec![],
        }]
    };
    let mut pages = Vec::new();
    let mut prompt: Vec<&str> = Vec::new();
    let mut options: Vec<&str> = Vec::new();
    let mut lines = text.lines().peekable();
    while let Some(line) = lines.next() {
        if let Some(option) = numbered(line, options.len() + 1) {
            options.push(option);
            continue;
        }
        if !options.is_empty() {
            if line.trim().is_empty() {
                continue;
            }
            // The paragraph this line opens.
            let mut paragraph = vec![line];
            while let Some(next) = lines.peek() {
                if next.trim().is_empty() || numbered(next, 1).is_some() {
                    break;
                }
                paragraph.push(lines.next().unwrap_or_default());
            }
            if paragraph.iter().any(|line| line.contains('?')) {
                close(&mut pages, &mut prompt, &mut options);
                prompt = paragraph;
            } else {
                prompt.push("");
                prompt.extend(paragraph);
            }
            continue;
        }
        prompt.push(line);
    }
    close(&mut pages, &mut prompt, &mut options);
    if pages.is_empty() || pages.len() > MAX_PAGES {
        return whole();
    }
    pages
}

/// End the page being read: a list of fewer than two items is part of the
/// prompt rather than options.
fn close(pages: &mut Vec<Page>, prompt: &mut Vec<&str>, options: &mut Vec<&str>) {
    let mut text = prompt.join("\n");
    let options = std::mem::take(options);
    let options = if options.len() >= 2 {
        options.iter().map(|option| clip(option)).collect()
    } else {
        for (index, option) in options.iter().enumerate() {
            text.push_str(&format!("\n{}. {option}", index + 1));
        }
        vec![]
    };
    prompt.clear();
    let text = text.trim();
    if !text.is_empty() || !options.is_empty() {
        pages.push(Page {
            prompt: text.into(),
            options,
        });
    }
}

/// The item text of `line` when it is item `number` of a numbered list.
fn numbered(line: &str, number: usize) -> Option<&str> {
    if number > MAX_OPTIONS {
        return None;
    }
    let rest = line.trim_start().strip_prefix(&number.to_string())?;
    let rest = rest.strip_prefix('.').or_else(|| rest.strip_prefix(')'))?;
    if !rest.starts_with(char::is_whitespace) {
        return None;
    }
    let item = rest.trim();
    (!item.is_empty()).then_some(item)
}

fn clip(text: &str) -> String {
    if text.chars().count() <= MAX_LABEL {
        return text.into();
    }
    let mut out: String = text.chars().take(MAX_LABEL).collect();
    out.push('…');
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const TWO: &str = "I found two failing tests.\n\nWhich should I fix first?\n1. test_slugify\n2. test_unicode\n\nShould I also add a regression test?\n1) Yes\n2) No";

    #[test]
    fn a_numbered_list_is_the_options_and_a_later_question_its_own_page() {
        let pages = parse(TWO);
        assert_eq!(pages.len(), 2);
        assert_eq!(
            pages[0].prompt,
            "I found two failing tests.\n\nWhich should I fix first?"
        );
        assert_eq!(pages[0].options, ["test_slugify", "test_unicode"]);
        assert_eq!(pages[1].prompt, "Should I also add a regression test?");
        assert_eq!(pages[1].options, ["Yes", "No"]);
    }

    #[test]
    fn plain_text_is_one_page_of_free_text() {
        for text in [
            "Should the test cover empty input too?",
            "Which file?\n1. Only one item",
            "Steps:\n2. starts at two\n3. three",
        ] {
            let pages = parse(text);
            assert_eq!(pages.len(), 1, "{text}");
            assert!(pages[0].options.is_empty(), "{text}");
            assert_eq!(pages[0].prompt, text.trim(), "{text}");
        }
    }

    #[test]
    fn text_after_the_list_that_asks_nothing_stays_with_the_page() {
        let pages = parse("Pick one:\n1. A\n2. B\n\nEither works for me.");
        assert_eq!(pages.len(), 1);
        assert_eq!(pages[0].options, ["A", "B"]);
        assert!(pages[0].prompt.ends_with("Either works for me."));
    }

    #[test]
    fn paging_moves_forward_and_back_and_keeps_answers() {
        let mut flow = Flow::question(TWO);
        assert_eq!(flow.counter(), "1 of 2");
        assert!(!flow.back(), "the first page has no previous page");
        assert_eq!(flow.select(1), Step::Moved, "a pick advances");
        assert_eq!(flow.counter(), "2 of 2");
        assert!(flow.back());
        assert_eq!(flow.picked(), Some(1), "the earlier pick stays");
        assert_eq!(flow.control(Control::Next), Step::Moved);
        assert_eq!(flow.control(Control::Back), Step::Moved);
        assert_eq!(flow.control(Control::Back), Step::Stay);
    }

    #[test]
    fn number_keys_pick_only_listed_options() {
        let mut flow = Flow::question(TWO);
        assert_eq!(flow.press_number(0), Step::Stay);
        assert_eq!(flow.press_number(3), Step::Stay, "out of range");
        assert!(!flow.takes_number(9));
        assert_eq!(flow.press_number(2), Step::Moved);
        assert_eq!(
            flow.press_number(1),
            Step::Done("1. test_unicode\n2. Yes".into())
        );
        let mut free = Flow::question("What should the slug be?");
        assert!(!free.takes_number(1), "no options, so 1 is text");
        assert_eq!(free.press_number(1), Step::Stay);
    }

    #[test]
    fn typed_text_overrides_a_pick_and_submits_in_page_order() {
        let mut flow = Flow::question(TWO);
        assert_eq!(flow.answer_typed("   "), Step::Stay, "blank is no answer");
        assert_eq!(flow.select(0), Step::Moved);
        assert!(flow.back());
        assert_eq!(flow.answer_typed("both, unicode first"), Step::Moved);
        assert_eq!(
            flow.control(Control::Next),
            Step::Done("1. both, unicode first\n2. No answer.".into())
        );
        let mut one = Flow::question("Should the test cover empty input too?");
        assert_eq!(
            one.answer_typed(" Yes, and None. "),
            Step::Done("Yes, and None.".into())
        );
    }

    #[test]
    fn an_approval_offers_allow_once_and_deny_only() {
        let mut flow = Flow::approval("May I delete slugs.py?");
        assert_eq!(flow.kind(), Kind::Approval);
        assert_eq!(flow.current().options, ["Allow once", "Deny"]);
        assert!(!flow.takes_number(3), "no standing approval");
        assert_eq!(flow.press_number(1), Step::Done(ALLOWED.into()));
        let mut flow = Flow::approval("May I delete slugs.py?");
        assert_eq!(flow.select(1), Step::Done(DENIED.into()));
        let panel = Flow::approval("May I push?").view("task", true);
        let keys: Vec<_> = panel.controls.iter().map(|(key, _)| key.as_str()).collect();
        assert_eq!(keys, ["task-approve", "task-deny"]);
    }

    fn prompt(risk: Risk, always: Option<&str>) -> Prompt {
        Prompt {
            tool: "shell".into(),
            command: "cargo test -p coder".into(),
            cwd: "/work/repo".into(),
            reason: "checks the change".into(),
            risk,
            always: always.map(str::to_owned),
        }
    }

    const RULE: &str = "ada may run shell `cargo test -p coder` in /work/repo without asking again";

    #[test]
    fn a_named_step_shows_its_command_risk_reason_and_directory() {
        let flow = Flow::approval_step("May I run the tests?", prompt(Risk::Low, Some(RULE)));
        assert_eq!(
            flow.current().options,
            ["Allow once", ALWAYS, "Deny"],
            "the host offered a rule"
        );
        let panel = flow.view("task", true);
        let view = rust_native::View::new("decision", 1, panel.node.clone());
        assert!(view.validate().is_ok());
        let keys: Vec<_> = panel.controls.iter().map(|(key, _)| key.as_str()).collect();
        assert_eq!(keys, ["task-approve", "task-always", "task-deny"]);
        let text = format!("{:?}", panel.node);
        for shown in [
            "task-decision-risk",
            "Low risk",
            "cargo test -p coder",
            "checks the change",
            "In /work/repo",
            "covers: ada may run shell",
        ] {
            assert!(text.contains(shown), "{shown}: {text}");
        }
        let markdown = flow.prompt().unwrap().markdown();
        assert!(markdown.starts_with("**shell** · Low risk"), "{markdown}");
        assert!(
            markdown.contains("```\ncargo test -p coder\n```"),
            "{markdown}"
        );
        assert!(markdown.contains(RULE), "{markdown}");
    }

    #[test]
    fn always_allow_sends_the_offered_rule_and_allow_once_none() {
        let mut flow = Flow::approval_step("May I?", prompt(Risk::Medium, Some(RULE)));
        assert_eq!(flow.press_number(2), Step::Done(ALLOWED.into()));
        assert_eq!(flow.always(), Some(RULE));
        let mut once = Flow::approval_step("May I?", prompt(Risk::Medium, Some(RULE)));
        assert_eq!(once.press_number(1), Step::Done(ALLOWED.into()));
        assert_eq!(once.always(), None);
        let mut deny = Flow::approval_step("May I?", prompt(Risk::Medium, Some(RULE)));
        assert_eq!(deny.press_number(3), Step::Done(DENIED.into()));
        assert_eq!(deny.always(), None);
    }

    #[test]
    fn a_step_without_a_rule_offers_no_standing_approval() {
        let mut flow = Flow::approval_step("May I push?", prompt(Risk::High, None));
        assert_eq!(flow.current().options, ["Allow once", "Deny"]);
        let text = format!("{:?}", flow.view("task", true).node);
        assert!(text.contains("High risk"), "{text}");
        assert!(!text.contains("covers:"), "{text}");
        assert_eq!(flow.press_number(2), Step::Done(DENIED.into()));
        assert_eq!(flow.always(), None);
    }

    #[test]
    fn the_panel_draws_the_page_its_options_and_paging() {
        let mut flow = Flow::question(TWO);
        let panel = flow.view("coder", true);
        let view = rust_native::View::new("decision", 1, panel.node.clone());
        assert!(view.validate().is_ok());
        assert_eq!(
            panel.controls,
            [
                ("coder-option-1".to_owned(), Control::Pick(0)),
                ("coder-option-2".to_owned(), Control::Pick(1)),
                ("coder-decision-next".to_owned(), Control::Next),
            ]
        );
        let text = format!("{:?}", panel.node);
        assert!(text.contains("Coder asks · 1 of 2"), "{text}");
        flow.select(0);
        let panel = flow.view("coder", false);
        assert!(
            panel
                .controls
                .contains(&("coder-decision-back".into(), Control::Back))
        );
        let text = format!("{:?}", panel.node);
        assert!(text.contains("Send answers"), "{text}");
        assert!(text.contains("enabled: false"), "{text}");
    }
}
