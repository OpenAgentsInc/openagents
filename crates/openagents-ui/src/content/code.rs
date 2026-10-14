//! Code blocks and the sticky action bar that heads them.

use maud::{Markup, Render, html};

use crate::actions::{ButtonVariant, Color, ControlSize, CopyButton};

/// A toolbar that sticks to the top of its scroll container while its
/// block is in view: `div.oa-sticky-action-bar`. A code block uses one as
/// its header (language on the left, copy on the right); other content can
/// use it too. Set `--sticky-action-bar-top` to clear a fixed page header.
#[derive(Clone, Debug, Default)]
pub struct StickyActionBar {
    label: Option<Markup>,
    actions: Vec<Markup>,
    aria_label: Option<String>,
}

impl StickyActionBar {
    /// An empty bar.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// The leading label (text is escaped).
    #[must_use]
    pub fn label(mut self, label: impl Render) -> Self {
        self.label = Some(label.render());
        self
    }

    /// Appends a trailing action, such as a button.
    #[must_use]
    pub fn action(mut self, action: impl Render) -> Self {
        self.actions.push(action.render());
        self
    }

    /// The bar's accessible name, used when it has actions.
    #[must_use]
    pub fn aria_label(mut self, label: impl Into<String>) -> Self {
        self.aria_label = Some(label.into());
        self
    }
}

impl Render for StickyActionBar {
    fn render(&self) -> Markup {
        let has_actions = !self.actions.is_empty();
        let role = has_actions.then_some("toolbar");
        let aria = if has_actions {
            self.aria_label.as_deref()
        } else {
            None
        };
        html! {
            div.oa-sticky-action-bar role=[role] aria-label=[aria] {
                @if let Some(label) = &self.label {
                    span.oa-sticky-action-bar__label { (label) }
                }
                @if has_actions {
                    div.oa-sticky-action-bar__actions {
                        @for action in &self.actions { (action) }
                    }
                }
            }
        }
    }
}

/// A block of code, terminal output, a log or a diff: a surface
/// (`div.oa-code-block`), an optional sticky header with a title or the
/// language and its actions, and a scrolling pane with `pre > code`. Code
/// is monospace and never wraps unless [`wrap`] is set; long lines scroll
/// sideways inside the pane.
///
/// The copy action is a [`CopyButton`] labelled "Copy" that carries the
/// code in `data-oa-copy`; the CopyButton script hook
/// ([`crate::actions::COPY_BUTTON_JS`], part of [`crate::script()`])
/// copies it in one click, with no inline script. Leave it off
/// ([`copyable`]) on a page that runs no script.
///
/// Lines: [`line_numbers`], [`diff`] and [`highlight_lines`] draw the code
/// one `span.oa-code-line` per line. Numbers are drawn by CSS from
/// `data-line`, so selecting and copying the text never picks them up.
/// [`wrap_toggle`] adds a Wrap switch to the header that works by CSS
/// alone (a checkbox), so it works without script. [`size`] `Sm` is the
/// compact size for tool output inside a [`super::Trace`]: smaller type and
/// a pane that scrolls past about sixteen lines.
///
/// [`wrap`]: CodeBlock::wrap
/// [`copyable`]: CodeBlock::copyable
/// [`line_numbers`]: CodeBlock::line_numbers
/// [`diff`]: CodeBlock::diff
/// [`highlight_lines`]: CodeBlock::highlight_lines
/// [`wrap_toggle`]: CodeBlock::wrap_toggle
/// [`size`]: CodeBlock::size
#[derive(Clone, Debug)]
pub struct CodeBlock {
    code: String,
    highlighted: Option<Markup>,
    language: Option<String>,
    title: Option<String>,
    copyable: bool,
    copy_icon: Option<Markup>,
    wrap: bool,
    wrap_toggle: bool,
    line_numbers: bool,
    first_line: usize,
    diff: bool,
    highlights: Vec<usize>,
    size: CodeSize,
}

/// A [`CodeBlock`]'s size (`data-size`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum CodeSize {
    /// The reading size, for code in a message.
    #[default]
    Md,
    /// The compact size, for tool output inside a trace.
    Sm,
}

/// One replacement (`old` became `new`) as a unified-looking diff for a
/// [`CodeBlock::diff`] block: a hunk line, the lines both share at the
/// start and the end as context, then what was removed and added. Also
/// returns the counts of added and removed lines.
#[must_use]
pub fn replacement_diff(old: &str, new: &str) -> (String, usize, usize) {
    let old_lines: Vec<&str> = if old.is_empty() {
        Vec::new()
    } else {
        old.lines().collect()
    };
    let new_lines: Vec<&str> = if new.is_empty() {
        Vec::new()
    } else {
        new.lines().collect()
    };
    let head = old_lines
        .iter()
        .zip(&new_lines)
        .take_while(|(a, b)| a == b)
        .count();
    let tail = old_lines[head..]
        .iter()
        .rev()
        .zip(new_lines[head..].iter().rev())
        .take_while(|(a, b)| a == b)
        .count();
    let mut out = format!("@@ -{} +{} @@\n", old_lines.len(), new_lines.len());
    for l in &old_lines[..head] {
        out.push_str(&format!(" {l}\n"));
    }
    let removed = &old_lines[head..old_lines.len() - tail];
    let added = &new_lines[head..new_lines.len() - tail];
    for l in removed {
        out.push_str(&format!("-{l}\n"));
    }
    for l in added {
        out.push_str(&format!("+{l}\n"));
    }
    for l in &old_lines[old_lines.len() - tail..] {
        out.push_str(&format!(" {l}\n"));
    }
    (out, added.len(), removed.len())
}

/// How a diff line reads (`data-diff`).
fn diff_kind(line: &str) -> Option<&'static str> {
    if line.starts_with("+++") || line.starts_with("---") {
        Some("file")
    } else if line.starts_with("@@") {
        Some("hunk")
    } else if line.starts_with('+') {
        Some("add")
    } else if line.starts_with('-') {
        Some("del")
    } else {
        None
    }
}

impl CodeBlock {
    /// A block showing `code` as escaped text.
    #[must_use]
    pub fn new(code: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            highlighted: None,
            language: None,
            title: None,
            copyable: true,
            copy_icon: None,
            wrap: false,
            wrap_toggle: false,
            line_numbers: false,
            first_line: 1,
            diff: false,
            highlights: Vec::new(),
            size: CodeSize::Md,
        }
    }

    /// The language: shown in the header (unless a [`title`] is set) and
    /// set as `language-<name>` on the `code` element. Characters outside
    /// `[A-Za-z0-9+#._-]` are dropped.
    ///
    /// [`title`]: CodeBlock::title
    #[must_use]
    pub fn language(mut self, language: &str) -> Self {
        let clean: String = language
            .chars()
            .filter(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '#' | '.' | '_' | '-'))
            .take(32)
            .collect();
        self.language = (!clean.is_empty()).then_some(clean);
        self
    }

    /// The header's title, such as a file name or a command, in place of
    /// the language.
    #[must_use]
    pub fn title(mut self, title: impl Into<String>) -> Self {
        let title = title.into();
        self.title = (!title.trim().is_empty()).then_some(title);
        self
    }

    /// Already highlighted markup to show in place of the plain text, for
    /// example spans with Prism-style `token` classes. The caller vouches
    /// that it is escaped. Line numbers, diff and highlighted lines don't
    /// apply to it.
    #[must_use]
    pub fn highlighted(mut self, markup: Markup) -> Self {
        self.highlighted = Some(markup);
        self
    }

    /// Whether to offer the copy action (default: yes).
    #[must_use]
    pub fn copyable(mut self, copyable: bool) -> Self {
        self.copyable = copyable;
        self
    }

    /// An icon for the copy action, drawn before its "Copy" text.
    #[must_use]
    pub fn copy_icon(mut self, icon: impl Render) -> Self {
        self.copy_icon = Some(icon.render());
        self
    }

    /// Wrap long lines instead of scrolling (default: no). With
    /// [`wrap_toggle`](CodeBlock::wrap_toggle), the switch's starting state.
    #[must_use]
    pub fn wrap(mut self, wrap: bool) -> Self {
        self.wrap = wrap;
        self
    }

    /// A Wrap switch in the header (default: no).
    #[must_use]
    pub fn wrap_toggle(mut self, toggle: bool) -> Self {
        self.wrap_toggle = toggle;
        self
    }

    /// Number the lines (default: no).
    #[must_use]
    pub fn line_numbers(mut self, numbers: bool) -> Self {
        self.line_numbers = numbers;
        self
    }

    /// The number of the first line (default 1), for an excerpt.
    #[must_use]
    pub fn first_line(mut self, first: usize) -> Self {
        self.first_line = first.max(1);
        self
    }

    /// Read the code as a unified diff: added lines green, removed lines
    /// red, hunk headers quiet (default: no).
    #[must_use]
    pub fn diff(mut self, diff: bool) -> Self {
        self.diff = diff;
        self
    }

    /// Mark these lines (numbered as shown, from [`first_line`]).
    ///
    /// [`first_line`]: CodeBlock::first_line
    #[must_use]
    pub fn highlight_lines(mut self, lines: impl IntoIterator<Item = usize>) -> Self {
        self.highlights = lines.into_iter().collect();
        self
    }

    /// The size (default [`CodeSize::Md`]).
    #[must_use]
    pub fn size(mut self, size: CodeSize) -> Self {
        self.size = size;
        self
    }

    fn copy_button(&self) -> Markup {
        let mut button = CopyButton::new(self.code.clone())
            .label("Copy")
            .aria_label("Copy code")
            .size(if self.size == CodeSize::Sm {
                ControlSize::Xs
            } else {
                ControlSize::Sm
            })
            .variant(ButtonVariant::Ghost)
            .color(Color::Secondary);
        if let Some(icon) = &self.copy_icon {
            button = button.copy_icon(icon.clone());
        }
        button.render()
    }

    fn wrap_switch(&self) -> Markup {
        html! {
            label.oa-code-block__wrap {
                input.oa-code-block__wrap-input type="checkbox" checked[self.wrap];
                span { "Wrap" }
            }
        }
    }

    fn by_line(&self) -> bool {
        self.highlighted.is_none()
            && (self.line_numbers || self.diff || !self.highlights.is_empty())
    }

    fn lines(&self) -> Markup {
        let text = self.code.strip_suffix('\n').unwrap_or(&self.code);
        let count = text.split('\n').count();
        html! {
            @for (index, line) in text.split('\n').enumerate() {
                @let number = self.first_line + index;
                @let diff = if self.diff { diff_kind(line) } else { None };
                span.oa-code-line data-line=(number) data-diff=[diff]
                    data-highlight[self.highlights.contains(&number)] {
                    (line)
                    @if index + 1 < count { "\n" }
                }
            }
        }
    }
}

impl Render for CodeBlock {
    fn render(&self) -> Markup {
        let language = self.language.as_deref();
        let code_class = language.map(|l| format!("oa-code-block__code language-{l}"));
        let label = self.title.as_deref().or(language);
        let header = (label.is_some() || self.copyable || self.wrap_toggle).then(|| {
            let mut bar = StickyActionBar::new().aria_label("Code actions");
            if let Some(label) = label {
                bar = bar.label(label);
            }
            if self.wrap_toggle {
                bar = bar.action(self.wrap_switch());
            }
            if self.copyable {
                bar = bar.action(self.copy_button());
            }
            bar
        });
        let pane_label = self
            .title
            .clone()
            .unwrap_or_else(|| language.map_or_else(|| "Code".to_owned(), |l| format!("{l} code")));
        let by_line = self.by_line();
        let size = (self.size == CodeSize::Sm).then_some("sm");
        html! {
            div.oa-code-block data-language=[language] data-size=[size]
                data-wrap[self.wrap && !self.wrap_toggle]
                data-numbered[by_line && self.line_numbers]
                data-diff[by_line && self.diff] {
                @if let Some(header) = &header { (header) }
                div.oa-code-block__pane tabindex="0" role="region" aria-label=(pane_label) {
                    pre.oa-code-block__pre {
                        code class=(code_class.as_deref().unwrap_or("oa-code-block__code")) {
                            @if let Some(markup) = &self.highlighted {
                                (markup)
                            } @else if by_line {
                                (self.lines())
                            } @else {
                                (self.code)
                            }
                        }
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use maud::PreEscaped;

    #[test]
    fn code_is_escaped_and_scrolls_in_a_pane() {
        let html = CodeBlock::new("fn main() { println!(\"<hi>\"); }")
            .language("rust\" onclick=\"x")
            .render()
            .into_string();
        assert!(html.contains("&lt;hi&gt;"), "{html}");
        assert!(!html.contains("onclick="), "{html}");
        assert!(html.contains("data-language=\"rustonclickx\""), "{html}");
        assert!(
            html.contains("class=\"oa-code-block__code language-rustonclickx\""),
            "{html}"
        );
        assert!(html.contains("class=\"oa-code-block__pane\" tabindex=\"0\""));
        assert!(html.contains("role=\"toolbar\" aria-label=\"Code actions\""));
        // The copy action is a CopyButton carrying the (unescaped) code.
        assert!(
            html.contains("data-oa-copy=\"fn main() { println!(&quot;&lt;hi&gt;&quot;); }\""),
            "{html}"
        );
        assert!(html.contains("<span>Copy</span>"), "{html}");
        assert!(html.contains("aria-label=\"Copy code\""), "{html}");
        assert!(!html.contains("data-wrap"), "{html}");
    }

    #[test]
    fn options() {
        let html = CodeBlock::new("x")
            .copyable(false)
            .wrap(true)
            .render()
            .into_string();
        assert!(!html.contains("oa-sticky-action-bar"), "{html}");
        assert!(html.contains("data-wrap"), "{html}");
        assert!(html.contains("class=\"oa-code-block__code\""), "{html}");

        let html = CodeBlock::new("ignored")
            .highlighted(PreEscaped(
                "<span class=\"token keyword\">fn</span>".to_owned(),
            ))
            .copy_icon(PreEscaped("<svg></svg>"))
            .render()
            .into_string();
        assert!(html.contains("<span class=\"token keyword\">fn</span>"));
        assert!(!html.contains(">ignored<"));
        assert!(
            html.contains("data-copy-icon=\"copy\"><svg></svg></span>"),
            "{html}"
        );
        assert!(html.contains("data-oa-copy=\"ignored\""), "{html}");
    }

    #[test]
    fn bar_without_actions_is_not_a_toolbar() {
        let html = StickyActionBar::new()
            .label("<b>")
            .aria_label("x")
            .render()
            .into_string();
        assert_eq!(
            html,
            "<div class=\"oa-sticky-action-bar\"><span class=\"oa-sticky-action-bar__label\">&lt;b&gt;</span></div>"
        );
    }

    #[test]
    fn lines_numbers_diff_and_highlights() {
        let html = CodeBlock::new("@@ -1,2 +1,2 @@\n-old <a>\n+new\n same\n")
            .title("src/main.rs")
            .language("diff")
            .diff(true)
            .line_numbers(true)
            .first_line(10)
            .highlight_lines([12])
            .size(CodeSize::Sm)
            .wrap_toggle(true)
            .render()
            .into_string();
        assert!(html.contains("data-size=\"sm\""), "{html}");
        assert!(html.contains("data-numbered"), "{html}");
        assert!(
            html.contains("data-line=\"10\" data-diff=\"hunk\""),
            "{html}"
        );
        assert!(
            html.contains("data-line=\"11\" data-diff=\"del\">-old &lt;a&gt;\n</span>"),
            "{html}"
        );
        assert!(
            html.contains("data-line=\"12\" data-diff=\"add\" data-highlight>+new\n</span>"),
            "{html}"
        );
        // The last line has no newline after it, and there is no empty line.
        assert!(html.contains("data-line=\"13\"> same</span>"), "{html}");
        assert!(!html.contains("data-line=\"14\""), "{html}");
        assert!(
            html.contains("oa-sticky-action-bar__label\">src/main.rs<"),
            "{html}"
        );
        assert!(html.contains("aria-label=\"src/main.rs\""), "{html}");
        assert!(
            html.contains("class=\"oa-code-block__wrap-input\" type=\"checkbox\""),
            "{html}"
        );
        // The switch, not the attribute, decides wrapping.
        assert!(!html.contains(" data-wrap"), "{html}");
    }
}
