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

    /// Attaches dropped or pasted files, or says why one cannot attach.
    pub(crate) fn attach_paths(&mut self, paths: Vec<std::path::PathBuf>) {
        for path in paths {
            match crate::attachments::from_path(&path) {
                Ok(_) => self.attach_image(path.display().to_string(), None),
                Err(error) => {
                    self.live.notice = Some(error);
                    return;
                }
            }
        }
    }

    /// Saves this prompt's attachments (pasted, dropped, and `@path` ones)
    /// and adds one note line per attachment to the draft (#11173). Returns
    /// how many were attached; on an error nothing changes.
    pub(crate) fn attach_to_prompt(&mut self) -> Result<usize, String> {
        use crate::attachments;
        let cwd = self.cwd.clone().unwrap_or_else(|| {
            std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."))
        });
        let mut loaded = Vec::new();
        for image in &self.composer.images {
            loaded.push(if image.source.starts_with("data:") {
                attachments::from_data_url(&image.source)?
            } else {
                attachments::from_path(std::path::Path::new(&image.source))?
            });
        }
        for path in attachments::mentioned_paths(&self.draft.text, &cwd) {
            if !self
                .composer
                .images
                .iter()
                .any(|image| std::path::Path::new(&image.source) == path)
            {
                loaded.push(attachments::from_path(&path)?);
            }
        }
        if loaded.is_empty() {
            return Ok(0);
        }
        if loaded.len() > attachments::MAX_ATTACHMENTS {
            return Err(format!(
                "A message can carry {} attachments at most.",
                attachments::MAX_ATTACHMENTS
            ));
        }
        let dir = self
            .attachment_dir
            .clone()
            .or_else(attachments::default_dir)
            .ok_or("Attachments need a home folder to be saved in.")?;
        let mut notes = Vec::new();
        for (index, attachment) in loaded.iter().enumerate() {
            let path = attachment.save(&dir)?;
            notes.push(attachments::note(attachment.label(), index + 1, &path));
        }
        if !self.draft.text.trim().is_empty() {
            self.draft.text.push('\n');
        }
        self.draft.text.push_str(&notes.join("\n"));
        self.composer.images.clear();
        Ok(loaded.len())
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
    fn a_dropped_screenshot_attaches_and_goes_out_as_a_saved_note_line() {
        let dir = tempfile::tempdir().unwrap();
        let cwd = dir.path().canonicalize().unwrap();
        std::fs::write(cwd.join("shot.png"), b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR").unwrap();
        let mut app = crate::App::default();
        app.set_mode(crate::Mode::Live);
        app.cwd = Some(cwd.clone());
        app.attachment_dir = Some(cwd.join("store").join("attachments"));
        // A terminal pastes a dropped file as its quoted path.
        app.handle(Event::Paste(format!(
            "'{}'",
            cwd.join("shot.png").display()
        )));
        assert_eq!(app.composer.images.len(), 1);
        assert!(app.draft.text.is_empty());
        assert!(
            crate::attachments::chip_for(&app.composer.images[0].source, 1)
                .starts_with("Image #1 shot.png")
        );
        app.draft.insert("what's wrong here");
        assert_eq!(app.attach_to_prompt().unwrap(), 1);
        assert!(app.composer.images.is_empty());
        assert!(
            app.draft.text.starts_with("what's wrong here\n[Image #1: "),
            "{}",
            app.draft.text
        );
        assert!(crate::attachments::mentions(&app.draft.text));
        assert!(crate::attachments::expand(&app.draft.text).is_some());
        // An @path in the prompt attaches too.
        app.draft = crate::Draft::default();
        app.draft.insert("and this one? @shot.png");
        assert_eq!(app.attach_to_prompt().unwrap(), 1);
        // Over the size limit: refused, with the reason.
        let mut big = b"\x89PNG\r\n\x1a\n".to_vec();
        big.resize(crate::attachments::MAX_IMAGE_BYTES + 1, 0);
        std::fs::write(cwd.join("big.png"), big).unwrap();
        app.handle(Event::Paste(cwd.join("big.png").display().to_string()));
        assert!(app.composer.images.is_empty());
        assert!(
            app.live
                .notice
                .as_deref()
                .unwrap_or_default()
                .contains("limit is 5 MB")
        );
        // Ordinary text that mentions a file name stays text.
        app.draft = crate::Draft::default();
        app.handle(Event::Paste("see shot.png for details".into()));
        assert!(app.composer.images.is_empty());
        assert!(app.draft.text.ends_with("see shot.png for details"));
    }
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
        // A real PNG header: a paste that is not an image is refused.
        app.handle(Event::Paste(
            "data:image/png;base64,iVBORw0KGgoAAAANSUhEUg==".into(),
        ));
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
