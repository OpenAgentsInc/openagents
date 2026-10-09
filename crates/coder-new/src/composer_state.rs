//! Rich state belongs to the input, not to the conversation or execution queue.
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Default, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum InputMode {
    #[default]
    Prompt,
    Bash,
}

#[derive(Clone, Default, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImageAttachment {
    pub id: Option<String>,
    /// A local path, URL, or embedded data URL supplied by the paste adapter.
    pub source: String,
}

#[derive(Clone, Default, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ComposerState {
    pub mode: InputMode,
    pub pasted: Vec<String>,
    pub images: Vec<ImageAttachment>,
}

impl crate::App {
    pub fn attach_image(&mut self, source: String, id: Option<String>) {
        self.composer.images.push(ImageAttachment { source, id });
        self.assign_image_ids();
    }

    pub(crate) fn assign_image_ids(&mut self) {
        let mut used = self
            .composer
            .images
            .iter()
            .filter_map(|image| image.id.clone())
            .collect::<std::collections::HashSet<_>>();
        for image in &mut self.composer.images {
            if image.id.is_none() {
                loop {
                    self.next_image_id += 1;
                    let id = format!("image-{}-{}", std::process::id(), self.next_image_id);
                    if used.insert(id.clone()) {
                        image.id = Some(id);
                        break;
                    }
                }
            }
        }
    }

    pub(crate) fn submit_bash(&mut self) {
        if !self.composer.images.is_empty() {
            self.live.notice = Some(
                "Bash input cannot execute image attachments. They remain in the composer.".into(),
            );
            return;
        }
        if self.draft.text.trim().is_empty() {
            return;
        }
        if self.live.busy {
            self.queue_prompt();
            return;
        }
        self.record_prompt();
        self.cancel_request();
        let command = std::mem::take(&mut self.draft.text);
        self.draft.cursor = 0;
        self.composer = ComposerState::default();
        self.live
            .entries
            .push(crate::live::Entry::User(format!("!{command}")));
        self.live.busy = true;
        self.scroll = u16::MAX;
        self.request = Some(crate::live::Request {
            id: self.request_id,
            key: model_access::ApiKey::new(""),
            kind: crate::live::Work::Bash {
                command,
                cwd: self
                    .cwd
                    .clone()
                    .unwrap_or_else(|| std::path::PathBuf::from(".")),
            },
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};
    #[test]
    fn bash_mode_routes_to_run_and_interrupt_restores_mode_and_paste() {
        let mut app = crate::App::default();
        app.set_mode(crate::Mode::Live);
        app.handle(Event::Key(KeyEvent::new(
            KeyCode::Char('!'),
            KeyModifiers::NONE,
        )));
        app.handle(Event::Paste("printf hello".into()));
        app.handle(Event::Key(KeyEvent::new(
            KeyCode::Enter,
            KeyModifiers::NONE,
        )));
        assert!(matches!(
            app.request.as_ref().unwrap().kind,
            crate::live::Work::Bash { .. }
        ));
        assert!(app.draft.text.is_empty());
        app.restore_unanswered_prompt();
        assert_eq!(app.draft.text, "printf hello");
        assert_eq!(app.composer.mode, InputMode::Bash);
        assert_eq!(app.composer.pasted, ["printf hello"]);
        app.restore_unanswered_prompt();
        assert_eq!(app.live.entries.len(), 0);
    }
    #[test]
    fn embedded_images_get_unique_ids_without_changing_existing_ids() {
        let mut app = crate::App::default();
        let existing = format!("image-{}-1", std::process::id());
        app.attach_image("existing".into(), Some(existing.clone()));
        app.attach_image("data:image/png;base64,AA".into(), None);
        assert_eq!(app.composer.images[0].id.as_ref(), Some(&existing));
        assert_ne!(app.composer.images[1].id.as_ref(), Some(&existing));
        let saved = app.composer.images.clone();
        app.assign_image_ids();
        assert_eq!(app.composer.images, saved);
    }
    #[test]
    fn image_paste_is_an_attachment_and_help_dismissal_precedes_queue() {
        let mut app = crate::App::default();
        app.set_mode(crate::Mode::Live);
        app.live.busy = true;
        app.handle(Event::Paste("data:image/png;base64,AA".into()));
        app.handle(Event::Key(KeyEvent::new(
            KeyCode::Enter,
            KeyModifiers::NONE,
        )));
        assert!(app.draft.text.is_empty());
        assert_eq!(app.queued_prompts.len(), 1);
        app.notice = Some(crate::slash::help());
        app.handle(Event::Key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE)));
        assert!(app.notice.is_none());
        assert_eq!(app.queued_prompts.len(), 1);
        app.handle(Event::Key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE)));
        assert!(app.queued_prompts.is_empty());
        assert_eq!(app.composer.images.len(), 1);
        assert!(app.composer.images[0].id.is_some());
    }
}
