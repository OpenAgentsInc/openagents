use super::*;
use rust_native::{Activation, Axis, FieldChange, TextRole, ValidatedView};
use serde::{Deserialize, Serialize};

#[derive(Clone, Serialize, Deserialize)]
#[serde(tag = "action", rename_all = "snake_case", deny_unknown_fields)]
enum Action {
    Choose { question: usize, option: usize },
    Write { question: usize },
    Submit,
}
#[derive(Default)]
struct Answer {
    selected: Vec<usize>,
    free: String,
}
#[derive(Default)]
pub(super) struct Questions {
    request: Option<InputRequest>,
    answers: Vec<Answer>,
    sent: bool,
    pending: Option<ValidatedView<Action>>,
    applied: Option<ValidatedView<Action>>,
}
impl Questions {
    fn reconcile(&mut self, request: Option<InputRequest>) -> CoreResult<()> {
        if self.request != request {
            self.answers.clear();
            self.request = None;
            self.pending = None;
            self.applied = None;
            self.sent = false;
            if let Some(request) = request {
                if request.questions.is_empty()
                    || request.questions.len() > 8
                    || request.request_id.is_empty()
                    || request.request_id.len() > 256
                    || request.questions.iter().any(|q| {
                        q.id.is_empty()
                            || q.id.len() > 256
                            || q.options.len() > 16
                            || q.question.len() > 8_192
                            || q.header.len() > 512
                            || q.options.iter().any(|v| v.len() > 2_048)
                            || q.prefill.as_ref().is_some_and(|v| v.len() > 4_000)
                    })
                    || request
                        .questions
                        .iter()
                        .enumerate()
                        .any(|(i, q)| request.questions[..i].iter().any(|p| p.id == q.id))
                {
                    return Err(CoreError::Closed);
                }
                self.answers = request
                    .questions
                    .iter()
                    .map(|q| {
                        let prefill = q.prefill.clone().unwrap_or_default();
                        match q.options.iter().position(|label| *label == prefill) {
                            Some(option) => Answer {
                                selected: vec![option],
                                free: String::new(),
                            },
                            None => Answer {
                                selected: vec![],
                                free: prefill,
                            },
                        }
                    })
                    .collect();
                self.request = Some(request);
            }
        }
        Ok(())
    }
    pub(super) fn frame(
        &mut self,
        instance: &str,
        revision: u64,
        request: Option<InputRequest>,
    ) -> CoreResult<Option<String>> {
        self.reconcile(request)?;
        self.pending = None;
        self.applied = None;
        let Some(request) = &self.request else {
            return Ok(None);
        };
        let mut children = vec![];
        for (index, (question, answer)) in request.questions.iter().zip(&self.answers).enumerate() {
            children.push(node(
                format!("header-{index}"),
                Element::Text {
                    value: question.header.clone(),
                    role: TextRole::Heading,
                },
            ));
            children.push(node(
                format!("question-{index}"),
                Element::Text {
                    value: question.question.clone(),
                    role: TextRole::Body,
                },
            ));
            for (option, label) in question.options.iter().enumerate() {
                children.push(node(
                    format!("choice-{index}-{option}"),
                    Element::Choice {
                        label: label.clone(),
                        selected: answer.selected.contains(&option),
                        enabled: !self.sent,
                        intent: Action::Choose {
                            question: index,
                            option,
                        },
                        children: vec![],
                    },
                ));
            }
            children.push(node(
                format!("answer-{index}"),
                Element::Field {
                    label: "Your answer".into(),
                    value: answer.free.clone(),
                    placeholder: String::new(),
                    secret: false,
                    multiline: question.multiline,
                    enabled: !self.sent,
                    max_bytes: 4_000,
                    on_change: Action::Write { question: index },
                },
            ));
        }
        children.push(node(
            "submit",
            Element::Button {
                label: "Send answer".into(),
                enabled: !self.sent && self.complete(),
                icon: None,
                shortcut: None,
                intent: Action::Submit,
            },
        ));
        let view = View::new_v3(
            format!("{instance}-questions"),
            revision,
            node(
                "root",
                Element::Stack {
                    axis: Axis::Vertical,
                    children,
                },
            ),
        )
        .validate()
        .map_err(|_| CoreError::Closed)?;
        let json = String::from_utf8(view.to_json().map_err(|_| CoreError::Closed)?)
            .map_err(|_| CoreError::Closed)?;
        self.pending = Some(view);
        Ok(Some(json))
    }
    fn complete(&self) -> bool {
        !self.answers.is_empty()
            && self
                .answers
                .iter()
                .all(|a| !a.free.trim().is_empty() || !a.selected.is_empty())
    }
    fn current(&self, request: Option<InputRequest>) -> CoreResult<()> {
        if self.sent || self.request.is_none() || self.request != request {
            Err(CoreError::Closed)
        } else {
            Ok(())
        }
    }
    pub(super) fn acknowledge(&mut self, instance: &str, revision: u64) -> CoreResult<()> {
        let view = self.pending.as_ref().ok_or(CoreError::Closed)?;
        if view.view().instance != instance || view.view().revision != revision {
            return Err(CoreError::Closed);
        }
        self.applied = self.pending.take();
        Ok(())
    }
    pub(super) fn change(
        &mut self,
        instance: String,
        revision: u64,
        key: String,
        value: String,
        request: Option<InputRequest>,
    ) -> CoreResult<()> {
        self.current(request)?;
        let action = self
            .applied
            .as_ref()
            .ok_or(CoreError::Closed)?
            .change_field(&FieldChange {
                activation: Activation {
                    instance,
                    revision,
                    node: key,
                },
                value: value.clone(),
            })
            .map_err(|_| CoreError::Closed)?
            .clone();
        let Action::Write { question } = action else {
            return Err(CoreError::Closed);
        };
        let answer = self.answers.get_mut(question).ok_or(CoreError::Closed)?;
        answer.free = value;
        answer.selected.clear();
        self.applied = None;
        Ok(())
    }
    pub(super) fn activate(
        &mut self,
        instance: String,
        revision: u64,
        key: String,
        request: Option<InputRequest>,
    ) -> CoreResult<Option<(String, Vec<UserInputAnswer>)>> {
        self.current(request)?;
        let action = self
            .applied
            .as_ref()
            .ok_or(CoreError::Closed)?
            .activate(&Activation {
                instance,
                revision,
                node: key,
            })
            .map_err(|_| CoreError::Closed)?
            .clone();
        let request = self.request.as_ref().ok_or(CoreError::Closed)?;
        match action {
            Action::Choose { question, option } => {
                let q = request.questions.get(question).ok_or(CoreError::Closed)?;
                if option >= q.options.len() {
                    return Err(CoreError::Closed);
                }
                let answer = self.answers.get_mut(question).ok_or(CoreError::Closed)?;
                answer.free.clear();
                if q.multi_select {
                    if let Some(index) = answer.selected.iter().position(|value| *value == option) {
                        answer.selected.remove(index);
                    } else {
                        answer.selected.push(option);
                        answer.selected.sort_unstable();
                    }
                } else {
                    answer.selected = vec![option];
                }
                self.applied = None;
                Ok(None)
            }
            Action::Submit if self.complete() => {
                let answers = request
                    .questions
                    .iter()
                    .zip(&self.answers)
                    .map(|(q, a)| UserInputAnswer {
                        question_id: q.id.clone(),
                        labels: if a.free.trim().is_empty() {
                            a.selected.iter().map(|i| q.options[*i].clone()).collect()
                        } else {
                            vec![a.free.clone()]
                        },
                    })
                    .collect();
                Ok(Some((request.request_id.clone(), answers)))
            }
            _ => Err(CoreError::Closed),
        }
    }
    pub(super) fn submitted(&mut self) {
        self.sent = true;
        self.applied = None;
        self.pending = None;
    }
}
fn node(key: impl Into<String>, element: Element<Action>) -> Node<Action> {
    Node {
        key: key.into(),
        style: Default::default(),
        element,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn request() -> InputRequest {
        InputRequest {
            entry_id: "entry".into(),
            request_id: "request".into(),
            questions: vec![UserInputQuestion {
                id: "permission".into(),
                header: "Permission".into(),
                question: "Allow this action?".into(),
                options: vec!["Allow once".into(), "Deny".into()],
                multi_select: false,
                prefill: None,
                multiline: false,
            }],
        }
    }
    fn mount(model: &mut Questions, revision: u64, request: &InputRequest) {
        model
            .frame("editor", revision, Some(request.clone()))
            .unwrap()
            .unwrap();
        model.acknowledge("editor-questions", revision).unwrap();
    }
    #[test]
    fn approval_uses_only_the_selected_rust_option_and_original_request() {
        let mut model = Questions::default();
        let request = request();
        mount(&mut model, 1, &request);
        assert!(
            model
                .activate(
                    "editor-questions".into(),
                    1,
                    "choice-0-1".into(),
                    Some(request.clone())
                )
                .unwrap()
                .is_none()
        );
        mount(&mut model, 2, &request);
        let (id, answers) = model
            .activate(
                "editor-questions".into(),
                2,
                "submit".into(),
                Some(request.clone()),
            )
            .unwrap()
            .unwrap();
        assert_eq!(id, "request");
        assert_eq!(
            answers,
            vec![UserInputAnswer {
                question_id: "permission".into(),
                labels: vec!["Deny".into()]
            }]
        );
        model.submitted();
        mount(&mut model, 3, &request);
        assert!(
            model
                .activate("editor-questions".into(), 3, "submit".into(), Some(request))
                .is_err()
        );
    }
    #[test]
    fn changed_or_resolved_request_rejects_old_controls() {
        let mut model = Questions::default();
        let request = request();
        mount(&mut model, 1, &request);
        let mut replacement = request.clone();
        replacement.questions[0].options.reverse();
        assert!(
            model
                .activate(
                    "editor-questions".into(),
                    1,
                    "choice-0-0".into(),
                    Some(replacement)
                )
                .is_err()
        );
        assert!(
            model
                .activate("editor-questions".into(), 1, "choice-0-0".into(), None)
                .is_err()
        );
        assert!(model.answers[0].selected.is_empty());
    }
    #[test]
    fn free_answer_obeys_current_field_byte_limit_and_revision() {
        let mut model = Questions::default();
        let request = request();
        mount(&mut model, 1, &request);
        assert!(
            model
                .change(
                    "editor-questions".into(),
                    0,
                    "answer-0".into(),
                    "old".into(),
                    Some(request.clone())
                )
                .is_err()
        );
        assert!(
            model
                .change(
                    "editor-questions".into(),
                    1,
                    "answer-0".into(),
                    "x".repeat(4_001),
                    Some(request.clone())
                )
                .is_err()
        );
        model
            .change(
                "editor-questions".into(),
                1,
                "answer-0".into(),
                "A bounded answer".into(),
                Some(request.clone()),
            )
            .unwrap();
        mount(&mut model, 2, &request);
        let (_, answers) = model
            .activate("editor-questions".into(), 2, "submit".into(), Some(request))
            .unwrap()
            .unwrap();
        assert_eq!(answers[0].labels, vec!["A bounded answer"]);
    }
    #[test]
    fn multiple_choices_keep_server_order_and_reset_on_replacement() {
        let mut model = Questions::default();
        let mut request = request();
        request.questions[0].multi_select = true;
        mount(&mut model, 1, &request);
        model
            .activate(
                "editor-questions".into(),
                1,
                "choice-0-1".into(),
                Some(request.clone()),
            )
            .unwrap();
        mount(&mut model, 2, &request);
        model
            .activate(
                "editor-questions".into(),
                2,
                "choice-0-0".into(),
                Some(request.clone()),
            )
            .unwrap();
        mount(&mut model, 3, &request);
        let (_, answers) = model
            .activate(
                "editor-questions".into(),
                3,
                "submit".into(),
                Some(request.clone()),
            )
            .unwrap()
            .unwrap();
        assert_eq!(answers[0].labels, vec!["Allow once", "Deny"]);
        request.request_id = "replacement".into();
        mount(&mut model, 4, &request);
        assert!(!model.complete());
    }
}
