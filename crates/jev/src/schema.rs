//! Compile a flat object schema into [`Questions`], and read the answers back
//! as a typed value with an answers sidecar.
//!
//! A boolean property becomes a Noul question, a string property with `enum`
//! becomes a Choice, and an integer property with `minimum`/`maximum` (or
//! `levels`) becomes a Score. A property's `description` is the question's
//! instructions; its `criteria` carries the meaning of each option (an object
//! keyed by option for a Choice, an array from the lowest level for a Score,
//! and `true`/`false` keys for a Noul). Properties are asked in the order the
//! schema object yields them.
//!
//! [`Sift`] asks the same questions of many items: items × questions go into
//! one request until the request would carry more than
//! [`SiftLimits::questions_per_request`] questions or its state would exceed
//! [`SiftLimits::state_bytes`]; the items are then split in order, at most
//! [`SiftLimits::concurrency`] requests run at once, and the first failure
//! cancels the rest. An item that cannot fit alone is refused before any
//! request runs.

use indexmap::IndexMap;
use serde::Serialize;
use serde_json::{Map, Value, json};
use tokio::task::JoinSet;

use crate::Result;
use crate::answers::{Answer, SystemOneResponse};
use crate::client::{Client, SystemOneRequest};
use crate::error::Error;
use crate::questions::{Choice, Noul, NoulCriteria, Question, Questions, Score};

/// What one schema property compiles to.
#[derive(Debug, Clone, PartialEq)]
pub enum Field {
    /// A boolean property: the typed value is `true` at or above one half.
    Bool,
    /// A string property with `enum`: the typed value is the chosen option.
    Enum(Vec<String>),
    /// An integer property: the typed value is `minimum` plus the selected
    /// level.
    Level {
        /// The integer the lowest level stands for.
        minimum: i64,
        /// How many levels there are.
        levels: usize,
    },
}

/// A schema compiled to questions, keeping how to read each answer.
#[derive(Debug, Clone, PartialEq)]
pub struct Compiled {
    questions: Questions,
    fields: IndexMap<String, Field>,
}

/// One item's typed value and the raw answers beside it.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Decoded {
    /// The typed object the schema described.
    pub value: Map<String, Value>,
    /// Every answer as the door sent it, keyed by property, with its
    /// probabilities — the `<as>$answers` sidecar.
    pub answers: Map<String, Value>,
}

/// Compile a flat object schema into questions.
///
/// # Errors
///
/// Returns [`Error::Config`] when the schema is not an object with
/// `properties`, when a property's type is not boolean, string with `enum`,
/// or integer with levels, or when a property name is empty or starts with
/// `$` (reserved for sidecars).
pub fn compile(schema: &Value) -> Result<Compiled> {
    let properties = schema
        .get("properties")
        .and_then(Value::as_object)
        .ok_or_else(|| Error::Config("the schema must be an object with `properties`".into()))?;
    if properties.is_empty() {
        return Err(Error::Config("the schema names no properties".into()));
    }
    let mut questions = Questions::new();
    let mut fields = IndexMap::new();
    for (name, property) in properties {
        if name.is_empty() || name.starts_with('$') {
            return Err(Error::Config(format!(
                "property `{name}` can't be a question: names must be non-empty and not start with `$`"
            )));
        }
        let (question, field) = compile_property(name, property)?;
        questions = questions.with(name.clone(), question);
        fields.insert(name.clone(), field);
    }
    Ok(Compiled { questions, fields })
}

fn compile_property(name: &str, property: &Value) -> Result<(Question, Field)> {
    let kind = property.get("type").and_then(Value::as_str).unwrap_or("");
    let instructions = property
        .get("description")
        .and_then(Value::as_str)
        .filter(|text| !text.trim().is_empty())
        .ok_or_else(|| Error::Config(format!("property `{name}` needs a `description`")))?;
    let criteria = property.get("criteria");
    match kind {
        "boolean" => {
            let mut noul = NoulCriteria::new();
            if let Some(yes) = criteria.and_then(|c| c.get("true")).and_then(Value::as_str) {
                noul = noul.when_true(yes);
            }
            if let Some(no) = criteria
                .and_then(|c| c.get("false"))
                .and_then(Value::as_str)
            {
                noul = noul.when_false(no);
            }
            let question = if criteria.is_some() {
                Noul::with_criteria(instructions, noul)
            } else {
                Noul::new(instructions)
            };
            Ok((question.into(), Field::Bool))
        }
        "string" => {
            let options: Vec<String> = property
                .get("enum")
                .and_then(Value::as_array)
                .map(|values| {
                    values
                        .iter()
                        .filter_map(Value::as_str)
                        .map(str::to_owned)
                        .collect()
                })
                .unwrap_or_default();
            if options.len() < 2 {
                return Err(Error::Config(format!(
                    "property `{name}` needs an `enum` of at least two strings to become a choice"
                )));
            }
            let mut choice = Choice::new(instructions, IndexMap::new());
            for option in &options {
                match criteria.and_then(|c| c.get(option)).and_then(Value::as_str) {
                    Some(meaning) => choice = choice.option(option.clone(), meaning),
                    None => choice = choice.bare_option(option.clone()),
                }
            }
            Ok((choice.into(), Field::Enum(options)))
        }
        "integer" => {
            let minimum = property.get("minimum").and_then(Value::as_i64).unwrap_or(0);
            let levels: Vec<Option<crate::Entry>> = match criteria.and_then(Value::as_array) {
                Some(levels) => levels
                    .iter()
                    .map(|level| level.as_str().map(crate::Entry::from))
                    .collect(),
                None => {
                    let maximum = property.get("maximum").and_then(Value::as_i64).ok_or_else(|| {
                        Error::Config(format!(
                            "property `{name}` needs `maximum` or a `criteria` array to become a score"
                        ))
                    })?;
                    let count = maximum - minimum + 1;
                    if !(2..=10).contains(&count) {
                        return Err(Error::Config(format!(
                            "property `{name}` spans {count} levels; a score has 2 to 10"
                        )));
                    }
                    (minimum..=maximum)
                        .map(|level| Some(crate::Entry::from(level.to_string())))
                        .collect()
                }
            };
            if !(2..=10).contains(&levels.len()) {
                return Err(Error::Config(format!(
                    "property `{name}` has {} levels; a score has 2 to 10",
                    levels.len()
                )));
            }
            let field = Field::Level {
                minimum,
                levels: levels.len(),
            };
            Ok((Score::new(instructions, levels).into(), field))
        }
        other => Err(Error::Config(format!(
            "property `{name}` has type `{other}`; only boolean, string with enum, and integer compile to questions"
        ))),
    }
}

impl Compiled {
    /// The questions, keyed by property name.
    #[must_use]
    pub fn questions(&self) -> &Questions {
        &self.questions
    }

    /// How each property reads its answer, in schema order.
    #[must_use]
    pub fn fields(&self) -> &IndexMap<String, Field> {
        &self.fields
    }

    /// Read one response, whose question ids are the property names.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Question`] when an answer is missing or of another
    /// type than its property asked.
    pub fn decode(&self, response: &SystemOneResponse) -> Result<Decoded> {
        self.decode_prefixed(&response.answers, "")
    }

    fn decode_prefixed(&self, answers: &IndexMap<String, Answer>, prefix: &str) -> Result<Decoded> {
        let mut value = Map::new();
        let mut sidecar = Map::new();
        for (name, field) in &self.fields {
            let id = format!("{prefix}{name}");
            let answer = answers.get(&id).ok_or_else(|| Error::Question {
                id: id.clone(),
                message: "the response carries no answer for this question".into(),
            })?;
            let typed = match (field, answer) {
                (Field::Bool, Answer::Noul(noul)) => json!(noul.noul >= 0.5),
                (Field::Enum(_), Answer::Choice(choice)) => json!(choice.choice),
                (Field::Level { minimum, levels }, Answer::Score(score)) => {
                    let level = score
                        .selected
                        .as_deref()
                        .and_then(|selected| selected.parse::<u32>().ok())
                        .or_else(|| {
                            score
                                .probabilities
                                .iter()
                                .max_by(|a, b| a.1.total_cmp(b.1).then(a.0.cmp(b.0)))
                                .map(|(level, _)| *level)
                        })
                        .unwrap_or_else(|| {
                            score.score.round().clamp(0.0, (*levels - 1) as f64) as u32
                        });
                    json!(minimum + i64::from(level))
                }
                (_, other) => {
                    return Err(Error::Question {
                        id,
                        message: format!(
                            "the answer is a {} where the schema asked otherwise",
                            other.kind()
                        ),
                    });
                }
            };
            value.insert(name.clone(), typed);
            sidecar.insert(name.clone(), answer.to_value());
        }
        Ok(Decoded {
            value,
            answers: sidecar,
        })
    }
}

/// How large one sift request may grow.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SiftLimits {
    /// Most questions in one request (items × properties).
    pub questions_per_request: usize,
    /// Most bytes of JSON state in one request.
    pub state_bytes: usize,
    /// Most requests in flight at once.
    pub concurrency: usize,
}

impl Default for SiftLimits {
    fn default() -> Self {
        Self {
            questions_per_request: 64,
            state_bytes: 64 * 1024,
            concurrency: 4,
        }
    }
}

/// Which items each request carries, in order.
///
/// # Errors
///
/// Returns [`Error::Config`] when a limit is zero, when the schema alone has
/// more questions than one request admits, or when one item's state does not
/// fit a request by itself.
pub fn plan(compiled: &Compiled, items: &[Value], limits: SiftLimits) -> Result<Vec<Vec<usize>>> {
    if limits.questions_per_request == 0 || limits.state_bytes == 0 || limits.concurrency == 0 {
        return Err(Error::Config("sift limits must all be above zero".into()));
    }
    let per_item = compiled.questions.len();
    if per_item > limits.questions_per_request {
        return Err(Error::Config(format!(
            "the schema asks {per_item} questions per item; one request admits {}",
            limits.questions_per_request
        )));
    }
    let overhead = "{\"items\":[]}".len();
    let mut batches: Vec<Vec<usize>> = Vec::new();
    let mut current: Vec<usize> = Vec::new();
    let mut current_bytes = overhead;
    for (index, item) in items.iter().enumerate() {
        let bytes = serde_json::to_vec(item)
            .map_err(|error| {
                Error::Config(format!("item {index} can't be encoded as JSON: {error}"))
            })?
            .len();
        if overhead + bytes > limits.state_bytes {
            return Err(Error::Config(format!(
                "item {index} is {bytes} bytes of state; one request admits {}",
                limits.state_bytes
            )));
        }
        let too_many = (current.len() + 1) * per_item > limits.questions_per_request;
        let too_big = current_bytes + bytes + usize::from(!current.is_empty()) > limits.state_bytes;
        if !current.is_empty() && (too_many || too_big) {
            batches.push(std::mem::take(&mut current));
            current_bytes = overhead;
        }
        current_bytes += bytes + usize::from(!current.is_empty());
        current.push(index);
    }
    if !current.is_empty() {
        batches.push(current);
    }
    Ok(batches)
}

/// The request one batch sends: the items under `items`, and each item's
/// questions under `<index>.<property>`.
#[must_use]
pub fn batch_request(compiled: &Compiled, items: &[Value], batch: &[usize]) -> SystemOneRequest {
    let mut questions = Questions::new();
    let state: Vec<&Value> = batch.iter().map(|index| &items[*index]).collect();
    for (position, index) in batch.iter().enumerate() {
        for (name, question) in compiled.questions.iter() {
            let scoped = scope(question, position, batch.len());
            questions = questions.with(format!("{index}.{name}"), scoped);
        }
    }
    SystemOneRequest::new(json!({ "items": state }), questions)
}

fn scope(question: &Question, position: usize, count: usize) -> Question {
    if count == 1 {
        return question.clone();
    }
    let lead = format!("About `items[{position}]` only: ");
    let Ok(mut value) = serde_json::to_value(question) else {
        return question.clone();
    };
    if let Some(instructions) = value.get("instructions").and_then(Value::as_str) {
        let text = format!("{lead}{instructions}");
        value["instructions"] = json!(text);
    }
    Question::from_value(value)
}

/// Ask one compiled schema of many items.
#[derive(Debug, Clone)]
pub struct Sift {
    compiled: Compiled,
    limits: SiftLimits,
    model: Option<String>,
}

impl Sift {
    /// A sift over one compiled schema with the default limits.
    #[must_use]
    pub fn new(compiled: Compiled) -> Self {
        Self {
            compiled,
            limits: SiftLimits::default(),
            model: None,
        }
    }

    /// Other limits.
    #[must_use]
    pub fn limits(mut self, limits: SiftLimits) -> Self {
        self.limits = limits;
        self
    }

    /// Ask another door than the client's default.
    #[must_use]
    pub fn model<M: Into<String>>(mut self, model: M) -> Self {
        self.model = Some(model.into());
        self
    }

    /// Ask every item, in order. The first failed request cancels the rest.
    ///
    /// # Errors
    ///
    /// Returns the planning errors of [`plan`], then whichever request error
    /// came first.
    pub async fn run(&self, client: &Client, items: &[Value]) -> Result<Vec<Decoded>> {
        let batches = plan(&self.compiled, items, self.limits)?;
        let mut decoded: Vec<Option<Decoded>> = vec![None; items.len()];
        let mut pending = batches.into_iter();
        let mut set = JoinSet::new();
        let mut start = |set: &mut JoinSet<Result<(Vec<usize>, SystemOneResponse)>>| {
            if let Some(batch) = pending.next() {
                let mut request = batch_request(&self.compiled, items, &batch);
                if let Some(model) = &self.model {
                    request = request.model(model.clone());
                }
                let client = client.clone();
                set.spawn(async move {
                    let response = client.system_one(request).await?;
                    Ok((batch, response))
                });
            }
        };
        for _ in 0..self.limits.concurrency {
            start(&mut set);
        }
        while let Some(joined) = set.join_next().await {
            let (batch, response) = match joined {
                Ok(Ok(done)) => done,
                Ok(Err(error)) => {
                    set.abort_all();
                    return Err(error);
                }
                Err(join) => {
                    set.abort_all();
                    return Err(Error::Config(format!(
                        "a sift request stopped before it answered: {join}"
                    )));
                }
            };
            for index in batch {
                let item = self
                    .compiled
                    .decode_prefixed(&response.answers, &format!("{index}."))?;
                decoded[index] = Some(item);
            }
            start(&mut set);
        }
        Ok(decoded.into_iter().flatten().collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn schema() -> Value {
        json!({
            "type": "object",
            "properties": {
                "refund": {
                    "type": "boolean",
                    "description": "Does the customer ask for money back?",
                    "criteria": {"true": "Asks for a refund or chargeback", "false": "Anything else"}
                },
                "department": {
                    "type": "string",
                    "enum": ["billing", "technical", "none"],
                    "description": "Which team handles this?",
                    "criteria": {"billing": "Charges and invoices", "technical": "Bugs and outages"}
                },
                "urgency": {
                    "type": "integer",
                    "minimum": 1,
                    "maximum": 3,
                    "description": "How urgent is it?",
                    "criteria": ["Can wait", "This week", "Today"]
                }
            }
        })
    }

    #[test]
    fn a_flat_schema_compiles_to_one_question_per_property() {
        let compiled = compile(&schema()).unwrap();
        let questions = compiled.questions().to_value();
        assert_eq!(questions["refund"]["type"], "noul");
        assert_eq!(
            questions["refund"]["criteria"]["true"],
            "Asks for a refund or chargeback"
        );
        assert_eq!(questions["department"]["type"], "choice");
        assert_eq!(
            questions["department"]["criteria"]["technical"],
            "Bugs and outages"
        );
        assert_eq!(questions["department"]["criteria"]["none"], Value::Null);
        assert_eq!(questions["urgency"]["type"], "score");
        assert_eq!(questions["urgency"]["criteria"][2], "Today");
        assert_eq!(
            compiled.fields()["urgency"],
            Field::Level {
                minimum: 1,
                levels: 3
            }
        );
    }

    #[test]
    fn bad_shapes_are_refused_before_any_request() {
        let cases = [
            json!({"type": "object"}),
            json!({"properties": {"$answers": {"type": "boolean", "description": "x"}}}),
            json!({"properties": {"a": {"type": "boolean"}}}),
            json!({"properties": {"a": {"type": "string", "description": "x"}}}),
            json!({"properties": {"a": {"type": "integer", "description": "x"}}}),
            json!({"properties": {"a": {"type": "integer", "description": "x", "minimum": 0, "maximum": 40}}}),
            json!({"properties": {"a": {"type": "number", "description": "x"}}}),
        ];
        for case in cases {
            assert!(matches!(compile(&case), Err(Error::Config(_))), "{case}");
        }
    }

    #[test]
    fn answers_decode_to_a_typed_value_with_probabilities_beside_it() {
        let compiled = compile(&schema()).unwrap();
        let mut answers: IndexMap<String, Answer> = IndexMap::new();
        answers.insert(
            "refund".into(),
            Answer::Noul(crate::NoulAnswer {
                noul: 0.91,
                selected: None,
            }),
        );
        answers.insert(
            "department".into(),
            Answer::Choice(crate::ChoiceAnswer {
                choice: "billing".into(),
                confidence: 0.8,
                probabilities: [("billing", 0.8), ("technical", 0.15), ("none", 0.05)]
                    .into_iter()
                    .map(|(k, v)| (k.to_owned(), v))
                    .collect(),
            }),
        );
        answers.insert(
            "urgency".into(),
            Answer::Score(crate::ScoreAnswer {
                score: 1.7,
                confidence: 0.6,
                selected: Some("2".into()),
                legend: [(0, "Can wait"), (1, "This week"), (2, "Today")]
                    .into_iter()
                    .map(|(k, v)| (k, crate::Entry::from(v)))
                    .collect(),
                probabilities: [(0, 0.1), (1, 0.3), (2, 0.6)].into_iter().collect(),
            }),
        );
        let decoded = compiled.decode_prefixed(&answers, "").unwrap();
        assert_eq!(
            Value::Object(decoded.value),
            json!({"refund": true, "department": "billing", "urgency": 3})
        );
        assert_eq!(
            decoded.answers["department"]["probabilities"]["technical"],
            0.15
        );
        assert_eq!(decoded.answers["urgency"]["probabilities"]["2"], 0.6);

        let mut missing = answers.clone();
        missing.shift_remove("urgency");
        assert!(matches!(
            compiled.decode_prefixed(&missing, ""),
            Err(Error::Question { id, .. }) if id == "urgency"
        ));
    }

    #[test]
    fn a_sift_splits_items_in_order_and_refuses_one_that_cannot_fit() {
        let compiled = compile(&schema()).unwrap();
        let items: Vec<Value> = (0..7).map(|n| json!({"n": n})).collect();
        let limits = SiftLimits {
            questions_per_request: 7,
            ..SiftLimits::default()
        };
        assert_eq!(
            plan(&compiled, &items, limits).unwrap(),
            vec![vec![0, 1], vec![2, 3], vec![4, 5], vec![6]]
        );

        let by_bytes = SiftLimits {
            state_bytes: 40,
            ..SiftLimits::default()
        };
        let batches = plan(&compiled, &items, by_bytes).unwrap();
        assert_eq!(batches.concat(), (0..7).collect::<Vec<_>>());
        assert!(batches.iter().all(|batch| batch.len() <= 3), "{batches:?}");

        let huge = vec![json!({"text": "x".repeat(100)})];
        assert!(matches!(
            plan(&compiled, &huge, by_bytes),
            Err(Error::Config(_))
        ));
        let narrow = SiftLimits {
            questions_per_request: 2,
            ..SiftLimits::default()
        };
        assert!(matches!(
            plan(&compiled, &items, narrow),
            Err(Error::Config(_))
        ));

        let request = batch_request(&compiled, &items, &[2, 3]);
        let body = request.body("jev-latest").unwrap();
        assert_eq!(body["state"]["items"][1], json!({"n": 3}));
        let questions = &body["questions"];
        assert_eq!(questions.as_object().unwrap().len(), 6);
        assert!(
            questions["3.refund"]["instructions"]
                .as_str()
                .unwrap()
                .starts_with("About `items[1]` only: ")
        );
        assert_eq!(questions["2.department"]["type"], "choice");
    }
}
