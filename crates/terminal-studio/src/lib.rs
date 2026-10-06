//! Shared studio presentation and admitted mount services, separate from
//! the renderer and from the transport-free terminal application.
mod native;
pub mod onboarding;
pub mod opening;
pub mod route;
pub mod studio;
use std::path::PathBuf;
use std::sync::mpsc::Receiver;
use terminal_core::studio::{Prepared, View};

pub struct Native {
    home: Option<PathBuf>,
    #[cfg(feature = "onboarding-host")]
    onboarding: Option<std::sync::Arc<onboarding::host::Capture>>,
}
impl Native {
    pub fn new(home: Option<PathBuf>) -> Self {
        Self {
            home,
            #[cfg(feature = "onboarding-host")]
            onboarding: None,
        }
    }
}
#[cfg(feature = "onboarding-host")]
impl Native {
    pub fn with_onboarding(mut self, config: onboarding::host::Config) -> Self {
        self.onboarding = Some(std::sync::Arc::new(onboarding::host::Capture::new(config)));
        self
    }
}
impl terminal_core::studio::Transport for Native {
    fn read_studio(&self) -> Receiver<Result<View, String>> {
        native::studio_read(
            self.home.as_deref(),
            #[cfg(feature = "onboarding-host")]
            self.onboarding.clone(),
        )
    }
    fn prepare_studio(
        &self,
        source: &[u8],
        review: Option<&terminal_core::studio::Review>,
        line: &str,
        workspace: Option<&str>,
    ) -> Receiver<Result<Prepared, String>> {
        let receiver =
            native::studio_prepare(source, review, line, self.home.as_deref(), workspace);
        #[cfg(feature = "onboarding-host")]
        if let Some(capture) = &self.onboarding {
            let capture = capture.clone();
            let snapshot = serde_json::from_slice(source).ok();
            let review = review.and_then(|r| serde_json::from_slice(&r.source).ok());
            let (tx, rx) = std::sync::mpsc::channel();
            std::thread::spawn(move || {
                if let Ok(result) = receiver.recv() {
                    if let (Ok(prepared), Some(snapshot)) = (&result, snapshot) {
                        capture.prepare(&prepared.request, snapshot, review);
                    }
                    let _ = tx.send(result);
                }
            });
            return rx;
        }
        receiver
    }
    fn read_review(&self, task: &str) -> Receiver<Result<terminal_core::studio::Review, String>> {
        native::studio_review(task, self.home.as_deref())
    }
    fn send_studio(&self, command: &Prepared) -> Receiver<Result<String, String>> {
        native::studio_send(
            command,
            self.home.as_deref(),
            #[cfg(feature = "onboarding-host")]
            self.onboarding.clone(),
        )
    }
}
