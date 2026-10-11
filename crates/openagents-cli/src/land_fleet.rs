//! The landing queue on openagents.com and the phone (#11250).
//!
//! While `openagents land work` runs, it reports the queue's entries as
//! items of kind `landing` on this computer's agent list
//! ([`coder_sync::activity::report_landing`]): each open entry, and the
//! ones that ended in the last [`RECENT_SECS`], titled `#N` and the entry's
//! summary, with a line that says where it is (`queued`, `landing:
//! checks`, `landed <commit>`, `bounced: <reason>`). They sit beside the
//! issue runs `chat work` reports on the same computer; the website keeps
//! both. Queue entries take no Stop or Message from the lists: an entry is
//! withdrawn with `openagents land withdraw`.
//!
//! Reporting needs the account's sign-in (`OPENAGENTS_APP_TOKEN`, else
//! Coder's); without one the queue only shows in `openagents land status`.

use std::time::Duration;

use coder::task::land_plan::Lane;
use coder::task::land_queue::{self, Entry, Queue, State};
use coder_sync::activity::{Item, LANDING, MAX_ITEMS};

/// How long an entry that ended stays on the list.
pub(crate) const RECENT_SECS: u64 = 6 * 3600;
/// How often the queue is reported while the integrator runs.
const EVERY: Duration = Duration::from_secs(10);
const TITLE_CHARS: usize = 120;
const LINE_CHARS: usize = 200;

fn short(text: &str, limit: usize) -> String {
    let line = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if line.chars().count() <= limit {
        return line;
    }
    let mut cut: String = line.chars().take(limit.saturating_sub(1)).collect();
    cut.push('…');
    cut
}

/// An entry's title: `#N` and its summary, without the summary's own
/// trailing `(#N)`.
fn title(entry: &Entry) -> String {
    let summary = entry.summary.trim();
    match entry.issue {
        Some(issue) => {
            let summary = summary
                .strip_suffix(&format!("(#{issue})"))
                .map_or(summary, str::trim_end);
            short(&format!("#{issue} {summary}"), TITLE_CHARS)
        }
        None => short(summary, TITLE_CHARS),
    }
}

/// Where an entry is, in a line.
pub(crate) fn line(entry: &Entry) -> String {
    let text = match entry.state {
        State::Queued => "queued".to_owned(),
        State::Landing => match entry.lane {
            Some(Lane::Fast) => "landing: quick checks".to_owned(),
            _ => "landing: checks".to_owned(),
        },
        State::Landed => match entry.commit.as_deref() {
            Some(commit) => format!("landed {}", &commit[..commit.len().min(10)]),
            None => "landed".to_owned(),
        },
        State::Bounced => match entry.reason.as_deref().map(str::trim) {
            Some(reason) if !reason.is_empty() => format!("bounced: {reason}"),
            _ => "bounced".to_owned(),
        },
        State::Withdrawn => "withdrawn".to_owned(),
    };
    short(&text, LINE_CHARS)
}

/// One entry as an item of the integrator computer's list.
pub(crate) fn item(entry: &Entry) -> Item {
    Item {
        id: entry.id.clone(),
        kind: LANDING.into(),
        title: title(entry),
        engine: None,
        status: match entry.state {
            State::Queued | State::Landing => "working",
            State::Landed => "done",
            State::Bounced => "failed",
            State::Withdrawn => "stopped",
        }
        .into(),
        started_unix: entry.submitted_at,
        finished_unix: (!entry.state.open()).then_some(entry.updated_at),
        cost_usd: None,
        tokens: None,
        session: None,
        question: None,
        line: Some(line(entry)),
    }
}

/// The entries the list shows at `now`: open ones first (the one landing,
/// then the queue in order), then those that ended in the last
/// [`RECENT_SECS`], newest first; at most [`MAX_ITEMS`].
pub(crate) fn items(entries: &[Entry], now: u64) -> Vec<Item> {
    let mut open: Vec<&Entry> = entries.iter().filter(|e| e.state.open()).collect();
    open.sort_by(|a, b| {
        (b.state == State::Landing)
            .cmp(&(a.state == State::Landing))
            .then_with(|| a.id.cmp(&b.id))
    });
    let mut ended: Vec<&Entry> = entries
        .iter()
        .filter(|e| !e.state.open() && now.saturating_sub(e.updated_at) < RECENT_SECS)
        .collect();
    ended.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
    open.into_iter()
        .chain(ended)
        .take(MAX_ITEMS)
        .map(item)
        .collect()
}

/// Report the queue at `queue` (as `--queue` named it) on this computer's
/// list every [`EVERY`] while the integrator runs, on a thread of its own.
/// Nothing runs when this computer is not signed in.
pub(crate) fn spawn(queue: Option<String>) {
    let saved = match crate::mac::saved(None) {
        Ok(saved) => saved,
        Err(why) => {
            eprintln!("land: the queue won't show on openagents.com or your phone: {why}");
            return;
        }
    };
    let computer = openagents_login::computer_name();
    let _ = std::thread::Builder::new()
        .name("land-fleet".into())
        .spawn(move || {
            let Ok(runtime) = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
            else {
                return;
            };
            let Ok(http) = reqwest::Client::builder()
                .timeout(Duration::from_secs(20))
                .build()
            else {
                return;
            };
            let Ok(store) = land_queue::open_default(queue.as_deref()) else {
                return;
            };
            let queue = Queue {
                store: store.as_ref(),
            };
            let mut known = std::collections::HashMap::new();
            let mut last: Option<Vec<Item>> = None;
            let mut sent_at = std::time::Instant::now();
            loop {
                if let Ok(entries) = queue.entries_cached(&mut known) {
                    let items = items(&entries, land_queue::now());
                    // Unchanged lists go again every minute, so the
                    // website keeps the computer's list fresh.
                    if last.as_ref() != Some(&items) || sent_at.elapsed() > Duration::from_secs(60)
                    {
                        let sent = runtime.block_on(coder_sync::activity::report_landing(
                            &http, &saved, &computer, &items,
                        ));
                        if sent.is_ok() {
                            last = Some(items);
                            sent_at = std::time::Instant::now();
                        }
                    }
                }
                std::thread::sleep(EVERY);
            }
        });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(id: &str, state: State, updated_at: u64) -> Entry {
        Entry {
            id: id.into(),
            branch: format!("land/{id}"),
            target: "main".into(),
            issue: Some(11250),
            summary: "Fleet rows for cloud runs (#11250)".into(),
            machine: "oa-dev-env-1".into(),
            submitted_at: 1_000,
            state,
            updated_at,
            ..Entry::default()
        }
    }

    #[test]
    fn each_state_reads_as_a_line_with_the_issue_first() {
        let mut e = entry("a", State::Queued, 1_000);
        let queued = item(&e);
        assert_eq!(queued.kind, "landing");
        assert_eq!(queued.title, "#11250 Fleet rows for cloud runs");
        assert_eq!(queued.status, "working");
        assert_eq!(queued.line.as_deref(), Some("queued"));
        assert_eq!(queued.started_unix, 1_000);
        assert_eq!(queued.finished_unix, None);
        assert_eq!(queued.cost_usd, None);

        e.state = State::Landing;
        e.lane = Some(Lane::Code);
        assert_eq!(line(&e), "landing: checks");
        e.lane = Some(Lane::Fast);
        assert_eq!(line(&e), "landing: quick checks");

        e.state = State::Landed;
        e.updated_at = 1_600;
        e.commit = Some("248c7ce8d6a1b2c3d4e5f60718293a4b5c6d7e8f".into());
        let landed = item(&e);
        assert_eq!(landed.line.as_deref(), Some("landed 248c7ce8d6"));
        assert_eq!(landed.status, "done");
        assert_eq!(landed.finished_unix, Some(1_600));

        e.state = State::Bounced;
        e.reason = Some("The checks fail after rebasing\nonto main.".into());
        let bounced = item(&e);
        assert_eq!(
            bounced.line.as_deref(),
            Some("bounced: The checks fail after rebasing onto main.")
        );
        assert_eq!(bounced.status, "failed");

        e.state = State::Withdrawn;
        assert_eq!(item(&e).status, "stopped");

        e.issue = None;
        e.summary = "Docs: tidy".into();
        assert_eq!(item(&e).title, "Docs: tidy");
    }

    #[test]
    fn the_list_shows_the_open_queue_then_what_ended_lately() {
        let now = 100_000;
        let entries = vec![
            entry("01-old-landed", State::Landed, now - RECENT_SECS - 1),
            entry("02-bounced", State::Bounced, now - 60),
            entry("03-queued", State::Queued, now - 30),
            entry("04-landing", State::Landing, now - 10),
            entry("05-landed", State::Landed, now - 5),
            entry("06-queued", State::Queued, now),
        ];
        let ids: Vec<String> = items(&entries, now).into_iter().map(|i| i.id).collect();
        assert_eq!(
            ids,
            [
                "04-landing",
                "03-queued",
                "06-queued",
                "05-landed",
                "02-bounced"
            ]
        );
        let many: Vec<Entry> = (0..40)
            .map(|n| entry(&format!("{n:03}"), State::Queued, now))
            .collect();
        assert_eq!(items(&many, now).len(), MAX_ITEMS);
    }
}
