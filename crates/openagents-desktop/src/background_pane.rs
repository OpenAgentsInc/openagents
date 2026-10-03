//! Settings' Background page (docs/background, phase 3): each background
//! rule on this computer, on or paused, its last result and when, and a
//! button to pause or resume it. It reads and writes the same rules the
//! host runs (`~/.openagents/background`), as `openagents background
//! list|pause|resume` and the host's `background.*` methods do, so every
//! surface shows the same thing. The newest notice is also a desktop
//! notification ([`crate::notices::Notices::observe_background`]).

/// Whether a rule runs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Status {
    On,
    Paused,
    Off,
    /// Its file does not read; the line says why.
    Broken,
}

/// One rule as the page shows it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Rule {
    pub id: String,
    pub name: String,
    pub status: Status,
    /// The last run's one line, or why the rule is broken.
    pub last: Option<String>,
    /// When it last ran (seconds since the epoch).
    pub when: Option<u64>,
}

impl Rule {
    /// The status line: on or paused, the last result, and how long ago.
    #[must_use]
    pub fn line(&self, now: u64) -> String {
        let mut parts = vec![
            match self.status {
                Status::On => "On",
                Status::Paused => "Paused",
                Status::Off => "Off",
                Status::Broken => "Broken",
            }
            .to_owned(),
        ];
        if let Some(last) = &self.last {
            parts.push(last.clone());
        }
        if let Some(when) = self.when {
            parts.push(ago(now.saturating_sub(when)));
        }
        parts.join(" · ")
    }

    /// Whether the button resumes it (it is paused or off).
    #[must_use]
    pub fn resumes(&self) -> bool {
        matches!(self.status, Status::Paused | Status::Off)
    }
}

/// `secs` ago, in a few words.
#[must_use]
pub fn ago(secs: u64) -> String {
    match secs {
        0..60 => "just now".into(),
        60..3600 => format!("{} min ago", secs / 60),
        3600..86_400 => format!("{} h ago", secs / 3600),
        _ => format!("{} d ago", secs / 86_400),
    }
}

/// The rules under `layout`, as the page shows them.
#[cfg(unix)]
#[must_use]
pub fn rows(layout: &background::Layout, now: u64) -> Vec<Rule> {
    background::view::list(layout)
        .into_iter()
        .map(|row| {
            let status = match (&row.error, row.enabled, row.paused_until) {
                (Some(_), ..) => Status::Broken,
                (None, false, _) => Status::Off,
                (None, true, Some(until)) if until > now => Status::Paused,
                (None, true, _) => Status::On,
            };
            Rule {
                last: row.error.clone().or(row.state.last_result.clone()),
                when: row.state.last_run,
                id: row.id,
                name: row.name,
                status,
            }
        })
        .collect()
}

/// Pause or resume rule `id`.
///
/// # Errors
/// No such rule, or it cannot be saved.
#[cfg(unix)]
pub fn set(layout: &background::Layout, id: &str, resume: bool) -> Result<(), String> {
    background::view::pause(layout, id, None, resume).map(drop)
}

/// The newest background notice: when and its line.
#[cfg(unix)]
#[must_use]
pub fn latest(layout: &background::Layout) -> Option<(u64, String)> {
    background::store::State::load(layout)
        .rules
        .values()
        .filter_map(|state| state.notice.clone())
        .max_by_key(|(at, _)| *at)
}

/// This user's layout, from `HOME` and the task store Coder uses.
#[cfg(unix)]
#[must_use]
pub fn here() -> Option<background::Layout> {
    background::Layout::from_env().ok()
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;

    #[test]
    fn rows_show_status_last_result_and_pause_and_resume_change_it() {
        let home = tempfile::tempdir().unwrap();
        let layout = background::Layout::new(home.path(), None).unwrap();
        let now = 2_000_000_000;
        let all = rows(&layout, now);
        let disk = all.iter().find(|r| r.id == "disk").unwrap();
        assert_eq!(disk.status, Status::Off);
        assert!(disk.resumes());
        assert_eq!(disk.line(now), "Off");
        set(&layout, "disk", true).unwrap();
        background::store::State::update(&layout, "disk", |state| {
            state.last_run = Some(now - 7200);
            state.last_result = Some("Freed 4 GB: 2 old build folders.".into());
            state.notice = Some((now - 7200, "Freed 4 GB: 2 old build folders.".into()));
        });
        let disk = rows(&layout, now)
            .into_iter()
            .find(|r| r.id == "disk")
            .unwrap();
        assert_eq!(disk.status, Status::On);
        assert_eq!(
            disk.line(now),
            "On · Freed 4 GB: 2 old build folders. · 2 h ago"
        );
        assert_eq!(latest(&layout).unwrap().0, now - 7200);
        set(&layout, "disk", false).unwrap();
        let disk = rows(&layout, now)
            .into_iter()
            .find(|r| r.id == "disk")
            .unwrap();
        assert_eq!(disk.status, Status::Off);
        assert!(set(&layout, "nope", true).is_err());
    }

    #[test]
    fn ages_read_in_a_few_words() {
        assert_eq!(ago(5), "just now");
        assert_eq!(ago(600), "10 min ago");
        assert_eq!(ago(3 * 86_400), "3 d ago");
    }
}
