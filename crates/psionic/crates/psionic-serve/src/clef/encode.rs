//! The Clef prompt: a byte-exact port of Cloudflare's reference
//! `encode_record` (`joint_schema_model.py`, the same file in
//! `Cloudflare/clef-flash` and `Cloudflare/clef`).
//!
//! Each piece is tokenized on its own (`add_special_tokens=False`, special
//! markers parsed), because tokenizing the joined string gives different ids
//! at the joins. The question span covers the rendered instructions and each
//! option span covers the rendered `{"option_id", "description"}` object.

use super::json::{ClefJson, render};

/// The reference system prompt.
pub const SYSTEM_PROMPT: &str = "Read the complete state and schema. Decide every field jointly. Each answer must be exactly one of that field's allowed options.";
/// The default noul option descriptions.
pub const NOUL_TRUE: &str = "The proposition is true or the answer is yes.";
pub const NOUL_FALSE: &str = "The proposition is false or the answer is no.";
/// The prompt length the head was trained at (the reference `max_length`).
pub const TRAINED_LENGTH: usize = 16_384;

/// A question's type, with the reference's type ids.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum QuestionType {
    Noul,
    Choice,
    Score,
}

impl QuestionType {
    #[must_use]
    pub fn id(self) -> usize {
        match self {
            Self::Noul => 0,
            Self::Choice => 1,
            Self::Score => 2,
        }
    }

    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::Noul => "noul",
            Self::Choice => "choice",
            Self::Score => "score",
        }
    }
}

/// One question of a request, validated.
#[derive(Clone, Debug)]
pub struct ClefQuestion {
    pub id: String,
    pub kind: QuestionType,
    /// What the INSTRUCTION line renders: `instructions`, or the id when it
    /// is missing, `null` or `""`.
    pub instructions: ClefJson,
    /// Options in prompt order: noul `true, false`; choice keys sorted by
    /// code point; score `"0".."n-1"`. Each is `(option_id, description)`.
    pub options: Vec<(String, ClefJson)>,
    /// Choice keys in request order (answers report probabilities in this
    /// order); score levels `"0".."n-1"`; noul `true, false`.
    pub request_order: Vec<String>,
    /// The score legend: the criteria values in order.
    pub legend: Vec<ClefJson>,
}

/// How a request over the token budget is handled.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Truncation {
    /// Refuse with `not_admitted` (the default).
    Refuse,
    /// Keep the head of the state, as the reference does silently.
    StateTail,
}

/// A validated System One request.
#[derive(Clone, Debug)]
pub struct ClefRequest {
    pub model: String,
    pub state: ClefJson,
    pub questions: Vec<ClefQuestion>,
    pub truncation: Truncation,
}

/// Request limits ([`super::ClefLimits`] holds the served values).
#[derive(Clone, Copy, Debug)]
pub struct RequestLimits {
    pub max_questions: usize,
    pub max_options: usize,
}

/// Why a request is refused before any compute.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RequestRefusal {
    /// The request itself is wrong; no server would answer it (400).
    Invalid(String),
    /// This server will not take it, another door might (`not_admitted`).
    NotAdmitted(String),
}

fn invalid(message: impl Into<String>) -> RequestRefusal {
    RequestRefusal::Invalid(message.into())
}

impl ClefRequest {
    /// Reads a request body (already parsed with [`super::json::parse`]).
    pub fn from_json(body: &ClefJson, limits: RequestLimits) -> Result<Self, RequestRefusal> {
        let ClefJson::Object(_) = body else {
            return Err(invalid("the request body must be a JSON object"));
        };
        let model = match body.get("model") {
            Some(ClefJson::String(model)) => model.clone(),
            None | Some(ClefJson::Null) => String::new(),
            Some(_) => return Err(invalid("model must be a string")),
        };
        let state = body
            .get("state")
            .cloned()
            .ok_or_else(|| invalid("state is required"))?;
        for media in ["images", "videos"] {
            if body.get(media).is_some_and(ClefJson::is_truthy) {
                return Err(RequestRefusal::NotAdmitted(format!(
                    "{media} are not supported by this Clef server yet; send text only or use another door"
                )));
            }
        }
        let truncation = match body.get("truncation") {
            None | Some(ClefJson::Null) => Truncation::Refuse,
            Some(ClefJson::String(value)) if value == "refuse" => Truncation::Refuse,
            Some(ClefJson::String(value)) if value == "state_tail" => Truncation::StateTail,
            Some(_) => {
                return Err(invalid("truncation must be \"refuse\" or \"state_tail\""));
            }
        };
        let Some(ClefJson::Object(questions)) = body.get("questions") else {
            return Err(invalid(
                "questions must be an object with at least one question",
            ));
        };
        if questions.is_empty() {
            return Err(invalid("at least one question is required"));
        }
        if questions.len() > limits.max_questions {
            return Err(RequestRefusal::NotAdmitted(format!(
                "{} questions is more than this server takes ({})",
                questions.len(),
                limits.max_questions
            )));
        }
        let questions = questions
            .iter()
            .map(|(id, question)| ClefQuestion::from_json(id, question, limits))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Self {
            model,
            state,
            questions,
            truncation,
        })
    }
}

impl ClefQuestion {
    fn from_json(
        id: &str,
        question: &ClefJson,
        limits: RequestLimits,
    ) -> Result<Self, RequestRefusal> {
        let ClefJson::Object(_) = question else {
            return Err(invalid(format!("{id}: a question must be an object")));
        };
        let kind = match question.get("type") {
            Some(ClefJson::String(kind)) if kind == "noul" => QuestionType::Noul,
            Some(ClefJson::String(kind)) if kind == "choice" => QuestionType::Choice,
            Some(ClefJson::String(kind)) if kind == "score" => QuestionType::Score,
            _ => {
                return Err(invalid(format!(
                    "{id}: type must be noul, choice, or score"
                )));
            }
        };
        let instructions = match question.get("instructions") {
            None | Some(ClefJson::Null) => ClefJson::String(id.to_string()),
            Some(ClefJson::String(text)) if text.is_empty() => ClefJson::String(id.to_string()),
            Some(other) => other.clone(),
        };
        let criteria = question.get("criteria");
        if kind != QuestionType::Noul && !criteria.is_some_and(ClefJson::is_truthy) {
            return Err(invalid(format!("{id}: criteria must not be empty")));
        }
        let (options, request_order, legend) = match kind {
            QuestionType::Noul => {
                let mut true_description = ClefJson::String(NOUL_TRUE.to_string());
                let mut false_description = ClefJson::String(NOUL_FALSE.to_string());
                match criteria {
                    None => {}
                    Some(value) if !value.is_truthy() => {}
                    Some(ClefJson::Object(members)) => {
                        for (key, value) in members {
                            match key.as_str() {
                                "true" => true_description = value.clone(),
                                "false" => false_description = value.clone(),
                                _ => {}
                            }
                        }
                    }
                    Some(_) => {
                        return Err(invalid(format!(
                            "{id}: noul criteria must be an object with \"true\" and \"false\""
                        )));
                    }
                }
                (
                    vec![
                        (String::from("true"), true_description),
                        (String::from("false"), false_description),
                    ],
                    vec![String::from("true"), String::from("false")],
                    Vec::new(),
                )
            }
            QuestionType::Choice => {
                let Some(ClefJson::Object(members)) = criteria else {
                    return Err(invalid(format!(
                        "{id}: choice criteria must be an object of option ids"
                    )));
                };
                let request_order: Vec<String> =
                    members.iter().map(|(key, _)| key.clone()).collect();
                let mut options: Vec<(String, ClefJson)> = members.clone();
                options.sort_by(|left, right| left.0.cmp(&right.0));
                (options, request_order, Vec::new())
            }
            QuestionType::Score => {
                // The reference enumerates the criteria: a list gives its
                // values, an object its keys.
                let legend: Vec<ClefJson> = match criteria {
                    Some(ClefJson::Array(values)) => values.clone(),
                    Some(ClefJson::Object(members)) => members
                        .iter()
                        .map(|(key, _)| ClefJson::String(key.clone()))
                        .collect(),
                    _ => {
                        return Err(invalid(format!(
                            "{id}: score criteria must be a list of level descriptions"
                        )));
                    }
                };
                let options = legend
                    .iter()
                    .enumerate()
                    .map(|(index, value)| (index.to_string(), value.clone()))
                    .collect::<Vec<_>>();
                let request_order = (0..legend.len()).map(|index| index.to_string()).collect();
                (options, request_order, legend)
            }
        };
        if kind != QuestionType::Noul {
            if options.len() < 2 {
                return Err(invalid(format!(
                    "{id}: a question needs at least 2 options"
                )));
            }
            if options.len() > limits.max_options {
                return Err(RequestRefusal::NotAdmitted(format!(
                    "{id}: {} options is more than this server takes ({})",
                    options.len(),
                    limits.max_options
                )));
            }
        }
        Ok(Self {
            id: id.to_string(),
            kind,
            instructions,
            options,
            request_order,
            legend,
        })
    }
}

/// One encoded question: spans are `[start, end)` into the input ids.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EncodedQuestion {
    pub question_id: String,
    pub question_type: usize,
    pub question_span: (usize, usize),
    pub option_spans: Vec<(usize, usize)>,
    pub option_ids: Vec<String>,
}

/// A prompt ready for the backbone.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EncodedRecord {
    pub input_ids: Vec<u32>,
    pub questions: Vec<EncodedQuestion>,
    /// State tokens dropped by `truncation: "state_tail"`.
    pub truncated_state_tokens: usize,
}

/// Why a request does not fit the token budget.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BudgetRefusal {
    pub prompt_tokens: usize,
    pub fixed_tokens: usize,
    pub budget: usize,
}

/// Encodes a request. `tokenize` encodes one piece without BOS/EOS and with
/// special markers parsed. `budget` is the most tokens admitted: a schema
/// that alone exceeds it is always refused; a state that pushes the prompt
/// over it is refused unless `truncation` is `state_tail`.
pub fn encode_record(
    request: &ClefRequest,
    tokenize: &dyn Fn(&str) -> Vec<u32>,
    budget: usize,
) -> Result<EncodedRecord, BudgetRefusal> {
    let mut schema_ids = tokenize("\n\nSCHEMA FIELDS:\n");
    let mut questions = Vec::with_capacity(request.questions.len());
    for (index, question) in request.questions.iter().enumerate() {
        schema_ids.extend(tokenize(&format!(
            "\nFIELD {}\nID: {}\nTYPE: {}\nINSTRUCTION: ",
            index + 1,
            question.id,
            question.kind.label()
        )));
        let question_start = schema_ids.len();
        schema_ids.extend(tokenize(&render(&question.instructions)));
        let question_end = schema_ids.len();
        schema_ids.extend(tokenize("\nALLOWED OPTIONS:\n"));
        let mut option_spans = Vec::with_capacity(question.options.len());
        let mut option_ids = Vec::with_capacity(question.options.len());
        for (option_index, (option_id, description)) in question.options.iter().enumerate() {
            schema_ids.extend(tokenize(&format!("OPTION {}: ", option_index + 1)));
            let option_start = schema_ids.len();
            let mut semantics = vec![(
                String::from("option_id"),
                ClefJson::String(option_id.clone()),
            )];
            if *description != ClefJson::Null {
                semantics.push((String::from("description"), description.clone()));
            }
            schema_ids.extend(tokenize(&render(&ClefJson::Object(semantics))));
            option_spans.push((option_start, schema_ids.len()));
            option_ids.push(option_id.clone());
            schema_ids.extend(tokenize("\n"));
        }
        schema_ids.extend(tokenize("END FIELD\n"));
        questions.push(EncodedQuestion {
            question_id: question.id.clone(),
            question_type: question.kind.id(),
            question_span: (question_start, question_end),
            option_spans,
            option_ids,
        });
    }
    let prefix_ids = tokenize(&format!(
        "<|im_start|>system\n{SYSTEM_PROMPT}<|im_end|>\n<|im_start|>user\nSTATE:\n"
    ));
    let suffix_ids = tokenize(
        "\n<|im_end|>\n<|im_start|>assistant\n<think>\n\n</think>\n\nJOINT SCHEMA DECISIONS:",
    );
    let mut state_ids = tokenize(&render(&request.state));
    let fixed = prefix_ids.len() + schema_ids.len() + suffix_ids.len();
    let refusal = BudgetRefusal {
        prompt_tokens: fixed + state_ids.len(),
        fixed_tokens: fixed,
        budget,
    };
    if fixed > budget {
        return Err(refusal);
    }
    let mut truncated_state_tokens = 0;
    if fixed + state_ids.len() > budget {
        if request.truncation != Truncation::StateTail {
            return Err(refusal);
        }
        truncated_state_tokens = fixed + state_ids.len() - budget;
        state_ids.truncate(budget - fixed);
    }
    let offset = prefix_ids.len() + state_ids.len();
    for question in &mut questions {
        question.question_span.0 += offset;
        question.question_span.1 += offset;
        for span in &mut question.option_spans {
            span.0 += offset;
            span.1 += offset;
        }
    }
    let mut input_ids = prefix_ids;
    input_ids.extend(state_ids);
    input_ids.extend(schema_ids);
    input_ids.extend(suffix_ids);
    Ok(EncodedRecord {
        input_ids,
        questions,
        truncated_state_tokens,
    })
}
