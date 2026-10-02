//! **Give feedback**: a comment on text the tester selected in the app
//! (#10127).
//!
//! Long-pressing or selecting text on the phone, or right-clicking a
//! selection on the desktop, offers **Give feedback** beside Copy. Its
//! dialog quotes the selection and asks for a comment; **Send** files it as
//! a playtest [`Report`] of kind [`Kind::Comment`] with the selection in
//! [`Report::selection`] and the comment as `happened`. It travels exactly
//! as any report does: sealed with NIP-17 to the triage key
//! ([`crate::report::wrap`]) and read in `openagents playtest inbox`. It
//! carries the selected text, where it came from (the conversation, the
//! message's index, and a reply's route, tier, and model), the build, and
//! the device, and no other chat content.

use crate::report::{Context, Kind, MAX_TEXT_CHARS, Report, SCHEMA, Selection};

/// The selection menu's item.
pub const BUTTON: &str = "Give feedback";
/// The comment field's placeholder.
pub const PLACEHOLDER: &str = "What's wrong or what should change?";
/// What the dialog says once the feedback is sent.
pub const SENT: &str = "Sent";
/// What the dialog says when the build can't send yet and keeps it.
pub const SAVED: &str = "Saved. It sends once this build can reach OpenAgents.";

/// The selection as the report carries it: trimmed, and cut to
/// [`MAX_TEXT_CHARS`] characters, keeping its start.
#[must_use]
pub fn clip(text: &str) -> String {
    let text = text.trim();
    if text.chars().count() <= MAX_TEXT_CHARS {
        return text.to_owned();
    }
    let mut cut: String = text.chars().take(MAX_TEXT_CHARS - 1).collect();
    cut.push('…');
    cut
}

/// The report for a comment on `selection`.
///
/// # Errors
///
/// A sentence for the tester: no comment, or a report that fails
/// [`Report::check`].
pub fn report(context: Context, mut selection: Selection, comment: &str) -> Result<Report, String> {
    let comment = comment.trim();
    if comment.is_empty() {
        return Err("Write what's wrong or what should change.".into());
    }
    selection.text = clip(&selection.text);
    let report = Report {
        schema: SCHEMA.into(),
        context,
        kind: Kind::Comment,
        happened: comment.to_owned(),
        expected: String::new(),
        steps: String::new(),
        quote: false,
        task: None,
        session: None,
        screenshot: None,
        notes: vec![],
        chat: None,
        selection: Some(selection),
    };
    report.check()?;
    Ok(report)
}

#[cfg(test)]
#[path = "feedback/tests.rs"]
mod tests;
