//! The question wizard's state, shared by the desktop and the web (#12216):
//! which of an agent's questions waits for an answer, and the paged answers
//! the person gives before they go back as `RespondInput`.

use openagents_doc::{MessagePart, MessageRole, SessionMessageEntry};
use openagents_proto::{UserInputAnswer, UserInputQuestion};

/// Single-select questions auto-advance after this long.
pub const AUTO_ADVANCE_MS: u64 = 220;

/// Find the unresolved input request the panel should serve, if any: an
/// unresolved input part on the LAST assistant entry — regardless of the
/// entry's run status. The question stays answerable until the user actually
/// answers it (user requirement): a run that died under its question (engine
/// restart reaping it) leaves an aborted entry whose answer the engine
/// delivers as a resumed turn (`RespondInput`'s dead-run fallback). A newer
/// assistant entry supersedes an unanswered question. Assistant-entry-scoped,
/// not last-entry: a steer prompt sent while the agent waits appends a USER
/// entry after the streaming assistant entry, and a last-entry-only read made
/// the QuestionPanel vanish exactly when the user typed (earlier forensics;
/// matches the original composer.tsx, which reads the live-assistant fold —
/// rebuilt from replay even after the run died).
pub fn pending_input_request(
    transcript: &[SessionMessageEntry],
) -> Option<(String, Vec<UserInputQuestion>)> {
    transcript
        .iter()
        .rev()
        .find(|entry| entry.role == MessageRole::Assistant)
        .and_then(|entry| {
            entry.parts.iter().find_map(|part| match part {
                MessagePart::Input {
                    request_id,
                    questions,
                    resolved: false,
                    ..
                } => Some((request_id.clone(), questions.clone())),
                _ => None,
            })
        })
}

/// Whether the transcript shows `request_id` explicitly resolved (here or on
/// another device) — the wizard latch's release condition.
pub fn input_request_resolved(transcript: &[SessionMessageEntry], request_id: &str) -> bool {
    transcript.iter().any(|entry| {
        entry.parts.iter().any(|part| {
            matches!(
                part,
                MessagePart::Input {
                    request_id: rid,
                    resolved: true,
                    ..
                } if rid == request_id
            )
        })
    })
}

// ---------------------------------------------------------------------------
// Question wizard (pure reducer)
// ---------------------------------------------------------------------------

/// Reducer outcome of a wizard interaction.
#[derive(Debug, Clone, PartialEq)]
pub enum WizardStep {
    Stay,
    /// Single-select landed — advance after [`AUTO_ADVANCE_MS`].
    AutoAdvance,
    /// All pages answered — submit these answers.
    Done(Vec<UserInputAnswer>),
}

/// Paged question state ("1/3"): single-select auto-advances, multi-select and
/// typed answers advance explicitly, number keys 1-9 select, Back pages back.
#[derive(Debug, Clone)]
pub struct Wizard {
    pub request_id: String,
    pub questions: Vec<UserInputQuestion>,
    pub page: usize,
    picked: Vec<Vec<usize>>,
    typed: Vec<String>,
}

impl Wizard {
    pub fn new(request_id: String, questions: Vec<UserInputQuestion>) -> Self {
        let n = questions.len();
        let typed = questions
            .iter()
            .map(|q| q.prefill.clone().unwrap_or_default())
            .collect();
        Self {
            request_id,
            questions,
            page: 0,
            picked: vec![Vec::new(); n],
            typed,
        }
    }

    pub fn counter(&self) -> String {
        format!("{}/{}", self.page + 1, self.questions.len().max(1))
    }

    pub fn current(&self) -> Option<&UserInputQuestion> {
        self.questions.get(self.page)
    }

    pub fn is_picked(&self, option_ix: usize) -> bool {
        self.picked
            .get(self.page)
            .is_some_and(|p| p.contains(&option_ix))
    }

    /// Whether the current page has any picked option.
    pub fn page_has_pick(&self) -> bool {
        self.picked.get(self.page).is_some_and(|p| !p.is_empty())
    }

    /// Click/tap an option.
    pub fn select(&mut self, option_ix: usize) -> WizardStep {
        let Some(question) = self.questions.get(self.page) else {
            return WizardStep::Stay;
        };
        if option_ix >= question.options.len() {
            return WizardStep::Stay;
        }
        let multi = question.multi_select;
        let Some(picked) = self.picked.get_mut(self.page) else {
            return WizardStep::Stay;
        };
        if multi {
            match picked.iter().position(|&p| p == option_ix) {
                Some(at) => {
                    picked.remove(at);
                }
                None => picked.push(option_ix),
            }
            WizardStep::Stay
        } else {
            *picked = vec![option_ix];
            WizardStep::AutoAdvance
        }
    }

    /// Number key 1-9.
    pub fn press_number(&mut self, number: usize) -> WizardStep {
        if number == 0 {
            return WizardStep::Stay;
        }
        self.select(number - 1)
    }

    /// What the person typed on the current page.
    pub fn typed(&self) -> &str {
        self.typed.get(self.page).map_or("", String::as_str)
    }

    pub fn set_typed(&mut self, text: String) {
        if let Some(slot) = self.typed.get_mut(self.page) {
            *slot = text;
        }
    }

    /// Explicit submit / auto-advance landing.
    pub fn advance(&mut self) -> WizardStep {
        if self.page + 1 < self.questions.len() {
            self.page += 1;
            WizardStep::Stay
        } else {
            WizardStep::Done(self.answers())
        }
    }

    /// Page back; false when already on the first page.
    pub fn back(&mut self) -> bool {
        if self.page > 0 {
            self.page -= 1;
            true
        } else {
            false
        }
    }

    /// Answers per question: free text overrides picked labels.
    pub fn answers(&self) -> Vec<UserInputAnswer> {
        self.questions
            .iter()
            .enumerate()
            .map(|(ix, q)| {
                let typed = self
                    .typed
                    .get(ix)
                    .map(|s| if q.multiline { s.as_str() } else { s.trim() })
                    .unwrap_or("");
                let labels = if !typed.is_empty() || q.multiline {
                    vec![typed.to_string()]
                } else {
                    self.picked
                        .get(ix)
                        .map(|picked| {
                            picked
                                .iter()
                                .filter_map(|&p| q.options.get(p).cloned())
                                .collect()
                        })
                        .unwrap_or_default()
                };
                UserInputAnswer {
                    question_id: q.id.clone(),
                    labels,
                }
            })
            .collect()
    }
}
