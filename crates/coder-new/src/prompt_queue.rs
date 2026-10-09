//! Accept prompts while busy and safely transfer pending work back to the composer.
use crate::{App, Draft, Mode, Screen, live};

pub type Inbox =
    std::sync::Arc<std::sync::Mutex<Vec<std::sync::Arc<std::sync::Mutex<Option<String>>>>>>;

pub(crate) struct Prompt {
    pub text: String,
    pub composer: crate::composer_state::ComposerState,
    pub editable: bool,
    slot: std::sync::Arc<std::sync::Mutex<Option<String>>>,
    session: Option<String>,
    pub agent: Option<String>,
}

impl App {
    pub(crate) fn queue_prompt(&mut self) {
        if self.draft.text.trim().is_empty() && self.composer.images.is_empty() {
            return;
        }
        self.record_prompt();
        let slot = std::sync::Arc::new(std::sync::Mutex::new(Some(self.draft.text.clone())));
        if self.live.busy
            && self.active_delegation.is_none()
            && self.selected_agent.is_none()
            && self.composer.mode == crate::composer_state::InputMode::Prompt
            && self.composer.images.is_empty()
            && !self.draft.text.trim().starts_with('/')
        {
            self.prompt_inbox.lock().unwrap().push(slot.clone());
        }
        self.queued_prompts.push(Prompt {
            slot,
            editable: true,
            composer: std::mem::take(&mut self.composer),
            text: std::mem::take(&mut self.draft.text),
            session: self.session_id().map(str::to_owned),
            agent: self
                .selected_agent
                .and_then(|i| self.delegations.get(i))
                .map(|a| a.id.clone()),
        });
        self.draft.cursor = 0;
        self.live.notice = None;
        if self.queue_hint_count < 3 {
            self.notice = Some("Press up to edit queued messages".into());
            self.queue_hint_count += 1;
        }
    }

    pub(crate) fn restore_queued_prompts(&mut self) -> bool {
        let agent = self
            .selected_agent
            .and_then(|i| self.delegations.get(i))
            .map(|a| a.id.clone());
        let session = self.session_id().map(str::to_owned);
        self.acknowledge_prompts();
        let cursor = self.draft.cursor;
        let mut text = Vec::new();
        let mut images = Vec::new();
        let mut pasted = Vec::new();
        let mut retrieved = false;
        self.queued_prompts.retain(|p| {
            if p.editable && p.agent == agent && p.session == session {
                // Taking the slot cancels execution atomically. If a worker won
                // the race, it owns the prompt and we must not restore it.
                if let Some(pending) = p.slot.lock().unwrap().take() {
                    retrieved = true;
                    images.extend(p.composer.images.clone());
                    pasted.extend(p.composer.pasted.clone());
                    if !pending.is_empty() {
                        text.push(pending);
                    }
                    false
                } else {
                    true
                }
            } else {
                true
            }
        });
        if !retrieved {
            return false;
        }
        let prefix = text.join("\n");
        let offset = prefix.len() + usize::from(!prefix.is_empty() && !self.draft.text.is_empty());
        if !self.draft.text.is_empty() {
            text.push(std::mem::take(&mut self.draft.text));
        }
        self.draft.text = text.join("\n");
        self.draft.cursor = (offset + cursor).min(self.draft.text.len());
        self.composer.images.extend(images);
        self.composer.pasted.extend(pasted);
        self.composer.mode = crate::composer_state::InputMode::Prompt;
        self.assign_image_ids();
        self.composer_history.reset();
        if self.notice.as_deref() == Some("Press up to edit queued messages") {
            self.notice = None;
        }
        true
    }

    pub(crate) fn acknowledge_prompts(&mut self) {
        let mut consumed = Vec::new();
        self.queued_prompts.retain(|p| {
            if p.slot.lock().unwrap().is_none() {
                consumed.push(p.text.clone());
                false
            } else {
                true
            }
        });
        if !consumed.is_empty() {
            for text in consumed {
                self.live.entries.push(live::Entry::User(text));
            }
            self.history.dirty = true;
        }
    }

    pub(crate) fn process_prompt_queue(&mut self) {
        self.acknowledge_prompts();
        if !self.queued_prompts.iter().any(|p| p.editable)
            && self.notice.as_deref() == Some("Press up to edit queued messages")
        {
            self.notice = None;
        }
        if self.mode != Mode::Live
            || self.live.busy
            || self.checking_key
            || self.checking_jev
            || self.brainstorm_job.is_some()
            || self.screen != Screen::Conversation
            || self.resume_picker.is_some()
            || self.model_picker.is_some()
        {
            return;
        }
        let session = self.session_id().map(str::to_owned);
        let Some(first) = self
            .queued_prompts
            .iter()
            .position(|p| p.editable && p.session == session)
        else {
            return;
        };
        let agent = self.queued_prompts[first].agent.clone();
        let index = match &agent {
            Some(id) => {
                let Some(i) = self.delegations.iter().position(|a| &a.id == id) else {
                    return;
                };
                Some(i)
            }
            None => None,
        };
        let slash = self.queued_prompts[first].text.trim().starts_with('/');
        let rich = self.queued_prompts[first].composer.clone();
        let single =
            slash || rich.mode == crate::composer_state::InputMode::Bash || !rich.images.is_empty();
        let mut batch = Vec::new();
        let mut position = 0;
        self.queued_prompts.retain(|p| {
            let take = if single {
                position == first
            } else {
                p.editable
                    && p.session == session
                    && p.agent == agent
                    && p.composer.mode == crate::composer_state::InputMode::Prompt
                    && p.composer.images.is_empty()
                    && !p.text.trim().starts_with('/')
            };
            position += 1;
            if take {
                batch.push(p.text.clone());
                false
            } else {
                true
            }
        });
        let selected = self.selected_agent;
        self.select_agent(index);
        let draft = std::mem::take(&mut self.draft);
        let composer = std::mem::replace(&mut self.composer, rich);
        let last = batch.pop().unwrap();
        // Each submission stays a distinct user message.
        for text in batch {
            let chat = if let Some(i) = index {
                &mut self.delegations[i].chat
            } else {
                &mut self.live
            };
            chat.entries.push(live::Entry::User(text));
        }
        self.draft = Draft {
            cursor: last.len(),
            text: last,
        };
        self.replaying_prompt = true;
        if self.composer.mode == crate::composer_state::InputMode::Bash {
            self.submit_bash();
        } else if slash {
            self.handle(crossterm::event::Event::Key(
                crossterm::event::KeyEvent::new(
                    crossterm::event::KeyCode::Enter,
                    crossterm::event::KeyModifiers::NONE,
                ),
            ));
        } else {
            self.submit_live();
        }
        self.replaying_prompt = false;
        if !self.draft.text.is_empty() || !self.composer.images.is_empty() {
            // A disabled agent or failed session admission must not discard input.
            self.queue_prompt();
        }
        self.draft = draft;
        self.composer = composer;
        self.select_agent(selected);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::live::{Update, Work};
    fn app() -> App {
        let mut app = App::default();
        app.set_mode(Mode::Live);
        app
    }
    #[test]
    fn accepts_and_batches_without_interrupting_or_overwriting_draft() {
        let mut app = app();
        app.submit("first", std::path::Path::new("."));
        let id = app.request.take().unwrap().id;
        app.submit("second", std::path::Path::new("."));
        app.submit("third", std::path::Path::new("."));
        assert_eq!(app.request_id, id);
        assert!(app.request.is_none());
        assert!(app.draft.text.is_empty());
        assert_eq!(app.queued_prompts.len(), 2);
        app.draft.text = "still typing".into();
        app.draft.cursor = 12;
        app.apply_update(Update::Finished {
            id,
            result: Err("fixture failure".into()),
        });
        app.process_prompt_queue();
        assert_eq!(app.draft.text, "still typing");
        assert!(app.queued_prompts.is_empty());
        let request = app.request.take().unwrap();
        match request.kind {
            Work::Microcoder { messages, .. } | Work::Chat { messages, .. } => {
                let texts: Vec<_> = messages
                    .iter()
                    .filter(|m| m.role == "user")
                    .map(|m| m.content.as_str())
                    .collect();
                assert_eq!(texts, ["first", "second", "third"]);
            }
            _ => panic!("chat request"),
        }
    }
    #[test]
    fn consumed_prompts_are_recorded_once_and_not_resubmitted() {
        let mut app = app();
        app.submit("first", std::path::Path::new("."));
        app.request.take();
        app.submit("next", std::path::Path::new("."));
        app.prompt_inbox.lock().unwrap()[0].lock().unwrap().take();
        app.acknowledge_prompts();
        app.acknowledge_prompts();
        assert!(app.queued_prompts.is_empty());
        assert_eq!(
            app.live
                .entries
                .iter()
                .filter(|entry| matches!(entry, live::Entry::User(text) if text == "next"))
                .count(),
            1
        );
        app.cancel_request();
        app.process_prompt_queue();
        assert!(app.request.is_none());
    }

    #[test]
    fn escape_restores_pending_input_without_stopping_active_work() {
        let mut app = app();
        app.submit("first", std::path::Path::new("."));
        app.submit("next", std::path::Path::new("."));
        let esc = || {
            crossterm::event::Event::Key(crossterm::event::KeyEvent::new(
                crossterm::event::KeyCode::Esc,
                crossterm::event::KeyModifiers::NONE,
            ))
        };
        app.handle(esc());
        assert!(app.live.busy);
        assert_eq!(app.draft.text, "next");
        assert!(app.queued_prompts.is_empty());
    }
    #[test]
    fn overlays_delay_processing_and_blank_input_is_ignored() {
        let mut app = app();
        app.submit("first", std::path::Path::new("."));
        app.submit(" ", std::path::Path::new("."));
        assert!(app.queued_prompts.is_empty());
        app.submit("next", std::path::Path::new("."));
        app.cancel_request();
        app.screen = Screen::Plugins;
        app.process_prompt_queue();
        assert_eq!(app.queued_prompts.len(), 1);
        app.screen = Screen::Conversation;
        app.process_prompt_queue();
        assert!(app.live.busy);
    }
    #[test]
    fn image_only_retrieval_keeps_identifiers_and_noneditable_messages() {
        let mut a = app();
        a.live.busy = true;
        a.composer.mode = crate::composer_state::InputMode::Bash;
        a.attach_image("embedded".into(), Some("queued-id".into()));
        a.queue_prompt();
        a.draft.text = "notification".into();
        a.queue_prompt();
        a.queued_prompts[1].editable = false;
        a.draft.text = "draft".into();
        a.draft.cursor = 2;
        a.attach_image("existing".into(), Some("draft-id".into()));
        assert!(a.restore_queued_prompts());
        assert_eq!(a.draft.text, "draft");
        assert_eq!(a.draft.cursor, 2);
        assert_eq!(a.composer.mode, crate::composer_state::InputMode::Prompt);
        assert_eq!(a.composer.images[0].id.as_deref(), Some("draft-id"));
        assert_eq!(a.composer.images[1].id.as_deref(), Some("queued-id"));
        assert_eq!(a.queued_prompts.len(), 1);
        assert!(!a.restore_queued_prompts());
    }
}
