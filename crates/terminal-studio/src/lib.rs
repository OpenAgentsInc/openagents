//! Shared studio presentation and admitted mount services, separate from
//! the renderer and from the transport-free terminal application.
mod native;
pub mod opening;
pub mod route;
pub mod studio;
use std::path::PathBuf;
use std::sync::mpsc::Receiver;
use terminal_core::studio::{Prepared, View};

pub struct Native {
    home: Option<PathBuf>,
}
impl Native {
    pub fn new(home: Option<PathBuf>) -> Self {
        Self { home }
    }
}
impl terminal_core::studio::Transport for Native {
    fn read_studio(&self) -> Receiver<Result<View, String>> {
        native::studio_read(self.home.as_deref())
    }
    fn prepare_studio(
        &self,
        source: &[u8],
        review: Option<&terminal_core::studio::Review>,
        line: &str,
        workspace: Option<&str>,
    ) -> Receiver<Result<Prepared, String>> {
        native::studio_prepare(source, review, line, self.home.as_deref(), workspace)
    }
    fn read_review(&self, task: &str) -> Receiver<Result<terminal_core::studio::Review, String>> {
        native::studio_review(task, self.home.as_deref())
    }
    fn send_studio(&self, command: &Prepared) -> Receiver<Result<String, String>> {
        native::studio_send(command, self.home.as_deref())
    }
}
