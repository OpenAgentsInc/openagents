//! The three question types, the state and criteria they carry, and the checks
//! that run before a request leaves.
//!
//! A question asks one narrow judgment. A Noul asks whether something is true,
//! a Choice picks one of named options, and a Score places the state on an
//! ordered rubric. Every question in one request reads the same state and is
//! answered on its own.

use indexmap::IndexMap;
use serde::ser::{SerializeMap, SerializeStruct};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_json::{Map, Value};

use crate::Result;
use crate::error::Error;

/// The most options a Choice question takes.
const MAX_CHOICE_OPTIONS: usize = 255;

/// The fewest levels a Score question takes.
const MIN_SCORE_LEVELS: usize = 2;

/// The most levels a Score question takes.
const MAX_SCORE_LEVELS: usize = 10;

/// Text, a JSON object, a JSON array, or nothing.
///
/// State, instructions, and every criterion take one of these four shapes. A
/// number or a boolean becomes its text form, because the API reads none of the
/// four as a bare scalar.
#[derive(Debug, Clone, PartialEq, Default)]
pub enum Entry {
    /// Text.
    Text(String),
    /// A JSON object.
    Object(Map<String, Value>),
    /// A JSON array.
    Array(Vec<Value>),
    /// Nothing, which leaves the field undescribed.
    #[default]
    Null,
}

impl Entry {
    /// Build an entry from anything that serializes to JSON.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Config`] when the value does not serialize.
    pub fn json<T: Serialize>(value: &T) -> Result<Self> {
        let value = serde_json::to_value(value)
            .map_err(|error| Error::Config(format!("the value is not JSON: {error}")))?;
        Ok(Self::from(value))
    }

    /// The entry as a JSON value.
    #[must_use]
    pub fn to_value(&self) -> Value {
        match self {
            Self::Text(text) => Value::String(text.clone()),
            Self::Object(map) => Value::Object(map.clone()),
            Self::Array(items) => Value::Array(items.clone()),
            Self::Null => Value::Null,
        }
    }

    /// Whether the entry describes nothing.
    #[must_use]
    pub fn is_null(&self) -> bool {
        matches!(self, Self::Null)
    }
}

impl From<&str> for Entry {
    fn from(text: &str) -> Self {
        Self::Text(text.to_string())
    }
}

impl From<String> for Entry {
    fn from(text: String) -> Self {
        Self::Text(text)
    }
}

impl From<Map<String, Value>> for Entry {
    fn from(map: Map<String, Value>) -> Self {
        Self::Object(map)
    }
}

impl From<Vec<Value>> for Entry {
    fn from(items: Vec<Value>) -> Self {
        Self::Array(items)
    }
}

impl From<Value> for Entry {
    fn from(value: Value) -> Self {
        match value {
            Value::String(text) => Self::Text(text),
            Value::Object(map) => Self::Object(map),
            Value::Array(items) => Self::Array(items),
            Value::Null => Self::Null,
            other => Self::Text(other.to_string()),
        }
    }
}

impl Serialize for Entry {
    fn serialize<S: Serializer>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error> {
        self.to_value().serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for Entry {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> std::result::Result<Self, D::Error> {
        Value::deserialize(deserializer).map(Self::from)
    }
}

/// What a yes and a no mean, for a Noul question that reads better with the two
/// outcomes described.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct NoulCriteria {
    /// What a yes means.
    pub r#true: Option<Entry>,
    /// What a no means.
    pub r#false: Option<Entry>,
}

impl NoulCriteria {
    /// Criteria that describe neither outcome.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Describe the yes outcome.
    #[must_use]
    pub fn when_true<E: Into<Entry>>(mut self, description: E) -> Self {
        self.r#true = Some(description.into());
        self
    }

    /// Describe the no outcome.
    #[must_use]
    pub fn when_false<E: Into<Entry>>(mut self, description: E) -> Self {
        self.r#false = Some(description.into());
        self
    }
}

impl Serialize for NoulCriteria {
    fn serialize<S: Serializer>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error> {
        let mut map = serializer.serialize_map(None)?;
        if let Some(entry) = self.r#true.as_ref() {
            map.serialize_entry("true", entry)?;
        }
        if let Some(entry) = self.r#false.as_ref() {
            map.serialize_entry("false", entry)?;
        }
        map.end()
    }
}

/// A question that asks whether something is true. The answer is one
/// probability of yes.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Noul {
    /// The judgment to make.
    pub instructions: Option<Entry>,
    /// What a yes and a no mean.
    pub criteria: Option<NoulCriteria>,
}

impl Noul {
    /// Ask a judgment with no outcome descriptions.
    #[must_use]
    pub fn new<E: Into<Entry>>(instructions: E) -> Self {
        Self {
            instructions: Some(instructions.into()),
            criteria: None,
        }
    }

    /// Ask a judgment and describe its two outcomes.
    #[must_use]
    pub fn with_criteria<E: Into<Entry>>(instructions: E, criteria: NoulCriteria) -> Self {
        Self {
            instructions: Some(instructions.into()),
            criteria: Some(criteria),
        }
    }
}

impl Serialize for Noul {
    fn serialize<S: Serializer>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error> {
        let fields =
            1 + usize::from(self.instructions.is_some()) + usize::from(self.criteria.is_some());
        let mut state = serializer.serialize_struct("Noul", fields)?;
        state.serialize_field("type", "noul")?;
        if let Some(instructions) = self.instructions.as_ref() {
            state.serialize_field("instructions", instructions)?;
        }
        if let Some(criteria) = self.criteria.as_ref() {
            state.serialize_field("criteria", criteria)?;
        }
        state.end()
    }
}

/// A question that picks one of named options. The answer names the option and
/// carries a probability for each one.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Choice {
    /// The judgment to make.
    pub instructions: Option<Entry>,
    /// The options, in the order the request sends them.
    pub criteria: IndexMap<String, Option<Entry>>,
}

impl Choice {
    /// Ask a judgment over named options.
    #[must_use]
    pub fn new<E: Into<Entry>>(instructions: E, criteria: IndexMap<String, Option<Entry>>) -> Self {
        Self {
            instructions: Some(instructions.into()),
            criteria,
        }
    }

    /// Add one option, keeping the order the calls are made in.
    #[must_use]
    pub fn option<N: Into<String>, E: Into<Entry>>(mut self, name: N, description: E) -> Self {
        self.criteria.insert(name.into(), Some(description.into()));
        self
    }

    /// Add one option with no description.
    #[must_use]
    pub fn bare_option<N: Into<String>>(mut self, name: N) -> Self {
        self.criteria.insert(name.into(), None);
        self
    }
}

impl Serialize for Choice {
    fn serialize<S: Serializer>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error> {
        let fields = 2 + usize::from(self.instructions.is_some());
        let mut state = serializer.serialize_struct("Choice", fields)?;
        state.serialize_field("type", "choice")?;
        if let Some(instructions) = self.instructions.as_ref() {
            state.serialize_field("instructions", instructions)?;
        }
        state.serialize_field("criteria", &self.criteria)?;
        state.end()
    }
}

/// A question that places the state on an ordered rubric. Level zero is the
/// first entry of `criteria`.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Score {
    /// The judgment to make.
    pub instructions: Option<Entry>,
    /// The levels, in order from zero.
    pub criteria: Vec<Option<Entry>>,
}

impl Score {
    /// Ask a judgment over an ordered rubric.
    #[must_use]
    pub fn new<E: Into<Entry>>(instructions: E, criteria: Vec<Option<Entry>>) -> Self {
        Self {
            instructions: Some(instructions.into()),
            criteria,
        }
    }

    /// Add one level after the levels already named.
    #[must_use]
    pub fn level<E: Into<Entry>>(mut self, description: E) -> Self {
        self.criteria.push(Some(description.into()));
        self
    }
}

impl Serialize for Score {
    fn serialize<S: Serializer>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error> {
        let fields = 2 + usize::from(self.instructions.is_some());
        let mut state = serializer.serialize_struct("Score", fields)?;
        state.serialize_field("type", "score")?;
        if let Some(instructions) = self.instructions.as_ref() {
            state.serialize_field("instructions", instructions)?;
        }
        state.serialize_field("criteria", &self.criteria)?;
        state.end()
    }
}

/// One question of any type.
///
/// [`Question::Raw`] carries a body the SDK does not model, for a field the API
/// adds before this crate does.
#[derive(Debug, Clone, PartialEq)]
pub enum Question {
    /// A question that asks whether something is true.
    Noul(Noul),
    /// A question that picks one of named options.
    Choice(Choice),
    /// A question that places the state on an ordered rubric.
    Score(Score),
    /// A question written out as JSON, sent as it stands.
    Raw(Value),
}

impl Question {
    /// The wire name of the question's type, or the `type` a raw question
    /// carries.
    #[must_use]
    pub fn kind(&self) -> Option<&str> {
        match self {
            Self::Noul(_) => Some("noul"),
            Self::Choice(_) => Some("choice"),
            Self::Score(_) => Some("score"),
            Self::Raw(value) => value.get("type").and_then(Value::as_str),
        }
    }
}

impl From<Noul> for Question {
    fn from(question: Noul) -> Self {
        Self::Noul(question)
    }
}

impl From<Choice> for Question {
    fn from(question: Choice) -> Self {
        Self::Choice(question)
    }
}

impl From<Score> for Question {
    fn from(question: Score) -> Self {
        Self::Score(question)
    }
}

impl From<Value> for Question {
    fn from(question: Value) -> Self {
        Self::Raw(question)
    }
}

impl Serialize for Question {
    fn serialize<S: Serializer>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error> {
        match self {
            Self::Noul(question) => question.serialize(serializer),
            Self::Choice(question) => question.serialize(serializer),
            Self::Score(question) => question.serialize(serializer),
            Self::Raw(question) => question.serialize(serializer),
        }
    }
}

/// The questions one request asks, in the order they were added.
///
/// ```
/// use jev::{Noul, Questions, Score};
///
/// let questions = Questions::new()
///     .with("refund", Noul::new("Does the customer ask for money back?"))
///     .with(
///         "severity",
///         Score::new("How severe is the issue?", Vec::new())
///             .level("Cosmetic")
///             .level("Blocking"),
///     );
/// assert_eq!(questions.len(), 2);
/// assert!(questions.validate().is_ok());
/// ```
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Questions(IndexMap<String, Question>);

impl Questions {
    /// An empty set. A request needs at least one question.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Add one question, keeping the order the calls are made in.
    #[must_use]
    pub fn with<I: Into<String>, Q: Into<Question>>(mut self, id: I, question: Q) -> Self {
        self.0.insert(id.into(), question.into());
        self
    }

    /// Add one question, returning the question that id already held.
    pub fn insert<I: Into<String>, Q: Into<Question>>(
        &mut self,
        id: I,
        question: Q,
    ) -> Option<Question> {
        self.0.insert(id.into(), question.into())
    }

    /// The question one id holds.
    #[must_use]
    pub fn get(&self, id: &str) -> Option<&Question> {
        self.0.get(id)
    }

    /// How many questions the set holds.
    #[must_use]
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// Whether the set holds no question.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Every question with its id, in order.
    pub fn iter(&self) -> impl Iterator<Item = (&str, &Question)> {
        self.0.iter().map(|(id, question)| (id.as_str(), question))
    }

    /// Check the set the way both official SDKs check theirs, and the two
    /// limits the API documents.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Question`] when the set is empty, a Score names fewer
    /// than two or more than ten levels, or a Choice names more than 255
    /// options. The error names the question at fault.
    pub fn validate(&self) -> Result<()> {
        if self.0.is_empty() {
            return Err(Error::Question {
                id: String::new(),
                message: "a request asks at least one question".to_string(),
            });
        }
        for (id, question) in self.iter() {
            match question {
                Question::Score(score) => check_score(id, score.criteria.len())?,
                Question::Choice(choice) => check_choice(id, choice.criteria.len())?,
                Question::Noul(_) => {}
                Question::Raw(value) => check_raw(id, value)?,
            }
        }
        Ok(())
    }
}

impl From<IndexMap<String, Question>> for Questions {
    fn from(questions: IndexMap<String, Question>) -> Self {
        Self(questions)
    }
}

impl<K: Into<String>, Q: Into<Question>> FromIterator<(K, Q)> for Questions {
    fn from_iter<T: IntoIterator<Item = (K, Q)>>(items: T) -> Self {
        Self(
            items
                .into_iter()
                .map(|(id, question)| (id.into(), question.into()))
                .collect(),
        )
    }
}

impl Serialize for Questions {
    fn serialize<S: Serializer>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error> {
        self.0.serialize(serializer)
    }
}

/// A Score question names an ordered rubric of two to ten levels.
fn check_score(id: &str, levels: usize) -> Result<()> {
    if levels < MIN_SCORE_LEVELS {
        return Err(Error::Question {
            id: id.to_string(),
            message: format!(
                "a Score question names at least {MIN_SCORE_LEVELS} levels, and this one names {levels}"
            ),
        });
    }
    if levels > MAX_SCORE_LEVELS {
        return Err(Error::Question {
            id: id.to_string(),
            message: format!(
                "a Score question names at most {MAX_SCORE_LEVELS} levels, and this one names {levels}"
            ),
        });
    }
    Ok(())
}

/// A Choice question names at most 255 options.
fn check_choice(id: &str, options: usize) -> Result<()> {
    if options > MAX_CHOICE_OPTIONS {
        return Err(Error::Question {
            id: id.to_string(),
            message: format!(
                "a Choice question names at most {MAX_CHOICE_OPTIONS} options, and this one names {options}"
            ),
        });
    }
    Ok(())
}

/// A question written out as JSON names a type, and a Choice or a Score names
/// criteria, the way the Python SDK checks a question dictionary.
fn check_raw(id: &str, value: &Value) -> Result<()> {
    let kind = value
        .get("type")
        .and_then(Value::as_str)
        .filter(|kind| !kind.is_empty())
        .ok_or_else(|| Error::Question {
            id: id.to_string(),
            message: "a question written out as JSON names a nonempty `type`".to_string(),
        })?;
    if matches!(kind, "choice" | "score") && value.get("criteria").is_none() {
        return Err(Error::Question {
            id: id.to_string(),
            message: format!("a {kind} question names `criteria`"),
        });
    }
    match kind {
        "score" => check_score(
            id,
            value
                .get("criteria")
                .and_then(Value::as_array)
                .map_or(0, Vec::len),
        ),
        "choice" => check_choice(
            id,
            value
                .get("criteria")
                .and_then(Value::as_object)
                .map_or(0, Map::len),
        ),
        _ => Ok(()),
    }
}
