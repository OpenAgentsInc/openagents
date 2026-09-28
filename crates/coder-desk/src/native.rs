//! The native backend: the desk protocol as it is.
//!
//! One request is one JSON object on one line, the answer is one more, and
//! the connection closes. Nothing is translated, so a desk that answers
//! this backend answers the contract.

use crate::{
    Backend, Codec, DeskError, DeskRow, GENERATION, Reading, Reply, Request, Screen, Verb, Window,
};

/// The backend for a desk that speaks the protocol itself.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Native;

impl Backend for Native {
    fn name(&self) -> &'static str {
        "native"
    }

    fn codec(&self) -> Codec {
        Codec::Line
    }

    fn requests(&self, verb: &Verb) -> Result<Vec<String>, DeskError> {
        let request = Request::new(verb.clone());
        match serde_json::to_string(&request) {
            Ok(line) => Ok(vec![line]),
            Err(error) => Err(DeskError::Asked(format!(
                "this request did not encode: {error}"
            ))),
        }
    }

    fn reply(&self, _verb: &Verb, answers: &[String]) -> Result<Reply, DeskError> {
        read_answer(answers.first().map(String::as_str).unwrap_or_default())
    }

    fn reading_requests(&self) -> Vec<String> {
        [Verb::Screens, Verb::Focused, Verb::List]
            .into_iter()
            .filter_map(|verb| serde_json::to_string(&Request::new(verb)).ok())
            .collect()
    }

    fn reading(&self, answers: &[String]) -> Result<Reading, DeskError> {
        let mut answers = answers.iter();
        let screens = match read_answer(next(&mut answers))? {
            Reply::Screens { screens } => screens,
            _ => Vec::new(),
        };
        let focused = match read_answer(next(&mut answers))? {
            Reply::Focused { window } => window.map(|window| window.screen),
            _ => None,
        };
        let windows = match read_answer(next(&mut answers))? {
            Reply::Windows { windows } => windows,
            _ => Vec::new(),
        };
        Ok(Reading {
            desks: desks(&screens, &windows),
            screens,
            focused: focused.filter(|name| !name.is_empty()),
        })
    }
}

/// The next answer, or an empty one.
fn next<'a>(answers: &mut impl Iterator<Item = &'a String>) -> &'a str {
    answers.next().map(String::as_str).unwrap_or_default()
}

/// One answer line, read as the reply it carries.
fn read_answer(line: &str) -> Result<Reply, DeskError> {
    let answer: crate::Answer = serde_json::from_str(line.trim()).map_err(|error| {
        DeskError::Unreadable(format!(
            "the desk answered something this client cannot read: {error}"
        ))
    })?;
    if answer.generation != GENERATION && !matches!(answer.reply, Reply::Refused(_)) {
        return Err(DeskError::Unreadable(format!(
            "this client speaks generation {GENERATION} of the desk protocol and the desk \
             answered generation {}",
            answer.generation
        )));
    }
    Ok(answer.reply)
}

/// The desks a session holds, from its screens and its windows: every desk
/// a screen shows or a window sits on, with the screen showing it and how
/// many windows it holds. The protocol folds the desks into `screens`, and
/// generation 1's [`Screen`] row carries neither, so a native desk answers
/// them from what it does carry.
fn desks(screens: &[Screen], windows: &[Window]) -> Vec<DeskRow> {
    let mut rows: Vec<DeskRow> = Vec::new();
    let mut row = |id: i64, screen: &str, windows: i64| match rows
        .iter_mut()
        .find(|row: &&mut DeskRow| row.id == id)
    {
        Some(held) => {
            held.windows += windows;
            if held.screen.is_empty() {
                held.screen = screen.to_string();
            }
        }
        None => rows.push(DeskRow {
            id,
            screen: screen.to_string(),
            windows,
        }),
    };
    for screen in screens {
        row(i64::from(screen.desk), &screen.name, 0);
    }
    for window in windows {
        row(i64::from(window.desk), &window.screen, 1);
    }
    rows.sort_by_key(|row| row.id);
    rows
}
