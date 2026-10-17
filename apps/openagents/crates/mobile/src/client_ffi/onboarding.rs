//! First launch (#11996): a few skippable tutorial pages, shown once, then a
//! new chat. There is no sign-in. Rust owns the pages' words and whether the
//! tutorial was seen; the app paints the pages.
//!
//! Also the account boundary the tutorial and Settings stand on:
//! [`CoreAccount::start_identity`] and [`CoreAccount::identity`]. Both are
//! stand-ins until #11995 makes the identity from 12 words and signs in
//! silently.
use super::CoreAccount;
use std::{
    path::{Path, PathBuf},
    sync::atomic::{AtomicBool, Ordering},
};

/// One tutorial page. `id` is stable: the app picks its picture by it.
#[derive(uniffi::Record, Clone, Debug, PartialEq, Eq)]
pub struct OnboardingStep {
    pub id: String,
    pub title: String,
    pub body: String,
    /// The page's main button: "Next", or on the last page the way into
    /// the chat.
    pub action: String,
}

fn step(id: &str, title: &str, body: &str, action: &str) -> OnboardingStep {
    OnboardingStep {
        id: id.into(),
        title: title.into(),
        body: body.into(),
        action: action.into(),
    }
}

/// The tutorial, in order.
pub fn onboarding_steps() -> Vec<OnboardingStep> {
    vec![
        step(
            "ask",
            "Ask anything",
            "Ask a question, think something through, or start a coding task. We answer right here.",
            "Next",
        ),
        step(
            "code",
            "Coding runs where you want",
            "Coding tasks run in our cloud, or on your own computer once you connect it.",
            "Next",
        ),
        step(
            "connect",
            "Connect your computer",
            "Install OpenAgents on your computer, then scan the code it shows with this phone.",
            "Next",
        ),
        step(
            "backup",
            "Back up your account",
            "Your account lives on this phone. Back it up in Settings so you can get it back on a new one.",
            "Start chatting",
        ),
    ]
}

/// Whether this install has seen the tutorial. The record is a file in the
/// core's data folder, so it goes away with the app's data.
#[derive(uniffi::Object)]
pub struct CoreOnboarding {
    /// `None` for the demo, which remembers nothing between launches.
    record: Option<PathBuf>,
    seen: AtomicBool,
}

impl CoreOnboarding {
    fn record_in(data_dir: &Path) -> PathBuf {
        data_dir.join("onboarding").join("seen")
    }
}

#[uniffi::export]
impl CoreOnboarding {
    #[uniffi::constructor]
    pub fn new(data_dir: String) -> std::sync::Arc<Self> {
        let record = Self::record_in(Path::new(&data_dir));
        let seen = record.exists();
        std::sync::Arc::new(Self {
            record: Some(record),
            seen: AtomicBool::new(seen),
        })
    }

    /// The offline demo's tutorial: always unseen, and finishing it is
    /// forgotten when the app closes.
    #[uniffi::constructor]
    pub fn demo() -> std::sync::Arc<Self> {
        std::sync::Arc::new(Self {
            record: None,
            seen: AtomicBool::new(false),
        })
    }

    pub fn steps(&self) -> Vec<OnboardingStep> {
        onboarding_steps()
    }

    /// Whether to show the tutorial now.
    pub fn should_show(&self) -> bool {
        !self.seen.load(Ordering::SeqCst)
    }

    /// The person finished or skipped the tutorial; it doesn't come back.
    /// Writing the record is best effort: if it fails, the tutorial shows
    /// again on the next launch, which is harmless.
    pub fn finish(&self) {
        self.seen.store(true, Ordering::SeqCst);
        if let Some(record) = &self.record {
            if let Some(dir) = record.parent() {
                let _ = std::fs::create_dir_all(dir);
            }
            let _ = std::fs::write(record, b"1\n");
        }
    }
}

/// The person's own account identity, for Settings' "Your ID" row.
#[derive(uniffi::Record, Clone, Debug, PartialEq, Eq)]
pub struct IdentityView {
    /// The public key as `npub1…`.
    pub npub: String,
    /// Whether the 12 words were backed up and confirmed.
    pub words_saved: bool,
}

#[uniffi::export]
impl CoreAccount {
    /// Starts this phone's account on launch, with nothing for the person
    /// to do.
    ///
    /// TODO(#11995): make or load the 12 words, derive the NIP-06 key and
    /// sign in with it silently. Until then this restores a saved login if
    /// there is one, and otherwise leaves the phone signed out, where the
    /// chat answers as a visitor.
    pub fn start_identity(&self) {
        self.restore();
    }

    /// The account's identity, or `None` while there is none to show.
    ///
    /// TODO(#11995): return the NIP-06 identity and the words-saved marker.
    /// Settings shows "Your ID", "Back up your account" and "Restore from
    /// 12 words" only once this returns one.
    pub fn identity(&self) -> Option<IdentityView> {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_tutorial_shows_once_and_is_remembered() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().display().to_string();
        let first = CoreOnboarding::new(path.clone());
        assert!(first.should_show());
        first.finish();
        assert!(!first.should_show());
        assert!(!CoreOnboarding::new(path).should_show());
    }

    #[test]
    fn the_demo_tutorial_is_not_remembered() {
        let demo = CoreOnboarding::demo();
        assert!(demo.should_show());
        demo.finish();
        assert!(!demo.should_show());
        assert!(CoreOnboarding::demo().should_show());
    }

    #[test]
    fn the_tutorial_has_the_four_pages_and_ends_in_a_chat() {
        let steps = onboarding_steps();
        let ids: Vec<_> = steps.iter().map(|s| s.id.as_str()).collect();
        assert_eq!(ids, ["ask", "code", "connect", "backup"]);
        assert!(steps[..3].iter().all(|s| s.action == "Next"));
        assert_eq!(steps[3].action, "Start chatting");
    }

    #[test]
    fn the_tutorial_never_asks_to_sign_in_and_reads_plainly() {
        for s in onboarding_steps() {
            for text in [&s.title, &s.body, &s.action] {
                assert!(oa_copy::violations(text, &[]).is_empty(), "{text}");
                let lower = text.to_lowercase();
                assert!(
                    !lower.contains("sign in") && !lower.contains("log in"),
                    "{text}"
                );
                // OpenAgents speaks as "we".
                assert!(!text.split_whitespace().any(|w| w == "I"), "{text}");
            }
        }
    }
}
