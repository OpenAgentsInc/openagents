//! `playtest::events`: the named funnel steps of the launch playtest, and
//! the hook that receives them.
//!
//! This module records nothing itself. Each step goes to the installed
//! [`Sink`], which is [`Noop`] until the analytics service installs its own
//! with [`install`]. A step carries the playtest's random session id when
//! the visitor started a playtest and agreed to have their steps recorded;
//! it never carries an email, an account id, an IP address, or free text.
//!
//! Steps come from two places: the server's own handlers (a page shown, a
//! form kept), and the few the page's script reports to
//! [`super::EVENTS`] ([`Step::from_page`]).

use std::sync::{Arc, RwLock};

/// One step of the launch funnel, in the order a visitor usually meets
/// them. The names are stable: the analytics side keys on them.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Step {
    /// The launch homepage was shown.
    LandingViewed,
    /// Play the beta was chosen on the homepage.
    PlayClicked,
    /// An email joined the waitlist.
    WaitlistJoined,
    /// The playtest intro was shown.
    IntroViewed,
    /// The visitor started the playtest.
    PlaytestStarted,
    /// The Haunted House opened inside a playtest.
    TutorialOpened,
    /// The page offered the survey (after a while, or the tutorial's end).
    FinishPrompted,
    /// The visitor chose Finish playtest.
    FinishClicked,
    /// The survey was shown.
    SurveyViewed,
    /// A survey was kept.
    SurveySubmitted,
    /// The share link on the thank-you page was copied.
    ShareCopied,
    /// The profile form was shown.
    ProfileViewed,
    /// A profile was kept.
    ProfileSubmitted,
}

impl Step {
    /// Every step, in funnel order.
    pub const ALL: [Step; 13] = [
        Step::LandingViewed,
        Step::PlayClicked,
        Step::WaitlistJoined,
        Step::IntroViewed,
        Step::PlaytestStarted,
        Step::TutorialOpened,
        Step::FinishPrompted,
        Step::FinishClicked,
        Step::SurveyViewed,
        Step::SurveySubmitted,
        Step::ShareCopied,
        Step::ProfileViewed,
        Step::ProfileSubmitted,
    ];

    /// The step's stable name.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Step::LandingViewed => "landing_viewed",
            Step::PlayClicked => "play_clicked",
            Step::WaitlistJoined => "waitlist_joined",
            Step::IntroViewed => "intro_viewed",
            Step::PlaytestStarted => "playtest_started",
            Step::TutorialOpened => "tutorial_opened",
            Step::FinishPrompted => "finish_prompted",
            Step::FinishClicked => "finish_clicked",
            Step::SurveyViewed => "survey_viewed",
            Step::SurveySubmitted => "survey_submitted",
            Step::ShareCopied => "share_copied",
            Step::ProfileViewed => "profile_viewed",
            Step::ProfileSubmitted => "profile_submitted",
        }
    }

    /// The step named `name`.
    #[must_use]
    pub fn parse(name: &str) -> Option<Step> {
        Step::ALL.into_iter().find(|step| step.name() == name)
    }

    /// Whether the page's script may report this step. Every other step
    /// is the server's to emit.
    #[must_use]
    pub const fn from_page(self) -> bool {
        matches!(
            self,
            Step::TutorialOpened | Step::FinishPrompted | Step::ShareCopied
        )
    }
}

/// One step as it happened.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Event {
    pub step: Step,
    /// The playtest's session id (`pt_` and 24 hex digits), when the
    /// visitor started one and agreed to be recorded.
    pub session: Option<String>,
    /// Where the visitor came from (`utm_source` or `ref`), lowercased and
    /// limited to letters, digits, `-`, and `_`.
    pub source: Option<String>,
    /// `mobile`, `tablet`, or `desktop`, from the user agent.
    pub device: Option<&'static str>,
    pub at_unix: u64,
}

/// Receives each step. Implementations must return quickly and never
/// fail the request: queue or count, then return.
pub trait Sink: Send + Sync {
    fn record(&self, event: &Event);
}

/// The default sink: it drops every step.
pub struct Noop;

impl Sink for Noop {
    fn record(&self, _event: &Event) {}
}

static SINK: RwLock<Option<Arc<dyn Sink>>> = RwLock::new(None);

/// Send every later step to `sink`, replacing the one before.
pub fn install(sink: Arc<dyn Sink>) {
    if let Ok(mut current) = SINK.write() {
        *current = Some(sink);
    }
}

/// Hand `event` to the installed sink, or drop it when there is none.
pub fn emit(event: &Event) {
    let sink = SINK.read().ok().and_then(|current| current.clone());
    match sink {
        Some(sink) => sink.record(event),
        None => Noop.record(event),
    }
}

/// The steps every test emitted, kept by one shared sink, so tests that
/// run at once each read their own session's steps.
#[cfg(test)]
pub(crate) mod kept {
    use super::{Arc, Event, Sink, Step, install};
    use std::sync::{Mutex, OnceLock};

    struct Keep(Mutex<Vec<Event>>);
    impl Sink for Keep {
        fn record(&self, event: &Event) {
            if let Ok(mut kept) = self.0.lock() {
                kept.push(event.clone());
            }
        }
    }

    fn keep() -> &'static Arc<Keep> {
        static KEEP: OnceLock<Arc<Keep>> = OnceLock::new();
        KEEP.get_or_init(|| {
            let keep = Arc::new(Keep(Mutex::new(Vec::new())));
            install(keep.clone());
            keep
        })
    }

    /// Install the shared sink before a test emits anything.
    pub(crate) fn start() {
        keep();
    }

    /// The steps recorded for `session`, in order.
    pub(crate) fn steps(session: &str) -> Vec<Step> {
        keep()
            .0
            .lock()
            .unwrap()
            .iter()
            .filter(|e| e.session.as_deref() == Some(session))
            .map(|e| e.step)
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_step_has_a_unique_name_that_parses_back() {
        let mut names: Vec<_> = Step::ALL.iter().map(|step| step.name()).collect();
        for step in Step::ALL {
            assert_eq!(Step::parse(step.name()), Some(step));
            assert!(
                step.name()
                    .bytes()
                    .all(|b| b.is_ascii_lowercase() || b == b'_')
            );
        }
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), Step::ALL.len());
        assert_eq!(Step::parse("nope"), None);
        // The page may only report what it alone can see.
        assert!(!Step::SurveySubmitted.from_page());
        assert!(Step::FinishPrompted.from_page());
    }

    #[test]
    fn an_installed_sink_receives_each_step() {
        kept::start();
        let session = "pt_events_unit_test";
        emit(&Event {
            step: Step::IntroViewed,
            session: Some(session.into()),
            source: None,
            device: Some("desktop"),
            at_unix: 1,
        });
        assert_eq!(kept::steps(session), [Step::IntroViewed]);
    }
}
