//! Presentation of requests offered by the current task owner.

use crate::decision::{Control, Flow, Prompt, Risk, Step};
use route_contract::{Digest, pending as wire};

#[derive(Debug, Default)]
pub(super) struct Pending {
    snapshot: Option<wire::Snapshot>,
    current: Option<(wire::Request, Flow)>,
    available: bool,
    answered: std::collections::BTreeSet<Digest>,
}

impl Pending {
    pub fn update(
        &mut self,
        snapshot: Option<wire::Snapshot>,
        task: &str,
        state: super::State,
    ) -> bool {
        if state != super::State::Running {
            let changed = self.available || self.current.is_some();
            self.available = false;
            self.current = None;
            return changed;
        }
        let before = (self.snapshot.clone(), self.available);
        if let Some(candidate) = &snapshot
            && let Some(old) = &self.snapshot
            && candidate.valid()
            && candidate.scope.task == task
            && candidate.scope.host == super::LOCAL
        {
            let version = |s: &wire::Snapshot| (s.scope.turn, s.scope.generation, s.revision);
            if version(candidate) < version(old)
                || (version(candidate) == version(old) && candidate != old)
            {
                return false;
            }
        }
        self.available = false;
        if let Some(snapshot) =
            snapshot.filter(|s| s.valid() && s.scope.task == task && s.scope.host == super::LOCAL)
        {
            let version = |s: &wire::Snapshot| (s.scope.turn, s.scope.generation, s.revision);
            if self
                .snapshot
                .as_ref()
                .is_none_or(|old| version(&snapshot) >= version(old))
            {
                self.available = snapshot.active && state == super::State::Running;
                if self
                    .snapshot
                    .as_ref()
                    .is_some_and(|old| old.scope != snapshot.scope)
                {
                    self.answered.clear();
                }
                let next = snapshot
                    .entries
                    .iter()
                    .find(|e| e.resolution.is_none() && !self.answered.contains(&e.request.id));
                if !self.available {
                    self.current = None;
                } else if let Some(entry) = next {
                    if self
                        .current
                        .as_ref()
                        .is_none_or(|(r, _)| r.id != entry.request.id)
                    {
                        let request = entry.request.clone();
                        let flow = match &request.form {
                            wire::Form::Approval => Flow::approval_step(
                                "May Coder go ahead?",
                                Prompt {
                                    tool: request.tool.clone(),
                                    command: request.prompt.command.clone(),
                                    cwd: request.authority.checkout.clone(),
                                    reason: request.prompt.reason.clone(),
                                    risk: match request.prompt.risk {
                                        wire::Risk::Low => Risk::Low,
                                        wire::Risk::Medium => Risk::Medium,
                                        wire::Risk::High => Risk::High,
                                    },
                                    always: None,
                                },
                            ),
                            wire::Form::Questions { form } => {
                                Flow::structured(form).expect("validated form")
                            }
                        };
                        self.current = Some((request, flow));
                    }
                } else {
                    self.current = None;
                }
                self.snapshot = Some(snapshot);
            }
        }
        before != (self.snapshot.clone(), self.available)
    }

    pub fn request(&self) -> Option<&wire::Request> {
        self.current.as_ref().map(|(r, _)| r)
    }
    pub fn flow(&self) -> Option<&Flow> {
        self.current.as_ref().map(|(_, f)| f)
    }
    pub fn ready(&self) -> bool {
        self.available && self.current.is_some()
    }
    pub fn accepts(&self, reply: &wire::Reply) -> bool {
        self.ready() && self.request().is_some_and(|r| r.accepts(reply))
    }
    pub fn reply(&self, id: &Digest, answer: wire::Answer) -> Option<wire::Reply> {
        let request = self.request().filter(|r| &r.id == id)?;
        let reply = wire::Reply {
            request: id.clone(),
            scope: request.scope.clone(),
            input_digest: request.input_digest.clone(),
            answer,
        };
        self.accepts(&reply).then_some(reply)
    }
    pub fn control(&mut self, id: &Digest, control: Control) -> Option<wire::Reply> {
        if !self.ready() || &self.request()?.id != id {
            return None;
        }
        let (_, flow) = self.current.as_mut()?;
        if !matches!(flow.control(control), Step::Done(_)) {
            return None;
        }
        let answer = match flow.kind() {
            crate::decision::Kind::Approval => {
                if flow.answer() == crate::decision::ALLOWED {
                    wire::Answer::AllowOnce
                } else {
                    wire::Answer::Deny
                }
            }
            crate::decision::Kind::Question => flow.structured_answer()?,
        };
        self.reply(id, answer)
    }
    pub fn typed(&mut self, text: &str) -> Option<wire::Reply> {
        if !self.ready() {
            return None;
        }
        let (request, flow) = self.current.as_mut()?;
        if !matches!(request.form, wire::Form::Questions { .. }) {
            return None;
        }
        if !matches!(flow.answer_typed(text), Step::Done(_)) {
            return None;
        }
        let id = request.id.clone();
        let answer = flow.structured_answer()?;
        self.reply(&id, answer)
    }
    pub fn ack(&mut self, id: &Digest) {
        self.answered.insert(id.clone());
        if self.request().is_some_and(|r| &r.id == id) {
            self.current = None;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use route_contract::{digest_of, observation::Scope};
    use serde_json::json;

    fn snapshot(generation: u64) -> wire::Snapshot {
        let scope = Scope {
            host: super::super::LOCAL.into(),
            task: "task".into(),
            turn: 1,
            generation,
        };
        let mut request = wire::Request {
            schema: wire::SCHEMA.into(),
            id: digest_of(&()),
            scope: scope.clone(),
            authority: wire::Authority {
                intent: "intent".into(),
                grant: "grant".into(),
                checkout: "/project".into(),
                target: "target".into(),
            },
            native_call: "tool".into(),
            child: None,
            tool: "Bash".into(),
            input: json!({"command":"make check"}),
            input_digest: digest_of(&()),
            prompt: wire::Prompt {
                command: "make check".into(),
                reason: "Run checks".into(),
                risk: wire::Risk::High,
            },
            form: wire::Form::Approval,
            expires_at: 100,
        };
        request.seal();
        wire::Snapshot {
            schema: wire::SNAPSHOT_SCHEMA.into(),
            scope,
            revision: 1,
            active: true,
            entries: vec![wire::Entry {
                request,
                resolution: None,
                answer: None,
            }],
        }
    }

    #[test]
    fn reconnect_retains_selection_but_stale_request_and_old_generation_cannot_answer() {
        let mut pending = Pending::default();
        let first = snapshot(1);
        let id = first.entries[0].request.id.clone();
        assert!(pending.update(Some(first.clone()), "task", super::super::State::Running));
        assert!(pending.control(&id, Control::Pick(0)).is_some());
        pending.update(None, "task", super::super::State::Running);
        assert!(!pending.ready());
        assert!(pending.control(&id, Control::Pick(0)).is_none());
        pending.update(Some(first), "task", super::super::State::Running);
        assert!(pending.ready());
        pending.update(Some(snapshot(2)), "task", super::super::State::Running);
        assert!(pending.control(&id, Control::Pick(0)).is_none());
        let latest = pending.request().unwrap().id.clone();
        pending.update(Some(snapshot(1)), "task", super::super::State::Running);
        assert_eq!(pending.request().unwrap().id, latest);
        pending.update(Some(snapshot(1)), "task", super::super::State::Ended);
        assert!(pending.request().is_none());
    }

    #[test]
    fn run_submits_a_bound_reply_without_continuing_or_restarting_the_task() {
        use super::super::{Action, Answer, Request, Run, State};
        let now = std::time::Instant::now();
        let mut run = Run::follow("chat", "task", None, now);
        let (ticket, _) = run.tick(now).unwrap();
        let snapshot = snapshot(1);
        let id = snapshot.entries[0].request.id.clone();
        run.outcome(
            ticket,
            Ok(Answer::Lines {
                configuration: None,
                lines: vec![],
                state: State::Running,
                native: Some(snapshot.clone()),
            }),
            now,
        );
        let (ticket, request) = run
            .action(
                Action::NativeDecision {
                    request: id.clone(),
                    control: Control::Pick(0),
                },
                "",
            )
            .unwrap();
        let Request::Respond { task, reply } = request else {
            panic!("a live answer is not a continuation")
        };
        assert_eq!(task, "task");
        assert_eq!(reply.answer, wire::Answer::AllowOnce);
        run.outcome(
            ticket,
            Ok(Answer::Responded(wire::Receipt {
                request: id.clone(),
                resolution: wire::Resolution::Answered,
                duplicate: false,
            })),
            now,
        );
        assert!(
            run.action(
                Action::NativeDecision {
                    request: id,
                    control: Control::Pick(0)
                },
                ""
            )
            .is_none()
        );
        assert_eq!(run.state(), State::Running);
    }

    #[test]
    fn acknowledgment_does_not_reopen_an_old_snapshot_or_answer_another_request() {
        let mut pending = Pending::default();
        let snapshot = snapshot(1);
        pending.update(Some(snapshot.clone()), "task", super::super::State::Running);
        let id = pending.request().unwrap().id.clone();
        pending.ack(&id);
        pending.update(Some(snapshot), "task", super::super::State::Running);
        assert!(pending.request().is_none());
        pending.update(
            Some(self::snapshot(2)),
            "other-task",
            super::super::State::Running,
        );
        assert!(pending.request().is_none());
    }

    #[test]
    fn typed_questions_select_multiple_ids_and_require_every_answer() {
        let form = wire::Questionnaire {
            questions: vec![
                wire::Question {
                    id: "targets".into(),
                    text: "Which targets?".into(),
                    choices: vec![
                        wire::Choice {
                            id: "linux".into(),
                            label: "Linux".into(),
                        },
                        wire::Choice {
                            id: "mac".into(),
                            label: "Mac".into(),
                        },
                    ],
                    multiple: true,
                    free_text: false,
                },
                wire::Question {
                    id: "notes".into(),
                    text: "Any notes?".into(),
                    choices: vec![],
                    multiple: false,
                    free_text: true,
                },
            ],
        };
        let mut flow = Flow::structured(&form).unwrap();
        assert_eq!(flow.advance(), Step::Stay);
        assert_eq!(flow.select(0), Step::Moved);
        assert_eq!(flow.select(1), Step::Moved);
        assert_eq!(flow.answer_typed("not permitted"), Step::Stay);
        assert_eq!(flow.advance(), Step::Moved);
        assert!(flow.structured_answer().is_none());
        assert!(matches!(flow.answer_typed("Ship both"), Step::Done(_)));
        let wire::Answer::Questions { answers } = flow.structured_answer().unwrap() else {
            panic!()
        };
        assert_eq!(answers[0].choices, ["linux", "mac"]);
        assert_eq!(answers[1].text.as_deref(), Some("Ship both"));
    }
}
