use rust_native::markdown::{Align, Block, Span};
use rust_native::style::{Color, Space, Style, TextAlign, TextWeight};
use rust_native::{
    Axis, Element, Glyph, MessageRole, Node, RichRun, TextRole, ValidatedView, View, ViewError,
};
use serde::Serialize;
use std::fmt::{self, Write};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RenderError {
    View(ViewError),
    MissingSource(String),
    OutputLimit,
}
impl fmt::Display for RenderError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::View(error) => error.fmt(f),
            Self::MissingSource(_) => {
                f.write_str("transcript source is unavailable in this process")
            }
            Self::OutputLimit => f.write_str("rendered HTML exceeds its byte bound"),
        }
    }
}
impl std::error::Error for RenderError {}
impl From<ViewError> for RenderError {
    fn from(error: ViewError) -> Self {
        Self::View(error)
    }
}

/// Render one validated surface. Every text and attribute is escaped. Events
/// identify nodes; serialized application intents are never placed in HTML.
pub fn render<I>(view: &ValidatedView<I>) -> Result<String, RenderError> {
    let view = view.view();
    let mut out = format!(
        "<div class=\"rn-view\" data-rn-instance=\"{}\" data-rn-revision=\"{}\" data-rn-schema=\"{}\">",
        escape(&view.instance),
        view.revision,
        escape(&view.schema)
    );
    node(&view.root, &mut out)?;
    out.push_str("</div>");
    Ok(out)
}

pub fn render_view<I: Serialize + Clone>(view: &View<I>) -> Result<String, RenderError> {
    render(&view.clone().validate()?)
}

fn escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}
fn color(value: Color) -> String {
    format!(
        "rgba({},{},{},{:.4})",
        value.red,
        value.green,
        value.blue,
        f32::from(value.alpha) / 255.0
    )
}
fn space(value: Space) -> u16 {
    match value {
        Space::None => 0,
        Space::Xs => 4,
        Space::Sm => 8,
        Space::Md => 16,
        Space::Lg => 24,
    }
}
fn styles(style: &Style) -> String {
    let mut out = String::new();
    macro_rules! prop {
        ($name:literal,$value:expr) => {
            let _ = write!(out, "{}:{};", $name, $value);
        };
    }
    if let Some(value) = style.foreground {
        prop!("color", color(value));
    }
    if let Some(value) = style.background {
        prop!("background-color", color(value));
    }
    for (name, value) in [
        ("padding-block-start", style.padding_top),
        ("padding-inline-end", style.padding_end),
        ("padding-block-end", style.padding_bottom),
        ("padding-inline-start", style.padding_start),
    ] {
        if let Some(value) = value {
            let _ = write!(out, "{name}:{}px;", space(value));
        }
    }
    if let Some([top, end, bottom, start]) = style.padding_points {
        prop!("padding", format!("{top}px {end}px {bottom}px {start}px"));
    }
    if let Some(value) = style.gap {
        prop!("gap", format!("{}px", space(value)));
    }
    if let Some(value) = style.gap_points {
        prop!("gap", format!("{value}px"));
    }
    if let Some(value) = style.weight {
        prop!(
            "font-weight",
            match value {
                TextWeight::Normal => 400,
                TextWeight::Medium => 500,
                TextWeight::Semibold => 600,
                TextWeight::Bold => 700,
            }
        );
    }
    if let Some(value) = style.align {
        prop!(
            "text-align",
            match value {
                TextAlign::Start => "start",
                TextAlign::Center => "center",
                TextAlign::End => "end",
            }
        );
    }
    if let Some(value) = style.radius {
        prop!("border-radius", format!("{value}px"));
    }
    if let Some(value) = style.border {
        prop!("border", format!("1px solid {}", color(value)));
    }
    if let Some(value) = style.fill_height {
        prop!("flex", if value { "1 1 0" } else { "0 0 auto" });
    }
    if let Some(value) = style.intrinsic_width {
        if value {
            prop!("flex", "0 0 auto");
            prop!("width", "max-content");
        }
    }
    if let Some(value) = style.text_size {
        prop!("font-size", format!("{value}px"));
    }
    if let Some(value) = style.line_height {
        prop!("line-height", format!("{value}px"));
    }
    if let Some(value) = style.min_height {
        prop!("min-height", format!("{value}px"));
    }
    if let Some([x, y]) = style.button_padding {
        prop!("padding", format!("{y}px {x}px"));
    }
    if let Some(value) = style.hover_foreground {
        prop!("--rn-hover-foreground", color(value));
    }
    if let Some(value) = style.hover_background {
        prop!("--rn-hover-background", color(value));
    }
    if let Some(value) = style.glyph_size {
        prop!("--rn-glyph-size", format!("{value}px"));
    }
    if let Some(value) = style.glyph_gap {
        prop!("--rn-glyph-gap", format!("{value}px"));
    }
    if let Some(value) = style.glyph_color {
        prop!("--rn-glyph-color", color(value));
    }
    if let Some(value) = style.monospace {
        prop!(
            "font-family",
            if value {
                "var(--rn-mono-font,monospace)"
            } else {
                "var(--rn-default-font)"
            }
        );
    }
    if let Some(value) = style.viewport {
        prop!("max-height", format!("{}px", value.max_height));
        prop!("overflow-y", "auto");
    }
    out
}
fn start<I>(tag: &str, class: &str, node: &Node<I>, extra: &str, out: &mut String) {
    let _ = write!(
        out,
        "<{tag} class=\"rn-node {class}\" data-rn-node=\"{}\" style=\"{}\"{extra}>",
        escape(&node.key),
        styles(&node.style)
    );
}
fn children<I>(rows: &[Node<I>], out: &mut String) -> Result<(), RenderError> {
    for row in rows {
        node(row, out)?;
    }
    Ok(())
}
fn node<I>(n: &Node<I>, out: &mut String) -> Result<(), RenderError> {
    if out.len() > 16 * 1024 * 1024 {
        return Err(RenderError::OutputLimit);
    }
    match &n.element {
        Element::Stack { axis, children: c } => {
            start(
                "div",
                match axis {
                    Axis::Vertical => "rn-stack",
                    Axis::Horizontal => "rn-stack rn-stack-horizontal",
                    Axis::Wrap => "rn-stack rn-stack-wrap",
                },
                n,
                "",
                out,
            );
            children(c, out)?;
            out.push_str("</div>");
        }
        Element::List { label, children: c } => {
            start(
                "div",
                "rn-list",
                n,
                &format!(" role=\"list\" aria-label=\"{}\"", escape(label)),
                out,
            );
            for child in c {
                out.push_str("<div role=\"listitem\">");
                node(child, out)?;
                out.push_str("</div>");
            }
            out.push_str("</div>");
        }
        Element::Text { value, role } => {
            start(
                "div",
                match role {
                    TextRole::Heading => "rn-text rn-heading",
                    TextRole::Code => "rn-text rn-code",
                    TextRole::Terminal => "rn-text rn-terminal",
                    TextRole::Status => "rn-text rn-status",
                    _ => "rn-text",
                },
                n,
                if *role == TextRole::Heading {
                    " role=\"heading\" aria-level=\"2\""
                } else {
                    ""
                },
                out,
            );
            out.push_str(&escape(value));
            out.push_str("</div>");
        }
        Element::RichText { runs, role } => {
            start(
                "div",
                if *role == TextRole::Terminal {
                    "rn-rich rn-terminal"
                } else {
                    "rn-rich"
                },
                n,
                "",
                out,
            );
            for run in runs {
                rich(run, out);
            }
            out.push_str("</div>");
        }
        Element::Field {
            label,
            value,
            placeholder,
            secret,
            multiline,
            enabled,
            max_bytes,
            ..
        } => {
            let mut outer = n.style;
            outer.border = None;
            outer.radius = None;
            let mut field_style = styles(&outer);
            if let Some(border) = n.style.border {
                let _ = write!(field_style, "--rn-field-border:{};", color(border));
            }
            if let Some(radius) = n.style.radius {
                let _ = write!(field_style, "--rn-field-radius:{radius}px;");
            }
            let _ = write!(
                out,
                "<label class=\"rn-node rn-field\" data-rn-node=\"{}\" style=\"{field_style}\">",
                escape(&n.key)
            );
            let _ = write!(out, "<span>{}</span>", escape(label));
            let attrs = format!(
                " class=\"rn-input\" name=\"{}\" data-rn-action=\"change\" data-rn-max-bytes=\"{max_bytes}\" data-rn-secret=\"{secret}\" aria-label=\"{}\" placeholder=\"{}\"{}",
                escape(&n.key),
                escape(label),
                escape(placeholder),
                if *enabled { "" } else { " disabled" }
            );
            if *multiline {
                let rows = n.style.min_height.map_or(3, |height| {
                    height
                        .div_ceil(n.style.line_height.unwrap_or(20))
                        .clamp(1, 40)
                });
                let _ = write!(
                    out,
                    "<textarea{attrs} rows=\"{rows}\">{}</textarea>",
                    escape(value)
                );
            } else {
                let _ = write!(
                    out,
                    "<input{attrs} type=\"{}\" value=\"{}\"{}>",
                    if *secret { "password" } else { "text" },
                    if *secret {
                        String::new()
                    } else {
                        escape(value)
                    },
                    if *secret {
                        " autocomplete=\"off\" spellcheck=\"false\""
                    } else {
                        ""
                    }
                );
            }
            out.push_str("</label>");
        }
        Element::Button {
            label,
            enabled,
            icon,
            shortcut,
            ..
        } => {
            let mut extra = format!(
                " type=\"button\" data-rn-action=\"activate\"{}",
                if *enabled { "" } else { " disabled" }
            );
            let mut class = "rn-button".to_owned();
            if let Some(icon) = icon {
                if icon.circular {
                    class.push_str(" rn-circular");
                    let _ = write!(extra, " aria-label=\"{}\"", escape(label));
                } else if icon.pill {
                    class.push_str(" rn-pill");
                }
                if matches!(icon.glyph, Glyph::Checked | Glyph::Unchecked) {
                    let _ = write!(
                        extra,
                        " role=\"checkbox\" aria-checked=\"{}\"",
                        icon.glyph == Glyph::Checked
                    );
                }
            }
            start("button", &class, n, &extra, out);
            if let Some(avatar) = n.style.button_avatar {
                let _ = write!(
                    out,
                    "<span class=\"rn-avatar\" aria-hidden=\"true\" style=\"width:{}px;height:{}px;font-size:{}px;background:{};color:{}\">{}</span>",
                    avatar.size,
                    avatar.size,
                    avatar.text_size,
                    color(avatar.background),
                    color(avatar.foreground),
                    escape(&avatar.initial.to_string())
                );
            }
            if let Some(icon) = icon {
                let _ = write!(
                    out,
                    "<span class=\"rn-icon\" aria-hidden=\"true\">{}</span>",
                    glyph(icon.glyph)
                );
            }
            if !icon.is_some_and(|icon| icon.circular) {
                if let Some(detail) = n.style.button_detail {
                    if let Some((first, rest)) = label.split_once('\n') {
                        out.push_str(&escape(first));
                        let _ = write!(
                            out,
                            "<span style=\"display:block;font-size:{}px;line-height:{}px;color:{};text-align:{}\">{}</span>",
                            detail.text_size,
                            detail.line_height,
                            color(detail.color),
                            if detail.leading { "start" } else { "inherit" },
                            escape(rest)
                        );
                    } else {
                        out.push_str(&escape(label));
                    }
                } else {
                    out.push_str(&escape(label));
                }
            }
            if let Some(shortcut) = shortcut {
                let _ = write!(
                    out,
                    "<span class=\"rn-shortcut\">{}</span>",
                    escape(shortcut)
                );
            }
            out.push_str("</button>");
        }
        Element::Choice {
            label,
            selected,
            enabled,
            children: c,
            ..
        } => {
            start(
                "button",
                "rn-choice",
                n,
                &format!(
                    " type=\"button\" data-rn-action=\"activate\" aria-label=\"{}\" aria-pressed=\"{selected}\"{}",
                    escape(label),
                    if *enabled { "" } else { " disabled" }
                ),
                out,
            );
            if c.is_empty() {
                out.push_str(&escape(label));
            } else {
                children(c, out)?;
            }
            out.push_str("</button>");
        }
        Element::Dialog {
            label,
            open,
            children: c,
            ..
        } => {
            start(
                "dialog",
                "rn-dialog",
                n,
                &format!(
                    " role=\"dialog\" aria-label=\"{}\" data-rn-dialog=\"{open}\"{}",
                    escape(label),
                    if *open { " open" } else { "" }
                ),
                out,
            );
            out.push_str("<button class=\"rn-button rn-dialog-close\" type=\"button\" data-rn-action=\"dismiss\" aria-label=\"Close dialog\">×</button><div class=\"rn-dialog-body\">");
            children(c, out)?;
            out.push_str("</div></dialog>");
        }
        Element::Transcript {
            label,
            children: c,
            earlier,
            source,
        } => {
            start(
                "div",
                "rn-transcript",
                n,
                &format!(
                    " role=\"log\" aria-label=\"{}\" aria-live=\"off\"",
                    escape(label)
                ),
                out,
            );
            if let Some(earlier) = earlier {
                let _ = write!(
                    out,
                    "<button type=\"button\" class=\"rn-button\" data-rn-action=\"activate\"{}>{}</button>",
                    if earlier.loading { " disabled" } else { "" },
                    escape(&earlier.label)
                );
            }
            if let Some(source) = source {
                let snapshot = rust_native::layout::source::get(source)
                    .ok_or_else(|| RenderError::MissingSource(source.clone()))?;
                for row in snapshot.rows() {
                    node(row, out)?;
                }
            } else {
                children(c, out)?;
            }
            out.push_str("</div>");
        }
        Element::Message {
            role,
            note,
            children: c,
        } => {
            start(
                "div",
                match role {
                    MessageRole::User => "rn-message rn-message-user",
                    MessageRole::Assistant => "rn-message rn-message-assistant",
                    MessageRole::System => "rn-message rn-message-system",
                },
                n,
                "",
                out,
            );
            children(c, out)?;
            if let Some(note) = note {
                let _ = write!(out, "<div class=\"rn-note\">{}</div>", escape(note));
            }
            out.push_str("</div>");
        }
        Element::Markdown { blocks } => {
            start("div", "rn-markdown", n, "", out);
            markdown(blocks, out);
            out.push_str("</div>");
        }
        Element::Tool {
            name,
            detail,
            state,
            children: c,
        } => {
            start("details", "rn-tool", n, "", out);
            let _ = write!(
                out,
                "<summary>{} {} <span class=\"rn-status\">{state:?}</span></summary>",
                escape(name),
                escape(detail)
            );
            children(c, out)?;
            out.push_str("</details>");
        }
        Element::Working { label } => {
            start("div", "rn-working", n, " role=\"status\"", out);
            out.push_str(&escape(label));
            out.push_str("</div>");
        }
        Element::Composer {
            token,
            placeholder,
            max_bytes,
            enabled,
            busy,
            stop,
            choices,
            draft,
            focus,
        } => {
            start("div", "rn-composer", n, "", out);
            let _ = write!(
                out,
                "<textarea class=\"rn-input\" data-rn-action=\"compose\" data-rn-token=\"{}\" data-rn-max-bytes=\"{max_bytes}\" placeholder=\"{}\" aria-label=\"{}\"{}{}>{}</textarea>",
                escape(token),
                escape(placeholder),
                escape(placeholder),
                if *enabled { "" } else { " disabled" },
                if *focus {
                    " data-rn-focus=\"true\""
                } else {
                    ""
                },
                escape(draft.as_deref().unwrap_or_default())
            );
            let _ = write!(
                out,
                "<button class=\"rn-button\" type=\"button\" data-rn-action=\"{}\" data-rn-token=\"{}\"{}>{}</button>",
                if *busy { "activate" } else { "submit" },
                escape(token),
                if !enabled || (*busy && stop.is_none()) {
                    " disabled"
                } else {
                    ""
                },
                if *busy { "Stop" } else { "Send" }
            );
            for choice in choices {
                let _ = write!(
                    out,
                    "<button class=\"rn-button\" type=\"button\" data-rn-action=\"submit\" data-rn-token=\"{}\"{}>{}</button>",
                    escape(&choice.token),
                    if *enabled && !busy { "" } else { " disabled" },
                    escape(&choice.label)
                );
            }
            out.push_str("</div>");
        }
        Element::Surface { resource, label } => {
            start(
                "div",
                "rn-surface",
                n,
                &format!(
                    " role=\"img\" aria-label=\"{}\" data-rn-surface=\"{}\"",
                    escape(label),
                    escape(resource)
                ),
                out,
            );
            out.push_str(&escape(label));
            out.push_str("</div>");
        }
    }
    Ok(())
}
fn rich(run: &RichRun, out: &mut String) {
    let mut style = String::new();
    if let Some(c) = run.foreground {
        let _ = write!(style, "color:{};", color(c));
    }
    if let Some(c) = run.background {
        let _ = write!(style, "background-color:{};", color(c));
    }
    if run.bold {
        style.push_str("font-weight:700;");
    }
    if run.italic {
        style.push_str("font-style:italic;");
    }
    if run.dim {
        style.push_str("opacity:.5;");
    }
    if run.underline || run.strike {
        let _ = write!(
            style,
            "text-decoration:{} {};",
            if run.underline { "underline" } else { "" },
            if run.strike { "line-through" } else { "" }
        );
    }
    let _ = write!(out, "<span style=\"{style}\">{}</span>", escape(&run.text));
}
fn spans(values: &[Span], out: &mut String) {
    for span in values {
        let class = if span.link.is_some() {
            " class=\"rn-inert-link\""
        } else {
            ""
        };
        let _ = write!(out, "<span{class}>");
        if span.bold {
            out.push_str("<strong>");
        }
        if span.italic {
            out.push_str("<em>");
        }
        if span.strike {
            out.push_str("<s>");
        }
        if span.code {
            out.push_str("<code>");
        }
        out.push_str(&escape(&span.text));
        if span.code {
            out.push_str("</code>");
        }
        if span.strike {
            out.push_str("</s>");
        }
        if span.italic {
            out.push_str("</em>");
        }
        if span.bold {
            out.push_str("</strong>");
        }
        out.push_str("</span>");
    }
}
fn markdown(blocks: &[Block], out: &mut String) {
    for block in blocks {
        match block {
            Block::Heading { level, spans: s } => {
                let level = (*level).clamp(1, 6);
                let _ = write!(out, "<h{level}>");
                spans(s, out);
                let _ = write!(out, "</h{level}>");
            }
            Block::Paragraph { spans: s } => {
                out.push_str("<p>");
                spans(s, out);
                out.push_str("</p>");
            }
            Block::Code { language, text } => {
                let _ = write!(
                    out,
                    "<pre><code data-rn-language=\"{}\">{}</code></pre>",
                    escape(language.as_deref().unwrap_or_default()),
                    escape(text)
                );
            }
            Block::Quote { blocks } => {
                out.push_str("<blockquote>");
                markdown(blocks, out);
                out.push_str("</blockquote>");
            }
            Block::Rule => out.push_str("<hr>"),
            Block::List {
                ordered,
                start,
                items,
            } => {
                let tag = if *ordered { "ol" } else { "ul" };
                let _ = write!(out, "<{tag} start=\"{start}\">");
                for item in items {
                    out.push_str("<li>");
                    if let Some(checked) = item.checked {
                        let _ = write!(
                            out,
                            "<input type=\"checkbox\" disabled aria-label=\"Task state\"{}>",
                            if checked { " checked" } else { "" }
                        );
                    }
                    markdown(&item.blocks, out);
                    out.push_str("</li>");
                }
                let _ = write!(out, "</{tag}>");
            }
            Block::Table {
                align,
                header,
                rows,
            } => {
                out.push_str("<div class=\"rn-table-window\"><table><thead><tr>");
                for (index, cell) in header.iter().enumerate() {
                    cell_html("th", cell, align.get(index), out);
                }
                out.push_str("</tr></thead><tbody>");
                for row in rows {
                    out.push_str("<tr>");
                    for (index, cell) in row.iter().enumerate() {
                        cell_html("td", cell, align.get(index), out);
                    }
                    out.push_str("</tr>");
                }
                out.push_str("</tbody></table></div>");
            }
        }
    }
}
fn cell_html(tag: &str, cell: &[Span], align: Option<&Align>, out: &mut String) {
    let align = match align {
        Some(Align::Center) => "center",
        Some(Align::Right) => "right",
        _ => "left",
    };
    let _ = write!(out, "<{tag} style=\"text-align:{align}\">");
    spans(cell, out);
    let _ = write!(out, "</{tag}>");
}
fn glyph(glyph: Glyph) -> &'static str {
    match glyph {
        Glyph::Back => "←",
        Glyph::Forward => "→",
        Glyph::Compose | Glyph::Edit => "✎",
        Glyph::Menu => "☰",
        Glyph::History => "↶",
        Glyph::Folder => "▱",
        Glyph::Computer => "▣",
        Glyph::Cloud => "☁",
        Glyph::Add | Glyph::Plus => "+",
        Glyph::ArrowUp => "↑",
        Glyph::ArrowDown => "↓",
        Glyph::Stop => "■",
        Glyph::Paperclip => "⌁",
        Glyph::Clipboard => "▤",
        Glyph::More => "⋯",
        Glyph::Search => "⌕",
        Glyph::Settings => "⚙",
        Glyph::Pin => "◆",
        Glyph::Archive => "▾",
        Glyph::Restore => "↥",
        Glyph::Check | Glyph::Checked => "☑",
        Glyph::Unchecked => "☐",
        Glyph::Ask => "?",
        Glyph::Flag => "⚑",
        Glyph::Terminal => ">_",
        Glyph::Wallet => "▰",
        Glyph::Key => "⚿",
        Glyph::Person => "♙",
        Glyph::Map => "⌘",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn view(element: Element<&'static str>) -> ValidatedView<&'static str> {
        View::new_v3(
            "preview",
            1,
            Node {
                key: "root".into(),
                style: Style::default(),
                element,
            },
        )
        .validate()
        .unwrap()
    }
    #[test]
    fn text_and_attributes_are_literal_and_intents_are_absent() {
        let html = render(&view(Element::Button {
            label: "<script>\"&'".into(),
            enabled: true,
            icon: None,
            shortcut: None,
            intent: "PRIVATE_INTENT",
        }))
        .unwrap();
        assert!(html.contains("&lt;script&gt;&quot;&amp;&#39;"));
        assert!(!html.contains("<script>"));
        assert!(!html.contains("PRIVATE_INTENT"));
        assert!(html.contains("data-rn-action=\"activate\""));
    }
    #[test]
    fn rich_runs_preserve_literal_text_and_generic_styles() {
        let html = render(&view(Element::RichText {
            role: TextRole::Terminal,
            runs: vec![RichRun {
                text: "<x>  \n".into(),
                foreground: Some(Color::rgb(1, 2, 3)),
                bold: true,
                ..RichRun::default()
            }],
        }))
        .unwrap();
        assert!(html.contains("rgba(1,2,3,1.0000)"));
        assert!(html.contains("font-weight:700"));
        assert!(html.contains("&lt;x&gt;  \n"));
    }
    #[test]
    fn secret_fields_do_not_place_values_or_intents_in_markup() {
        let html = render(&view(Element::Field {
            label: "API key".into(),
            value: String::new(),
            placeholder: "Key".into(),
            secret: true,
            multiline: false,
            enabled: true,
            max_bytes: 100,
            on_change: "KEY_CHANGE",
        }))
        .unwrap();
        assert!(html.contains("type=\"password\""));
        assert!(html.contains("name=\"root\""));
        assert!(html.contains("autocomplete=\"off\""));
        assert!(!html.contains("KEY_CHANGE"));
    }
    #[test]
    fn multiline_field_rows_follow_declared_geometry_without_an_extra_frame() {
        for (height, expected) in [(20, 1), (90, 5)] {
            let checked = View::new_v3(
                "preview",
                1,
                Node {
                    key: "draft".into(),
                    style: Style {
                        min_height: Some(height),
                        line_height: Some(20),
                        border: Some(Color::rgb(4, 5, 6)),
                        ..Style::default()
                    },
                    element: Element::Field {
                        label: "Draft".into(),
                        value: String::new(),
                        placeholder: String::new(),
                        secret: false,
                        multiline: true,
                        enabled: true,
                        max_bytes: 100,
                        on_change: "draft",
                    },
                },
            )
            .validate()
            .unwrap();
            let html = render(&checked).unwrap();
            assert!(html.contains(&format!("rows=\"{expected}\"")));
            assert!(html.contains("--rn-field-border:"));
            assert!(!html.contains("border:1px solid"));
        }
    }
    #[test]
    fn markdown_never_loads_links_or_executes_html() {
        let html = render(&view(Element::Markdown {
            blocks: rust_native::markdown::parse(
                "[unsafe](javascript:alert(1))\n\n<script>alert(1)</script>",
            ),
        }))
        .unwrap();
        assert!(!html.contains("href="));
        assert!(!html.contains("<script>"));
        assert!(html.contains("&lt;script&gt;"));
    }
}
