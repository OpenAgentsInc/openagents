//! A consented static excerpt of one displayed block (#10697).
//!
//! An excerpt, `openagents.terminal-excerpt.v1`, is a bounded, scrubbed
//! copy of one finished block as the terminal displayed it: the command,
//! its status, the first and last lines of its output, how many lines were
//! left out between them, and whether the terminal still held the whole
//! output. It names its source by the mount's instance, the block number,
//! and a digest over the scrubbed block, and it carries a digest over its
//! own content.
//!
//! It is static: holding one grants no input, attachment, task, review, or
//! spending right, and nothing in it reaches the host terminal. It holds no
//! working directory, environment, other block, other pane, or clipboard
//! content, and no output the block did not display. A running block, a
//! full-screen program's block, and a block with no command are refused
//! rather than excerpted.
//!
//! Sharing is two steps through the mount's control request: `excerpt`
//! without `consent` answers the preview, and `excerpt` with `consent`
//! set to the preview's digest exports exactly that excerpt and records
//! its identity for the session. A preview that changed since, such as
//! output that left the terminal's retention, refuses the consent.

use serde::{Deserialize, Serialize};

use crate::blocks::Block;

/// The excerpt schema.
pub const SCHEMA: &str = "openagents.terminal-excerpt.v1";
/// Lines kept from the start and from the end of the output.
pub const HEAD_LINES: usize = 20;
pub const TAIL_LINES: usize = 20;
/// The most characters one kept line holds.
pub const LINE_CHARS: usize = 200;
/// The most characters of the command.
pub const COMMAND_CHARS: usize = 1_024;

/// Where an excerpt came from.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Source {
    /// The mount's instance ID, which is also its generation.
    pub instance: String,
    pub block: u64,
    /// `sha256:` over the scrubbed block: command, status, and the whole
    /// output the terminal displayed.
    pub digest: String,
}

/// A static excerpt.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Excerpt {
    pub v: String,
    pub source: Source,
    pub command: String,
    pub command_cut: bool,
    pub status: Option<i32>,
    pub elapsed_ms: Option<u64>,
    pub head: Vec<String>,
    pub tail: Vec<String>,
    /// Output lines between `head` and `tail` left out.
    pub omitted_lines: usize,
    /// Kept lines cut at [`LINE_CHARS`].
    pub lines_cut: usize,
    /// False when the terminal no longer held the block's whole output.
    pub output_complete: bool,
    /// `sha256:` over the excerpt with this field empty.
    pub digest: String,
}

fn sha(value: &impl Serialize) -> String {
    format!("sha256:{}", crate::proposals::digest(value))
}

fn cut(line: &str, limit: usize) -> (String, bool) {
    match line.char_indices().nth(limit) {
        Some((at, _)) => (line[..at].to_owned(), true),
        None => (line.to_owned(), false),
    }
}

/// The source digest of `block` as `scrub` shows it.
#[must_use]
pub fn source_digest(block: &Block, scrub: &dyn Fn(&str) -> String) -> String {
    sha(&(
        scrub(&block.command),
        block.status,
        scrub(&block.output),
        block.truncated,
    ))
}

/// The excerpt of `block` in mount `instance`.
///
/// # Errors
///
/// Why the block is not excerpted: still running, a full-screen program's
/// screen, or no command.
pub fn excerpt(
    block: &Block,
    instance: &str,
    scrub: &dyn Fn(&str) -> String,
) -> Result<Excerpt, String> {
    if block.end.is_none() {
        return Err(format!(
            "block {} is still running; its output isn't final",
            block.id
        ));
    }
    if block.alternate {
        return Err(format!(
            "block {} ran a full-screen program, whose screen isn't an excerpt",
            block.id
        ));
    }
    let command = crate::ascii::ascii(&scrub(&block.command));
    if command.trim().is_empty() {
        return Err(format!("block {} has no command to excerpt", block.id));
    }
    let (command, command_cut) = cut(&command, COMMAND_CHARS);
    let output = crate::ascii::ascii(&scrub(&block.output));
    let lines: Vec<&str> = if output.is_empty() {
        Vec::new()
    } else {
        output.lines().collect()
    };
    let mut lines_cut = 0;
    let mut keep = |line: &&str| {
        let (line, was_cut) = cut(line, LINE_CHARS);
        lines_cut += usize::from(was_cut);
        line
    };
    let (head, tail, omitted) = if lines.len() <= HEAD_LINES + TAIL_LINES {
        (lines.iter().map(&mut keep).collect(), Vec::new(), 0)
    } else {
        (
            lines[..HEAD_LINES].iter().map(&mut keep).collect(),
            lines[lines.len() - TAIL_LINES..]
                .iter()
                .map(&mut keep)
                .collect(),
            lines.len() - HEAD_LINES - TAIL_LINES,
        )
    };
    let mut excerpt = Excerpt {
        v: SCHEMA.into(),
        source: Source {
            instance: instance.to_owned(),
            block: block.id,
            digest: source_digest(block, scrub),
        },
        command,
        command_cut,
        status: block.status,
        elapsed_ms: block.elapsed_ms,
        head,
        tail,
        omitted_lines: omitted,
        lines_cut,
        output_complete: !block.truncated,
        digest: String::new(),
    };
    excerpt.digest = sha(&excerpt);
    Ok(excerpt)
}

/// Checks an excerpt as a recipient can, without the terminal: its schema,
/// bounds, and its digest over its own content.
///
/// # Errors
///
/// What does not hold.
pub fn verify(excerpt: &Excerpt) -> Result<(), String> {
    if excerpt.v != SCHEMA {
        return Err(format!("not {SCHEMA}"));
    }
    if excerpt.head.len() > HEAD_LINES || excerpt.tail.len() > TAIL_LINES {
        return Err("more lines than an excerpt keeps".into());
    }
    if excerpt.command.chars().count() > COMMAND_CHARS
        || excerpt
            .head
            .iter()
            .chain(&excerpt.tail)
            .any(|line| line.chars().count() > LINE_CHARS)
    {
        return Err("a line longer than an excerpt keeps".into());
    }
    if excerpt.omitted_lines > 0 && excerpt.tail.is_empty() {
        return Err("lines are omitted with no tail".into());
    }
    let mut unsigned = excerpt.clone();
    unsigned.digest = String::new();
    if sha(&unsigned) != excerpt.digest {
        return Err("the excerpt's digest does not match its content".into());
    }
    Ok(())
}

/// Whether `excerpt` was made from exactly `block`, for the sharer, who
/// still holds the block.
#[must_use]
pub fn same_source(excerpt: &Excerpt, block: &Block, scrub: &dyn Fn(&str) -> String) -> bool {
    excerpt.source.block == block.id && excerpt.source.digest == source_digest(block, scrub)
}

/// The excerpt as plain text: the preview a person consents to, and the
/// text a recipient reads.
#[must_use]
pub fn text(excerpt: &Excerpt) -> String {
    let mut out = vec![format!(
        "$ {}{}",
        excerpt.command,
        if excerpt.command_cut { " [cut]" } else { "" }
    )];
    out.extend(excerpt.head.iter().cloned());
    if excerpt.omitted_lines > 0 {
        out.push(format!("[{} lines left out]", excerpt.omitted_lines));
    }
    out.extend(excerpt.tail.iter().cloned());
    if !excerpt.output_complete {
        out.push("[the terminal no longer held the whole output]".into());
    }
    out.push(format!(
        "[exit {}; block {} of terminal {}; {}]",
        excerpt
            .status
            .map_or_else(|| "unknown".to_owned(), |status| status.to_string()),
        excerpt.source.block,
        excerpt.source.instance,
        excerpt.digest
    ));
    out.join("\n")
}

/// One export this mount made: what went out, and when.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Exported {
    pub digest: String,
    pub block: u64,
    pub at_ms: u64,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::blocks::Position;

    fn block(id: u64, output: &str) -> Block {
        Block {
            id,
            command: "cargo test --token sk-live-secret".into(),
            cwd: Some("/home/ana/private".into()),
            start: Position { line: 0, col: 0 },
            end: Some(Position { line: 1, col: 0 }),
            status: Some(101),
            started_ms: 0,
            elapsed_ms: Some(1200),
            output: output.into(),
            truncated: false,
            collapsed: false,
            alternate: false,
        }
    }

    fn scrub(text: &str) -> String {
        text.replace("sk-live-secret", "[secret]")
    }

    #[test]
    fn an_excerpt_keeps_head_tail_and_status_and_nothing_else() {
        let output: Vec<String> = (1..=100).map(|n| format!("line {n}")).collect();
        let block = block(7, &output.join("\n"));
        let excerpt = excerpt(&block, "mount-1", &scrub).unwrap();
        verify(&excerpt).unwrap();
        assert!(same_source(&excerpt, &block, &scrub));
        assert_eq!(excerpt.command, "cargo test --token [secret]");
        assert_eq!(excerpt.head.len(), HEAD_LINES);
        assert_eq!(excerpt.tail.last().map(String::as_str), Some("line 100"));
        assert_eq!(excerpt.omitted_lines, 60);
        let shown = text(&excerpt);
        assert!(shown.contains("[60 lines left out]"));
        assert!(shown.contains("[exit 101; block 7 of terminal mount-1; sha256:"));
        let json = serde_json::to_string(&excerpt).unwrap();
        assert!(!json.contains("/home/ana") && !json.contains("sk-live"));
    }

    #[test]
    fn running_full_screen_and_empty_blocks_are_refused() {
        let mut running = block(1, "partial");
        running.end = None;
        assert!(
            excerpt(&running, "m", &scrub)
                .unwrap_err()
                .contains("still running")
        );
        let mut screen = block(2, "\x1b[?1049h");
        screen.alternate = true;
        assert!(
            excerpt(&screen, "m", &scrub)
                .unwrap_err()
                .contains("full-screen")
        );
        let mut empty = block(3, "");
        empty.command = " ".into();
        assert!(excerpt(&empty, "m", &scrub).is_err());
    }

    #[test]
    fn evicted_or_long_output_is_marked_and_a_changed_excerpt_fails() {
        let mut evicted = block(4, &"x".repeat(LINE_CHARS + 50));
        evicted.truncated = true;
        let made = excerpt(&evicted, "m", &scrub).unwrap();
        assert!(!made.output_complete);
        assert_eq!(made.lines_cut, 1);
        assert!(text(&made).contains("no longer held the whole output"));
        let mut forged = made.clone();
        forged.head[0] = "something else".into();
        assert!(verify(&forged).is_err());
        let other = block(4, "different output");
        assert!(!same_source(&made, &other, &scrub));
    }
}
