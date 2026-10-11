//! Accept prompts while busy and safely transfer pending work back to the composer.
use crate::{App, Draft, Mode, Screen, live};

/// The discovery hint for retrieving queued messages (#11121).
pub(crate) const QUEUE_HINT: &str = "Press up to edit queued messages";
/// How many times the hint is shown in one run of Coder.
const QUEUE_HINT_SHOWS: u8 = 3;

pub type Inbox =
    std::sync::Arc<std::sync::Mutex<Vec<std::sync::Arc<std::sync::Mutex<Option<String>>>>>>;

pub(crate) struct Prompt {
    pub text: String,
    pub composer: crate::composer_state::ComposerState,
    pub editable: bool,
    /// A background agent's notice (#11163): never edited back into the
    /// composer, but it starts the next turn like a typed prompt.
    pub notice: bool,
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
            notice: false,
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
        // Limited exposure: one showing counts once, however many messages
        // are queued while it is up.
        if self.notice.as_deref() != Some(QUEUE_HINT) && self.queue_hint_count < QUEUE_HINT_SHOWS {
            self.notice = Some(QUEUE_HINT.into());
            self.queue_hint_count += 1;
        }
    }

    /// Queues a background agent's notice for the main chat: a turn in
    /// progress reads it after its current tool batch; otherwise it starts
    /// the next turn.
    pub(crate) fn queue_notice(&mut self, text: String) {
        let slot = std::sync::Arc::new(std::sync::Mutex::new(Some(text.clone())));
        if self.live.busy && self.active_delegation.is_none() {
            self.prompt_inbox.lock().unwrap().push(slot.clone());
        }
        self.queued_prompts.push(Prompt {
            slot,
            editable: false,
            notice: true,
            composer: Default::default(),
            text,
            session: self.session_id().map(str::to_owned),
            agent: None,
        });
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
        if self.notice.as_deref() == Some(QUEUE_HINT) {
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
        self.drain_prompt_queue(false);
    }

    pub(crate) fn send_next_queued_prompt(&mut self) -> bool {
        self.drain_prompt_queue(true)
    }

    fn drain_prompt_queue(&mut self, immediate: bool) -> bool {
        self.acknowledge_prompts();
        if !self.queued_prompts.iter().any(|p| p.editable)
            && self.notice.as_deref() == Some(QUEUE_HINT)
        {
            self.notice = None;
        }
        if self.mode != Mode::Live
            || (self.live.busy && !immediate)
            || self.checking_key
            || self.checking_jev
            || self.brainstorm_job.is_some()
            // A usage limit holds the queue until it resets (#11179).
            || self.long_session.paused()
            || self.screen != Screen::Conversation
            || self.resume_picker.is_some()
            || self.model_picker.is_some()
        {
            return false;
        }
        let session = self.session_id().map(str::to_owned);
        let selected_agent = self
            .selected_agent
            .and_then(|i| self.delegations.get(i))
            .map(|a| a.id.clone());
        if immediate {
            if !self.queued_prompts.iter().any(|p| {
                (p.editable || p.notice) && p.session == session && p.agent == selected_agent
            }) {
                return false;
            }
            // Detach the worker's inbox before choosing the next pending message.
            self.cancel_request();
        }
        let Some(first) = self.queued_prompts.iter().position(|p| {
            (p.editable || p.notice)
                && p.session == session
                && (!immediate || p.agent == selected_agent)
        }) else {
            return false;
        };
        let agent = self.queued_prompts[first].agent.clone();
        let index = match &agent {
            Some(id) => {
                let Some(i) = self.delegations.iter().position(|a| &a.id == id) else {
                    return false;
                };
                Some(i)
            }
            None => None,
        };
        let slash = self.queued_prompts[first].text.trim().starts_with('/');
        let rich = self.queued_prompts[first].composer.clone();
        let single = immediate
            || slash
            || rich.mode == crate::composer_state::InputMode::Bash
            || !rich.images.is_empty();
        let mut batch = Vec::new();
        let mut position = 0;
        self.queued_prompts.retain(|p| {
            let take = if single {
                position == first
            } else {
                (p.editable || p.notice)
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
        true
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
    fn enter_sends_only_next_prompt_and_ignores_old_reply_updates() {
        let mut app = app();
        app.submit("first", std::path::Path::new("."));
        let old_id = app.request.take().unwrap().id;
        app.submit("second", std::path::Path::new("."));
        app.submit("third", std::path::Path::new("."));
        app.handle(crossterm::event::Event::Key(
            crossterm::event::KeyEvent::new(
                crossterm::event::KeyCode::Enter,
                crossterm::event::KeyModifiers::NONE,
            ),
        ));
        assert_eq!(app.queued_prompts.len(), 1);
        assert_eq!(app.queued_prompts[0].text, "third");
        assert!(
            app.live
                .entries
                .iter()
                .any(|e| matches!(e, live::Entry::User(t) if t == "second"))
        );
        let request = app.request.take().unwrap();
        assert_ne!(request.id, old_id);
        match request.kind {
            Work::Microcoder { messages, .. } | Work::Chat { messages, .. } => {
                let texts: Vec<_> = messages
                    .iter()
                    .filter(|m| m.role == "user")
                    .map(|m| m.content.as_str())
                    .collect();
                assert_eq!(texts, ["first", "second"]);
            }
            _ => panic!("chat request"),
        }
        app.apply_update(Update::Finished {
            id: old_id,
            result: Err("old reply".into()),
        });
        assert!(app.live.busy);
        app.acknowledge_prompts();
        assert_eq!(
            app.live
                .entries
                .iter()
                .filter(|e| matches!(e, live::Entry::User(t) if t == "second"))
                .count(),
            1
        );
    }

    #[test]
    fn enter_with_draft_still_queues_and_alt_enter_inserts_newline() {
        let mut app = app();
        app.submit("first", std::path::Path::new("."));
        let id = app.request.take().unwrap().id;
        app.submit("second", std::path::Path::new("."));
        app.draft.text = "third".into();
        app.draft.cursor = 5;
        app.handle(crossterm::event::Event::Key(
            crossterm::event::KeyEvent::new(
                crossterm::event::KeyCode::Enter,
                crossterm::event::KeyModifiers::NONE,
            ),
        ));
        assert_eq!(app.queued_prompts.len(), 2);
        assert_eq!(app.request_id, id);
        app.handle(crossterm::event::Event::Key(
            crossterm::event::KeyEvent::new(
                crossterm::event::KeyCode::Enter,
                crossterm::event::KeyModifiers::ALT,
            ),
        ));
        assert_eq!(app.draft.text, "\n");
        assert_eq!(app.queued_prompts.len(), 2);
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
    #[test]
    fn queue_hint_shows_only_for_editable_messages_and_a_few_times() {
        let mut a = app();
        a.live.busy = true;
        a.queue_notice("an agent finished".into());
        assert_ne!(a.notice.as_deref(), Some(QUEUE_HINT));
        for round in 0..5 {
            a.draft.text = format!("queued {round}");
            a.queue_prompt();
            a.draft.text = format!("queued again {round}");
            a.queue_prompt();
            let shown = a.notice.as_deref() == Some(QUEUE_HINT);
            assert_eq!(
                shown,
                round < usize::from(QUEUE_HINT_SHOWS),
                "round {round}"
            );
            if shown {
                assert!(a.restore_queued_prompts());
                assert_ne!(a.notice.as_deref(), Some(QUEUE_HINT));
            } else {
                assert!(a.restore_queued_prompts());
            }
            a.draft = Draft::default();
        }
        assert_eq!(a.queue_hint_count, QUEUE_HINT_SHOWS);
        // The agent's notice is never retrieved for editing.
        assert!(a.queued_prompts.iter().all(|p| p.notice && !p.editable));
    }
}
