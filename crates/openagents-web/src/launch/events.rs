//! The launch funnel's server steps, sent to the shared
//! [`playtest::events::record`] (the research stream's sink once the server
//! installs it, [`crate::research`]).
//!
//! A step always carries the playtest's session id (the `oa_playtest`
//! cookie). The page's own steps come from the research script
//! (`/static/r.js`) and from [`super::EVENTS`] ([`Step::from_page`]).

/// One server-side step. The names are the research stream's: lowercase
/// snake case, at most 48 characters.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Step {
    /// Play the beta was chosen on the homepage, in a known session.
    PlayClicked,
    /// The visitor started the playtest and agreed to be recorded.
    PlaytestStarted,
    /// The visitor started the playtest without recording.
    PlaytestOptedOut,
    /// The Haunted House opened inside a playtest.
    TutorialOpened,
    /// The page offered the survey (after a while, or the tutorial's end).
    TutorialFinishPrompted,
    /// The visitor chose Finish playtest.
    TutorialFinishClicked,
    /// The survey was shown.
    SurveyStarted,
    /// A survey was kept.
    SurveyCompleted,
    /// An email joined the waitlist during a playtest session.
    WaitlistJoined,
    /// The share link on the thank-you page was copied.
    ShareCopied,
    /// The profile form was shown.
    ProfileStarted,
    /// A profile was kept.
    ProfileCompleted,
    /// A signed-in account kept its first profile: sign-up returns to the
    /// profile, so this marks a sign-up that finished.
    SignupCompleted,
}

impl Step {
    /// Every step.
    pub const ALL: [Step; 13] = [
        Step::PlayClicked,
        Step::PlaytestStarted,
        Step::PlaytestOptedOut,
        Step::TutorialOpened,
        Step::TutorialFinishPrompted,
        Step::TutorialFinishClicked,
        Step::SurveyStarted,
        Step::SurveyCompleted,
        Step::WaitlistJoined,
        Step::ShareCopied,
        Step::ProfileStarted,
        Step::ProfileCompleted,
        Step::SignupCompleted,
    ];

    /// The step's stable name.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Step::PlayClicked => "play_clicked",
            Step::PlaytestStarted => "playtest_started",
            Step::PlaytestOptedOut => "playtest_opted_out",
            Step::TutorialOpened => "tutorial_opened",
            Step::TutorialFinishPrompted => "tutorial_finish_prompted",
            Step::TutorialFinishClicked => "tutorial_finish_clicked",
            Step::SurveyStarted => "survey_started",
            Step::SurveyCompleted => "survey_completed",
            Step::WaitlistJoined => "waitlist_joined",
            Step::ShareCopied => "share_copied",
            Step::ProfileStarted => "profile_started",
            Step::ProfileCompleted => "profile_completed",
            Step::SignupCompleted => "signup_completed",
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
            Step::TutorialOpened | Step::TutorialFinishPrompted | Step::ShareCopied
        )
    }
}

/// Send `step` for `session` with its properties.
pub(crate) fn emit(session: &str, step: Step, props: &[(&str, &str)]) {
    #[cfg(test)]
    kept::keep(session, step);
    playtest::events::record(session, step.name(), props);
}

/// The steps the tests emitted, by session, so tests that run at once each
/// read their own.
#[cfg(test)]
pub(crate) mod kept {
    use super::Step;
    use std::sync::Mutex;

    static KEPT: Mutex<Vec<(String, Step)>> = Mutex::new(Vec::new());

    pub(super) fn keep(session: &str, step: Step) {
        if let Ok(mut kept) = KEPT.lock() {
            kept.push((session.to_owned(), step));
        }
    }

    /// The steps recorded for `session`, in order.
    pub(crate) fn steps(session: &str) -> Vec<Step> {
        KEPT.lock()
            .unwrap()
            .iter()
            .filter(|(s, _)| s == session)
            .map(|(_, step)| *step)
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_step_has_a_unique_name_the_research_stream_takes() {
        let mut names: Vec<_> = Step::ALL.iter().map(|step| step.name()).collect();
        for step in Step::ALL {
            assert_eq!(Step::parse(step.name()), Some(step));
            assert!(
                crate::research::ingest::valid_name(step.name()),
                "{}",
                step.name()
            );
        }
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), Step::ALL.len());
        assert_eq!(Step::parse("nope"), None);
        // The research funnel's own names.
        for name in [
            "playtest_started",
            "survey_started",
            "survey_completed",
            "waitlist_joined",
            "playtest_opted_out",
            "signup_completed",
        ] {
            assert!(Step::parse(name).is_some(), "{name}");
        }
        assert!(Step::TutorialOpened.name().starts_with("tutorial_"));
        // The page may only report what it alone can see.
        assert!(!Step::SurveyCompleted.from_page());
        assert!(Step::TutorialFinishPrompted.from_page());
    }
}
