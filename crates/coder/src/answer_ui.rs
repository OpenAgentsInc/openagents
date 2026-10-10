//! Interactive answers on the chat path (#11113, phase 1).
//!
//! A model answer may carry one ```` ```openui-lang ```` block of
//! components from [`openui_lang::catalog`]. This module is the worker's
//! side of that:
//!
//! - [`wants_ui`]: whether a routed turn's model is told the catalog. It
//!   reads only the router's typed decision (route, corpus, surface), never
//!   the message's words.
//! - [`note`]: what the model is told: the catalog, and on a follow-up the
//!   interface the last answer showed, which it edits by name.
//! - [`finish`]: the finished reply's block merged into the earlier
//!   interface by name ([`openui_lang::edit`]), validated, and, when the
//!   validator dropped something, one repair round trip
//!   ([`openui_lang::feedback`]) whose result is kept only when it draws
//!   more. It also gives the patch a surface already showing the earlier
//!   interface needs ([`patch_payload`]).

use std::time::Duration;

use openui_lang::embed::{self, Segment};
use openui_lang::{Document, edit, feedback};
use serde_json::{Value, json};

use crate::generate::{Generate, Message, Meta, Role};
use crate::router::{Corpus, RouteId, Surface};

/// The longest a repair round trip may take before the reply is kept as
/// the model wrote it (the renderer drops what is invalid either way).
pub const REPAIR_WAIT: Duration = Duration::from_secs(8);

/// The most bytes a patch payload carries; a larger patch is not sent and
/// the surface draws the result's whole program instead.
pub const MAX_PATCH_BYTES: usize = 16 * 1024;

/// Whether a routed turn's model is told the component catalog: a how-to
/// grounded in the product notes, on a surface that draws the components.
/// Other surfaces show the Markdown fallback, so a block there costs
/// tokens and buys nothing.
#[must_use]
pub fn wants_ui(route: RouteId, corpus: Corpus, surface: Surface) -> bool {
    surface == Surface::Web
        && corpus == Corpus::Product
        && matches!(route, RouteId::ProductKb | RouteId::Account)
}

/// The note added to the model's instructions on a turn [`wants_ui`]
/// picked: when to use components, the catalog, and, when the last answer
/// showed an interface (`earlier`), how to edit it.
#[must_use]
pub fn note(earlier: Option<&str>) -> String {
    let mut out = String::from(
        "Add components only when steps, commands, links, or choices make the answer easier to \
         act on; otherwise answer in prose alone. ",
    );
    out.push_str(&openui_lang::prompt());
    if let Some(program) = earlier {
        out.push_str(
            "\nYour last answer showed the interface below. To change it, write a block with only \
             the statements that change: a statement replaces the one with its name, a new name \
             adds one, `name = null` removes one, and whatever `root` no longer uses is dropped. \
             To show something new instead, write a new `root`.\n```openui-lang\n",
        );
        out.push_str(program);
        if !program.ends_with('\n') {
            out.push('\n');
        }
        out.push_str("```\n");
    }
    out
}

/// The interface the conversation's last answer showed: the last closed
/// block of the last assistant message that has one.
#[must_use]
pub fn earlier_program(input: &[Message]) -> Option<String> {
    input
        .iter()
        .rev()
        .filter(|message| message.role == Role::Assistant)
        .find_map(|message| last_block(&message.text).map(|(_, source)| source.to_owned()))
}

/// The last closed block of `text`, with where its source starts.
fn last_block(text: &str) -> Option<(usize, &str)> {
    embed::segments(text)
        .into_iter()
        .rev()
        .find_map(|segment| match segment {
            Segment::Ui {
                source,
                closed: true,
            } => Some((source.as_ptr() as usize - text.as_ptr() as usize, source)),
            _ => None,
        })
}

/// A finished reply, after [`finish`].
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Finished {
    /// The reply with its block replaced by the program to draw.
    pub text: String,
    /// The statements a surface showing the earlier interface needs, when
    /// there was one and it changed.
    pub patch: Option<String>,
    /// Diagnostics on the model's block, after merging.
    pub problems: usize,
    /// Diagnostics left on the program drawn.
    pub left: usize,
    /// Whether the repair round trip's program was kept.
    pub repaired: bool,
}

/// How well a parse draws: a root first, then fewer problems.
fn score(document: &Document) -> (bool, std::cmp::Reverse<usize>) {
    (
        document.root.is_some(),
        std::cmp::Reverse(document.diagnostics.len()),
    )
}

/// Settles `text`'s last block: merged into `earlier` by name, validated,
/// and repaired once through `door` when the validator dropped something.
/// A reply with no closed block comes back as it was.
pub async fn finish<G: Generate>(door: &G, text: &str, earlier: Option<&str>) -> Finished {
    let Some((at, block)) = last_block(text) else {
        return Finished {
            text: text.to_owned(),
            ..Finished::default()
        };
    };
    // Merging into nothing still puts `root` first and drops what it does
    // not reach.
    let merged = edit::apply(earlier.unwrap_or_default(), block);
    let mut program = if merged.source.trim().is_empty() {
        block.to_owned()
    } else {
        merged.source
    };
    let document = openui_lang::parse(&program);
    let problems = document.diagnostics.len();
    let mut left = problems;
    let mut repaired = false;
    if let Some(told) = feedback::feedback(&program, &document)
        && let Some(reply) = ask_repair(door, &told).await
    {
        let fixed = feedback::repair(&program, &reply);
        let redone = openui_lang::parse(&fixed.source);
        if score(&redone) > score(&document) {
            left = redone.diagnostics.len();
            program = fixed.source;
            repaired = true;
        }
    }
    let patch = earlier.and_then(|earlier| {
        let change = edit::apply(earlier, &program);
        change.changed().then(|| change.patch())
    });
    let mut out = String::with_capacity(text.len() + program.len());
    out.push_str(&text[..at]);
    out.push_str(&program);
    out.push_str(&text[at + block.len()..]);
    Finished {
        text: out,
        patch,
        problems,
        left,
        repaired,
    }
}

/// One repair call: the catalog as instructions, the feedback as the only
/// message. `None` when it fails or runs past [`REPAIR_WAIT`].
async fn ask_repair<G: Generate>(door: &G, told: &feedback::Feedback) -> Option<String> {
    let instructions = format!(
        "You correct a block of components in an answer. {}",
        openui_lang::prompt()
    );
    let input = [Message {
        role: Role::User,
        text: told.prompt(),
    }];
    let mut sink = |_: &str| {};
    let mut meta = |_: Meta| {};
    let call = door.generate(&instructions, &input, &mut sink, &mut meta);
    match tokio::time::timeout(REPAIR_WAIT, call).await {
        Ok(Ok((reply, _))) if !reply.trim().is_empty() => Some(reply),
        _ => None,
    }
}

/// The `ui_patch` feedback payload for `patch`, or `None` when it is too
/// large to send (the result carries the whole program either way).
/// Receivers that do not know the type ignore it.
#[must_use]
pub fn patch_payload(version: u64, patch: &str) -> Option<Value> {
    (!patch.is_empty() && patch.len() <= MAX_PATCH_BYTES)
        .then(|| json!({"v": version, "type": "ui_patch", "patch": patch}))
}

#[cfg(test)]
mod tests;
