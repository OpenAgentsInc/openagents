//! The one question after `coder login` (#11089): where should this
//! computer's chats live? "Sync all my chats" or "Keep chats on this
//! computer". Asked once; the answer is kept in `sync.json`
//! ([`coder_sync::Settings::chosen`]) and told to the website, which keeps
//! the same choice per computer. The website's "Connect your terminal"
//! page asks it too: whichever answers first wins, so while the terminal
//! waits for a typed answer it also checks the website every two seconds.
//!
//! Syncing all sends the earlier chats too (the newest
//! [`crate::account_sync::EARLIER`]), now, so they are on the website when
//! the command ends.

use std::io::{BufRead, IsTerminal, Write};
use std::path::Path;
use std::sync::mpsc;
use std::time::Duration;

use coder_sync::{Choice, Settings};
use openagents_login::Saved;

/// The question, as the terminal shows it.
pub const QUESTION: &str = "Where should this computer's chats live?
  1. Sync all my chats: they show on openagents.com, earlier ones too.
  2. Keep chats on this computer: nothing is sent.
Type 1 or 2 (or choose on the website):";

/// A typed answer, if it is one.
#[must_use]
pub fn parse(answer: &str) -> Option<Choice> {
    match answer.trim().to_ascii_lowercase().as_str() {
        "1" | "sync" | "all" | "sync all" | "y" | "yes" => Some(Choice::All),
        "2" | "keep" | "local" | "no" | "n" => Some(Choice::Local),
        _ => None,
    }
}

/// Where the answer came from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum From {
    Terminal,
    Website,
}

/// Wait for a typed answer on `lines`, checking `web` between lines;
/// `None` when the input ends first.
pub fn ask(
    out: &mut impl Write,
    lines: &mpsc::Receiver<String>,
    mut web: impl FnMut() -> Option<Choice>,
    every: Duration,
) -> Option<(Choice, From)> {
    let _ = writeln!(out, "\n{QUESTION}");
    let _ = out.flush();
    loop {
        match lines.recv_timeout(every) {
            Ok(line) => match parse(&line) {
                Some(choice) => return Some((choice, From::Terminal)),
                None => {
                    let _ = writeln!(
                        out,
                        "Type 1 to sync all your chats, or 2 to keep them here:"
                    );
                    let _ = out.flush();
                }
            },
            Err(mpsc::RecvTimeoutError::Timeout) => {
                if let Some(choice) = web() {
                    return Some((choice, From::Website));
                }
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => return None,
        }
    }
}

/// Keep `choice` in `dir`, tell the website (when it came from here), and,
/// for "Sync all my chats", send this computer's chats now.
pub fn apply(
    dir: &Path,
    saved: &Saved,
    choice: Choice,
    from: From,
    out: &mut impl Write,
) -> Result<(), String> {
    let computer = openagents_login::computer_name();
    let mut settings = Settings::load(dir);
    settings.chosen = true;
    settings.on = choice == Choice::All;
    settings.store(dir)?;
    if from == From::Terminal {
        let _ = coder_sync::choose_now(saved, &computer, choice);
    }
    let said = |out: &mut dyn Write, text: &str| writeln!(out, "{text}").map_err(|e| e.to_string());
    let chosen_there = if from == From::Website {
        " (chosen on the website)"
    } else {
        ""
    };
    if choice == Choice::Local {
        return said(
            out,
            &format!(
                "Chats stay on this computer{chosen_there}. Run /sync all in Coder to sync them later."
            ),
        );
    }
    said(out, &format!("Syncing all your chats{chosen_there}…"))?;
    let _ = out.flush();
    let screen = secret_screen::Screen::host();
    let store = crate::sessions::Store::under(dir);
    let mut uploads = Vec::new();
    for summary in store
        .recent(crate::account_sync::EARLIER)
        .unwrap_or_default()
    {
        let Ok(document) = store.read(&summary.id) else {
            continue;
        };
        let upload = coder_sync::upload(&document, &computer, &screen);
        if settings.sends(&summary.id, &upload.digest) {
            uploads.push((summary.id, upload));
        }
    }
    // Oldest first, so the newest is on top in the sidebar.
    uploads.reverse();
    let wanted = uploads.len();
    let sent = coder_sync::send_now(saved, uploads);
    let count = sent.len();
    settings.sent.extend(sent);
    settings.store(dir)?;
    let text = match (count, wanted) {
        (0, 0) => "No chats here yet. New ones sync as you go.".to_owned(),
        (1, 1) => "Synced 1 chat. It's in the sidebar on openagents.com.".to_owned(),
        (n, w) if n == w => format!("Synced {n} chats. They're in the sidebar on openagents.com."),
        (n, w) => format!("Synced {n} of {w} chats. Coder sends the rest the next time it runs."),
    };
    said(out, &text)
}

/// After `coder login`: the website's choice if it has one, else the
/// question (in a terminal), else a pointer to where to choose.
pub fn after_login(dir: &Path, saved: &Saved, out: &mut impl Write) -> Result<(), String> {
    let computer = openagents_login::computer_name();
    if let Some(choice) = coder_sync::choice_now(saved, &computer) {
        return apply(dir, saved, choice, From::Website, out);
    }
    let settings = Settings::load(dir);
    if let Some(choice) = Choice::of(&settings) {
        // Chosen here before: the website learns it.
        let _ = coder_sync::choose_now(saved, &computer, choice);
        return Ok(());
    }
    if !std::io::stdin().is_terminal() {
        return writeln!(
            out,
            "Choose where this computer's chats live with /sync in Coder, or in Settings on openagents.com."
        )
        .map_err(|e| e.to_string());
    }
    let (send, lines) = mpsc::channel();
    std::thread::spawn(move || {
        for line in std::io::stdin().lock().lines() {
            let Ok(line) = line else { return };
            if send.send(line).is_err() {
                return;
            }
        }
    });
    let check = saved.clone();
    let asked = ask(
        out,
        &lines,
        || coder_sync::choice_now(&check, &computer),
        Duration::from_secs(2),
    );
    match asked {
        Some((choice, from)) => apply(dir, saved, choice, from, out),
        None => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn answers_are_typed_loosely() {
        assert_eq!(parse(" 1 "), Some(Choice::All));
        assert_eq!(parse("Sync"), Some(Choice::All));
        assert_eq!(parse("2"), Some(Choice::Local));
        assert_eq!(parse("keep"), Some(Choice::Local));
        assert_eq!(parse("maybe"), None);
    }

    #[test]
    fn the_question_waits_for_a_typed_answer_or_the_website() {
        let (send, lines) = mpsc::channel();
        send.send("what".to_owned()).unwrap();
        send.send("2".to_owned()).unwrap();
        let mut out = Vec::new();
        let asked = ask(&mut out, &lines, || None, Duration::from_millis(10));
        assert_eq!(asked, Some((Choice::Local, From::Terminal)));
        let shown = String::from_utf8(out).unwrap();
        assert!(
            shown.contains("Sync all my chats") && shown.contains("Keep chats on this computer")
        );
        assert!(shown.contains("Type 1 to sync all your chats"));

        // Nothing typed; the website answers.
        let (_keep, lines) = mpsc::channel::<String>();
        let mut checks = 0;
        let asked = ask(
            &mut Vec::new(),
            &lines,
            || {
                checks += 1;
                (checks == 3).then_some(Choice::All)
            },
            Duration::from_millis(5),
        );
        assert_eq!(asked, Some((Choice::All, From::Website)));

        // The input ended: no answer.
        let (send, lines) = mpsc::channel::<String>();
        drop(send);
        assert_eq!(
            ask(&mut Vec::new(), &lines, || None, Duration::from_millis(5)),
            None
        );
    }

    #[test]
    fn keeping_chats_here_is_remembered_and_sends_nothing() {
        let dir = tempfile::tempdir().unwrap();
        let saved: Saved = serde_json::from_value(serde_json::json!({
            "origin": "http://127.0.0.1:9", "account": "acct_1", "label": "Octo",
            "expires_at": u64::MAX, "token": "sess_x",
        }))
        .unwrap();
        let mut out = Vec::new();
        apply(dir.path(), &saved, Choice::Local, From::Website, &mut out).unwrap();
        let settings = Settings::load(dir.path());
        assert!(settings.chosen && !settings.on);
        assert!(
            String::from_utf8(out)
                .unwrap()
                .contains("Chats stay on this computer (chosen on the website).")
        );
        // Sync all with nothing saved here: says so.
        let mut out = Vec::new();
        apply(dir.path(), &saved, Choice::All, From::Website, &mut out).unwrap();
        assert!(Settings::load(dir.path()).on);
        assert!(
            String::from_utf8(out)
                .unwrap()
                .contains("No chats here yet.")
        );
    }
}
