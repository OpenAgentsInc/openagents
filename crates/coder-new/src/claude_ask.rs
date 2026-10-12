//! Claude Code's `AskUserQuestion`, answered on a choice screen (#11449).
//!
//! The permission handler of a claude chat's process hands the question to
//! [`Asks`] and waits. The terminal sees it waiting, shows a [`Screen`]:
//! one question at a time, its choices, and an Other field to type in. The
//! person's answers go back as the tool's updated input, under `answers`,
//! keyed by each question's text, the way Claude Code reads them. Esc
//! declines, and Claude Code is told to ask in its reply instead.

use std::sync::{Arc, Mutex};

use serde_json::{Value, json};

/// One choice of a question.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Choice {
    pub label: String,
    pub description: String,
}

/// One question Claude Code asks.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Question {
    pub question: String,
    pub header: String,
    pub choices: Vec<Choice>,
    /// The person may pick several.
    pub multi: bool,
}

/// The questions of an `AskUserQuestion` input, or `None` when it names
/// none.
#[must_use]
pub fn questions(input: &Value) -> Option<Vec<Question>> {
    let text = |value: &Value, key: &str| value[key].as_str().unwrap_or_default().trim().to_owned();
    let questions: Vec<Question> = input["questions"]
        .as_array()?
        .iter()
        .map(|question| Question {
            question: text(question, "question"),
            header: text(question, "header"),
            choices: question["options"]
                .as_array()
                .map(|options| {
                    options
                        .iter()
                        .map(|option| Choice {
                            label: text(option, "label"),
                            description: text(option, "description"),
                        })
                        .filter(|choice| !choice.label.is_empty())
                        .collect()
                })
                .unwrap_or_default(),
            multi: question["multiSelect"].as_bool().unwrap_or(false),
        })
        .filter(|question| !question.question.is_empty())
        .collect();
    (!questions.is_empty()).then_some(questions)
}

/// The tool's input with the person's answers added: `answers` maps each
/// question's text to the answer, several picks joined with ", ".
#[must_use]
pub fn answered(input: &Value, questions: &[Question], answers: &[String]) -> Value {
    let mut input = input.clone();
    let map: serde_json::Map<String, Value> = questions
        .iter()
        .zip(answers)
        .map(|(question, answer)| (question.question.clone(), json!(answer)))
        .collect();
    if let Some(object) = input.as_object_mut() {
        object.insert("answers".into(), Value::Object(map));
    }
    input
}

struct Pending {
    id: u64,
    questions: Vec<Question>,
    reply: tokio::sync::oneshot::Sender<Option<Vec<String>>>,
}

/// The question one claude process is waiting on, shared by its
/// permission handler and the terminal.
#[derive(Clone, Default)]
pub struct Asks(Arc<Mutex<Option<Pending>>>);

/// Clears its own question when the handler stops waiting, answered or
/// not (a stopped turn drops it).
struct Waiting<'a>(&'a Asks, u64);

impl Drop for Waiting<'_> {
    fn drop(&mut self) {
        let mut slot = self.0.slot();
        if slot.as_ref().is_some_and(|pending| pending.id == self.1) {
            slot.take();
        }
    }
}

static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);

impl Asks {
    fn slot(&self) -> std::sync::MutexGuard<'_, Option<Pending>> {
        self.0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    /// Waits for the person's answers to `input`'s questions: the updated
    /// input, or `None` when they declined, the input names no question,
    /// or another question is already waiting.
    pub async fn ask(&self, input: &Value) -> Option<Value> {
        let questions = questions(input)?;
        let (reply, answer) = tokio::sync::oneshot::channel();
        let id = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        {
            let mut slot = self.slot();
            if slot.is_some() {
                return None;
            }
            *slot = Some(Pending {
                id,
                questions: questions.clone(),
                reply,
            });
        }
        let _waiting = Waiting(self, id);
        let answers = answer.await.ok().flatten()?;
        Some(answered(input, &questions, &answers))
    }

    /// The questions waiting on the person, if any.
    #[must_use]
    pub fn waiting(&self) -> Option<Vec<Question>> {
        self.slot()
            .as_ref()
            .map(|pending| pending.questions.clone())
    }

    /// Answers the waiting question: one answer per question, or `None` to
    /// decline.
    pub fn answer(&self, answers: Option<Vec<String>>) {
        if let Some(pending) = self.slot().take() {
            let _ = pending.reply.send(answers);
        }
    }
}

/// The choice screen for the waiting questions.
pub struct Screen {
    asks: Asks,
    pub questions: Vec<Question>,
    /// The question on screen.
    pub index: usize,
    /// The row under the cursor; the last row is Other.
    pub cursor: usize,
    /// The picks of a multi-select question.
    pub picked: Vec<bool>,
    /// What the person typed under Other.
    pub other: String,
    answers: Vec<String>,
}

/// What a key did to the screen.
#[derive(Debug, PartialEq, Eq)]
pub enum Outcome {
    /// Still asking.
    Asking,
    /// Every question answered, or the person declined: the screen closes.
    Done,
}

impl Screen {
    #[must_use]
    pub fn new(asks: Asks, questions: Vec<Question>) -> Self {
        let picked = vec![false; questions.first().map_or(0, |q| q.choices.len())];
        Self {
            asks,
            questions,
            index: 0,
            cursor: 0,
            picked,
            other: String::new(),
            answers: Vec::new(),
        }
    }

    /// Whether these are the questions still waiting.
    #[must_use]
    pub fn current(&self) -> bool {
        self.asks.waiting().as_ref() == Some(&self.questions)
    }

    /// The question on screen.
    #[must_use]
    pub fn question(&self) -> &Question {
        &self.questions[self.index]
    }

    /// The row index of Other.
    #[must_use]
    pub fn other_row(&self) -> usize {
        self.question().choices.len()
    }

    pub fn up(&mut self) {
        self.cursor = self.cursor.checked_sub(1).unwrap_or(self.other_row());
    }

    pub fn down(&mut self) {
        self.cursor = if self.cursor >= self.other_row() {
            0
        } else {
            self.cursor + 1
        };
    }

    /// A typed character goes to Other.
    pub fn type_char(&mut self, c: char) {
        self.cursor = self.other_row();
        self.other.push(c);
    }

    pub fn backspace(&mut self) {
        if self.cursor == self.other_row() {
            self.other.pop();
        }
    }

    /// Space: a multi-select question toggles the choice under the cursor;
    /// on Other it is a typed space.
    pub fn toggle(&mut self) {
        if self.cursor == self.other_row() {
            self.other.push(' ');
        } else if self.question().multi {
            self.picked[self.cursor] = !self.picked[self.cursor];
        }
    }

    /// Enter: answers the question on screen and moves to the next, or
    /// sends every answer after the last.
    pub fn enter(&mut self) -> Outcome {
        let question = self.question();
        let other = self.other.trim().to_owned();
        let answer = if question.multi {
            let mut picks: Vec<String> = question
                .choices
                .iter()
                .zip(&self.picked)
                .filter(|(_, picked)| **picked)
                .map(|(choice, _)| choice.label.clone())
                .collect();
            if picks.is_empty() && self.cursor < self.other_row() {
                picks.push(question.choices[self.cursor].label.clone());
            }
            if !other.is_empty() {
                picks.push(other);
            }
            picks.join(", ")
        } else if self.cursor < self.other_row() {
            question.choices[self.cursor].label.clone()
        } else {
            other
        };
        if answer.is_empty() {
            return Outcome::Asking;
        }
        self.answers.push(answer);
        self.index += 1;
        if self.index == self.questions.len() {
            self.asks.answer(Some(std::mem::take(&mut self.answers)));
            return Outcome::Done;
        }
        self.cursor = 0;
        self.other.clear();
        self.picked = vec![false; self.question().choices.len()];
        Outcome::Asking
    }

    /// Esc: Claude Code asks in its reply instead.
    pub fn decline(&mut self) -> Outcome {
        self.asks.answer(None);
        Outcome::Done
    }

    /// The screen's lines: the question, its choices, Other, and the keys.
    #[must_use]
    pub fn text(&self) -> String {
        let question = self.question();
        let mut text = String::new();
        if self.questions.len() > 1 {
            text.push_str(&format!(
                "Question {} of {}\n",
                self.index + 1,
                self.questions.len()
            ));
        }
        if !question.header.is_empty() {
            text.push_str(&format!("{}\n", question.header));
        }
        text.push_str(&format!("{}\n\n", question.question));
        for (row, choice) in question.choices.iter().enumerate() {
            let pointer = if row == self.cursor { "›" } else { " " };
            let mark = if question.multi {
                if self.picked[row] { "[x] " } else { "[ ] " }
            } else {
                ""
            };
            text.push_str(&format!("{pointer} {mark}{}", choice.label));
            if !choice.description.is_empty() {
                text.push_str(&format!(" — {}", choice.description));
            }
            text.push('\n');
        }
        let pointer = if self.cursor == self.other_row() {
            "›"
        } else {
            " "
        };
        text.push_str(&format!("{pointer} Other: {}\n\n", self.other));
        text.push_str(if question.multi {
            "↑↓ move · Space pick · type for Other · Enter send · Esc answer in chat"
        } else {
            "↑↓ move · type for Other · Enter choose · Esc answer in chat"
        });
        text
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn input() -> Value {
        json!({"questions":[
            {"question":"Which database?","header":"Storage","multiSelect":false,
             "options":[{"label":"Postgres","description":"The one we run"},{"label":"SQLite","description":"Local only"}]},
            {"question":"Which checks?","header":"Checks","multiSelect":true,
             "options":[{"label":"Tests","description":""},{"label":"Lint","description":""},{"label":"Docs","description":""}]}
        ]})
    }

    /// The person picks one, then several plus Other, and the answers go
    /// back in the updated input keyed by question (#11449).
    #[tokio::test]
    async fn answers_come_back_in_the_updated_input() {
        let asks = Asks::default();
        let waiting = {
            let asks = asks.clone();
            tokio::spawn(async move { asks.ask(&input()).await })
        };
        let questions = loop {
            if let Some(questions) = asks.waiting() {
                break questions;
            }
            tokio::task::yield_now().await;
        };
        let mut screen = Screen::new(asks.clone(), questions);
        assert!(screen.current());
        assert!(screen.text().contains("Which database?"));
        screen.down();
        assert_eq!(screen.enter(), Outcome::Asking);
        assert!(screen.text().contains("[ ] Tests"));
        screen.toggle();
        screen.down();
        screen.down();
        screen.toggle();
        for c in "Bench".chars() {
            screen.type_char(c);
        }
        assert_eq!(screen.enter(), Outcome::Done);
        let updated = waiting.await.unwrap().unwrap();
        assert_eq!(updated["answers"]["Which database?"], "SQLite");
        assert_eq!(updated["answers"]["Which checks?"], "Tests, Docs, Bench");
        assert_eq!(updated["questions"], input()["questions"]);
        assert!(asks.waiting().is_none());
    }

    /// Esc declines: the handler gets nothing and the question is gone.
    #[tokio::test]
    async fn declining_answers_nothing() {
        let asks = Asks::default();
        let waiting = {
            let asks = asks.clone();
            tokio::spawn(async move { asks.ask(&input()).await })
        };
        let questions = loop {
            if let Some(questions) = asks.waiting() {
                break questions;
            }
            tokio::task::yield_now().await;
        };
        let mut screen = Screen::new(asks.clone(), questions);
        assert_eq!(screen.decline(), Outcome::Done);
        assert!(waiting.await.unwrap().is_none());
        assert!(!screen.current());
    }

    /// An empty Other answers nothing; a stopped wait clears the question.
    #[tokio::test]
    async fn a_stopped_wait_clears_the_question() {
        let asks = Asks::default();
        let waiting = {
            let asks = asks.clone();
            tokio::spawn(async move { asks.ask(&input()).await })
        };
        while asks.waiting().is_none() {
            tokio::task::yield_now().await;
        }
        let mut screen = Screen::new(asks.clone(), asks.waiting().unwrap());
        screen.up();
        assert_eq!(screen.cursor, screen.other_row());
        assert_eq!(screen.enter(), Outcome::Asking);
        waiting.abort();
        let _ = waiting.await;
        assert!(asks.waiting().is_none());
        assert!(questions(&json!({"questions":[]})).is_none());
    }
}
