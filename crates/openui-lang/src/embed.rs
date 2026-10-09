//! OpenUI Lang blocks inside Markdown, and the Markdown a surface shows
//! when it cannot draw the components.

use crate::tree::Node;
use crate::{LANG, Stream};

/// Where site paths point in the Markdown fallback.
pub const SITE: &str = "https://openagents.com";

/// A piece of a reply: Markdown, or a component block's source.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Segment<'a> {
    Markdown(&'a str),
    /// The statements between the fences; `closed` is false while the
    /// block is still streaming in.
    Ui {
        source: &'a str,
        closed: bool,
    },
}

/// Whether `line` opens a component block: ```` ```openui-lang ````, with
/// up to three spaces before it.
fn opens(line: &str) -> bool {
    let trimmed = line.trim_end_matches(['\n', '\r']);
    let indent = trimmed.len() - trimmed.trim_start_matches(' ').len();
    indent <= 3
        && trimmed
            .trim_start()
            .strip_prefix("```")
            .is_some_and(|info| info.trim() == LANG)
}

fn fence_of(line: &str) -> Option<(char, usize)> {
    let content = line.trim_start_matches(' ');
    let mark = content.chars().next().filter(|c| *c == '`' || *c == '~')?;
    let run = content.chars().take_while(|c| *c == mark).count();
    (run >= 3).then_some((mark, run))
}

/// Splits `text` into Markdown and component blocks. A block opens on a
/// line ```` ```openui-lang ```` outside any other code block and closes on
/// a line of three or more backticks.
#[must_use]
pub fn segments(text: &str) -> Vec<Segment<'_>> {
    let mut out = Vec::new();
    let mut start = 0;
    let mut at = 0;
    let mut other: Option<(char, usize)> = None;
    let mut ui: Option<usize> = None;
    for line in text.split_inclusive('\n') {
        let end = at + line.len();
        match (ui, other) {
            (Some(body), _) => {
                let content = line.trim();
                if content.len() >= 3 && content.chars().all(|c| c == '`') {
                    out.push(Segment::Ui {
                        source: &text[body..at],
                        closed: true,
                    });
                    ui = None;
                    start = end;
                }
            }
            (None, Some((mark, run))) => {
                let content = line.trim();
                if content.len() >= run && content.chars().all(|c| c == mark) {
                    other = None;
                }
            }
            (None, None) => {
                if opens(line) {
                    if start < at {
                        out.push(Segment::Markdown(&text[start..at]));
                    }
                    ui = Some(end);
                } else if let Some(fence) = fence_of(line) {
                    other = Some(fence);
                }
            }
        }
        at = end;
    }
    match ui {
        Some(body) => out.push(Segment::Ui {
            source: &text[body.min(text.len())..],
            closed: false,
        }),
        None if start < text.len() => out.push(Segment::Markdown(&text[start..])),
        None => {}
    }
    out
}

/// Whether `text` has a component block.
#[must_use]
pub fn has_ui(text: &str) -> bool {
    segments(text)
        .iter()
        .any(|segment| matches!(segment, Segment::Ui { .. }))
}

/// The trees of `text`'s component blocks, in order; an open block is read
/// with the streaming rules.
#[must_use]
pub fn trees(text: &str) -> Vec<Node> {
    segments(text)
        .into_iter()
        .filter_map(|segment| match segment {
            Segment::Ui { source, closed } => {
                let mut stream = Stream::default();
                if closed {
                    stream.finish(source).root
                } else {
                    stream.update(source).root
                }
            }
            Segment::Markdown(_) => None,
        })
        .collect()
}

/// `text` with each component block replaced by its Markdown fallback
/// ([`markdown`]): what a surface that cannot draw components shows.
#[must_use]
pub fn fallback(text: &str) -> String {
    let mut out = String::new();
    for segment in segments(text) {
        match segment {
            Segment::Markdown(markdown) => out.push_str(markdown),
            Segment::Ui { source, closed } => {
                let mut stream = Stream::default();
                let document = if closed {
                    stream.finish(source)
                } else {
                    stream.update(source)
                };
                if let Some(root) = document.root {
                    if !out.is_empty() && !out.ends_with("\n\n") {
                        out.push_str(if out.ends_with('\n') { "\n" } else { "\n\n" });
                    }
                    out.push_str(&markdown(&root));
                    out.push('\n');
                }
            }
        }
    }
    out
}

/// `text` without its component blocks: the prose alone.
#[must_use]
pub fn prose(text: &str) -> String {
    segments(text)
        .into_iter()
        .filter_map(|segment| match segment {
            Segment::Markdown(markdown) => Some(markdown),
            Segment::Ui { .. } => None,
        })
        .collect::<String>()
        .trim_end()
        .to_owned()
}

/// An absolute URL for `href`: a site path joined to [`SITE`].
#[must_use]
pub fn absolute(href: &str) -> String {
    if href.starts_with('/') {
        format!("{SITE}{href}")
    } else {
        href.to_owned()
    }
}

/// Every link target in `node`, absolute, in order.
#[must_use]
pub fn links(node: &Node) -> Vec<String> {
    let mut out = Vec::new();
    walk(node, &mut |node| match node {
        Node::Link { href, .. } | Node::Button { href, .. } | Node::LinkCard { href, .. } => {
            out.push(absolute(href));
        }
        _ => {}
    });
    out
}

/// Every command and code block in `node`, in order.
#[must_use]
pub fn commands(node: &Node) -> Vec<String> {
    let mut out = Vec::new();
    walk(node, &mut |node| match node {
        Node::CodeBlock { code, .. } => out.push(code.clone()),
        Node::Command { unix, windows } => {
            out.push(unix.clone());
            out.extend(windows.clone());
        }
        _ => {}
    });
    out
}

/// Calls `visit` on `node` and everything inside it.
pub fn walk(node: &Node, visit: &mut impl FnMut(&Node)) {
    visit(node);
    match node {
        Node::Stack { children } | Node::Columns { children } | Node::Card { children, .. } => {
            for child in children {
                walk(child, visit);
            }
        }
        Node::Steps { steps } => {
            for child in steps.iter().flat_map(|s| &s.children) {
                walk(child, visit);
            }
        }
        Node::Tabs { tabs } => {
            for child in tabs.iter().flat_map(|t| &t.children) {
                walk(child, visit);
            }
        }
        _ => {}
    }
}

/// The label over a command's macOS and Linux tab.
pub const UNIX_LABEL: &str = "macOS and Linux";
/// The label over a command's Windows tab.
pub const WINDOWS_LABEL: &str = "Windows";

/// `node` as Markdown: titles in bold, links as links (site paths made
/// absolute), steps numbered, and each command a code block under its
/// system's name. Buttons for signed-out readers are left out.
#[must_use]
pub fn markdown(node: &Node) -> String {
    let mut blocks = Vec::new();
    write(node, &mut blocks);
    blocks.join("\n\n")
}

fn link(label: &str, href: &str) -> String {
    format!("[{}]({})", escape(label), absolute(href))
}

fn escape(text: &str) -> String {
    text.replace('[', "\\[").replace(']', "\\]")
}

fn fenced(code: &str, language: &str) -> String {
    let longest = code.split(|c| c != '`').map(str::len).max().unwrap_or(0);
    let fence = "`".repeat((longest + 1).max(3));
    format!("{fence}{language}\n{code}\n{fence}")
}

fn write(node: &Node, out: &mut Vec<String>) {
    match node {
        Node::Stack { children } | Node::Columns { children } => {
            for child in children {
                write(child, out);
            }
        }
        Node::Card { title, children } => {
            out.push(format!("**{title}**"));
            for child in children {
                write(child, out);
            }
        }
        Node::Text { text } => out.push(text.clone()),
        Node::Link { label, href } => out.push(link(label, href)),
        Node::Button {
            label, href, show, ..
        } => {
            if show.shows(None) {
                out.push(link(label, href));
            }
        }
        Node::CodeBlock { code, language } => {
            out.push(fenced(code, language.as_deref().unwrap_or("")));
        }
        Node::Command { unix, windows } => match windows {
            Some(windows) => {
                out.push(format!("{UNIX_LABEL}:"));
                out.push(fenced(unix, "bash"));
                out.push(format!("{WINDOWS_LABEL} (PowerShell):"));
                out.push(fenced(windows, "powershell"));
            }
            None => out.push(fenced(unix, "bash")),
        },
        Node::Steps { steps } => {
            let mut list = String::new();
            for (n, step) in steps.iter().enumerate() {
                if n > 0 {
                    list.push_str("\n\n");
                }
                let marker = format!("{}. ", n + 1);
                let pad = " ".repeat(marker.len());
                list.push_str(&marker);
                list.push_str(&step.title);
                let mut inner = Vec::new();
                for child in &step.children {
                    write(child, &mut inner);
                }
                for block in inner {
                    list.push_str("\n\n");
                    let indented: Vec<String> = block
                        .lines()
                        .map(|line| {
                            if line.is_empty() {
                                String::new()
                            } else {
                                format!("{pad}{line}")
                            }
                        })
                        .collect();
                    list.push_str(&indented.join("\n"));
                }
            }
            out.push(list);
        }
        Node::Tabs { tabs } => {
            for tab in tabs {
                out.push(format!("**{}**", tab.label));
                for child in &tab.children {
                    write(child, out);
                }
            }
        }
        Node::LinkCard {
            title,
            description,
            href,
        } => out.push(format!("{}: {description}", link(title, href))),
    }
}

/// The text a reader of `node` sees, links written out after their labels,
/// one block per line: for checks that read an answer's words.
#[must_use]
pub fn plain(node: &Node) -> String {
    let mut out = Vec::new();
    walk(node, &mut |node| match node {
        Node::Card { title, .. } => out.push(title.clone()),
        Node::Text { text } => out.push(text.clone()),
        Node::Link { label, href } | Node::Button { label, href, .. } => {
            out.push(format!("{label} ({})", absolute(href)));
        }
        Node::LinkCard {
            title,
            description,
            href,
        } => out.push(format!("{title}: {description} ({})", absolute(href))),
        Node::CodeBlock { code, .. } => out.push(code.clone()),
        Node::Command { unix, windows } => {
            out.push(unix.clone());
            out.extend(windows.clone());
        }
        Node::Steps { steps } => out.extend(steps.iter().map(|s| s.title.clone())),
        Node::Tabs { tabs } => out.extend(tabs.iter().map(|t| t.label.clone())),
        Node::Stack { .. } | Node::Columns { .. } => {}
    });
    out.join("\n")
}
