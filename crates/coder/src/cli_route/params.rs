//! Filling a chosen command's parameters, and checking the result.
//!
//! Each positional and option of the command's form is one [`Param`]. A
//! value the device can enumerate is selected, never generated: a `HOST`
//! from the device's own computers, a `--workspace LABEL` from that
//! host's workspaces, and an enum from the usage line
//! (`--to all|ads|zone`) are each a Jev Choice over candidates code lists
//! ([`selection`]). Free text (a search string, a key, a count) is written
//! by the model into a JSON object whose keys code derives from the form
//! ([`fill_instructions`], [`read_fill`]). The assembled command is then
//! checked by [`validate`] with the command's own argument parser
//! ([`crate::argv::Args`]) against every form the help lists. A missing
//! required value is reported, never guessed.

use std::collections::BTreeMap;

use indexmap::IndexMap;
use jev::{Answer, Choice, Entry, Questions, SystemOneResponse};
use serde_json::{Map, Value, json};

use super::tree::{Leaf, Node};
use super::usage::{Form, Token};
use crate::argv::Args;

/// Metavars whose value is a whole number.
pub const NUMERIC: &[&str] = &["N", "SECONDS", "S", "F", "UNIX", "DAYS"];

/// The least probability at which a selected value is used.
pub const SELECT_CONFIDENCE: f64 = 0.5;

/// The longest free-text value a fill may carry.
pub const MAX_VALUE_BYTES: usize = 400;

/// How a parameter's value is found.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Kind {
    /// One of the device's computers.
    Host,
    /// One of the chosen host's workspace labels.
    Workspace,
    /// One of the values the usage line lists.
    Enum(Vec<String>),
    /// A `--switch`, on or off.
    Switch,
    /// Written by the model; `numeric` when the metavar is a count.
    Text { numeric: bool },
    /// `-- CMD [ARGS...]`, which the router never writes.
    Rest,
}

/// One positional or option of a form.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Param {
    /// The fill key: the metavar for a positional (`HOST`), `--name` for
    /// an option.
    pub key: String,
    /// What the help calls its value (`SECONDS`), or the switch's name.
    pub metavar: String,
    pub kind: Kind,
    pub required: bool,
    pub repeated: bool,
}

/// The parameters of `form`, in order.
#[must_use]
pub fn params(form: &Form) -> Vec<Param> {
    let mut out: Vec<Param> = Vec::new();
    for token in &form.tokens {
        let param = match token {
            Token::Literal { .. } => continue,
            Token::Positional {
                name,
                optional,
                repeated,
            } => Param {
                key: name.clone(),
                metavar: name.clone(),
                kind: if name == "HOST" {
                    Kind::Host
                } else {
                    Kind::Text {
                        numeric: NUMERIC.contains(&name.as_str()),
                    }
                },
                required: !optional,
                repeated: *repeated,
            },
            Token::Option {
                name,
                value,
                choices,
                open,
                optional,
                repeated,
            } => Param {
                key: format!("--{name}"),
                metavar: value.clone().unwrap_or_else(|| name.clone()),
                kind: match value.as_deref() {
                    None => Kind::Switch,
                    Some("HOST") => Kind::Host,
                    Some("LABEL") if name == "workspace" => Kind::Workspace,
                    Some(_) if !open && !choices.is_empty() => Kind::Enum(choices.clone()),
                    Some(value) => Kind::Text {
                        numeric: NUMERIC.contains(&value),
                    },
                },
                required: !optional,
                repeated: *repeated,
            },
            Token::Rest { optional } => Param {
                key: "CMD".to_string(),
                metavar: "CMD".to_string(),
                kind: Kind::Rest,
                required: !optional,
                repeated: true,
            },
        };
        // A form may name the same option twice (alternatives); keep one.
        if !out.iter().any(|seen| seen.key == param.key) {
            out.push(param);
        }
    }
    out
}

/// A filled value.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Filled {
    Word(String),
    Words(Vec<String>),
    On,
}

/// Values by [`Param::key`].
pub type Values = BTreeMap<String, Filled>;

/// One of the device's computers, as the caller knows it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Host {
    /// What the command takes as `HOST`: an alias, a key, or a unique
    /// key prefix.
    pub id: String,
    /// What the person calls it.
    pub label: String,
    /// The workspace labels it accepts, when known.
    pub workspaces: Vec<String>,
}

fn option_id(index: usize) -> String {
    format!("c{index}")
}

fn none_entry(required: bool) -> Option<Entry> {
    Some(Entry::from(if required {
        "None of these is the one the user means"
    } else {
        "The user did not ask for a particular one"
    }))
}

/// The selection questions for the enumerable parameters of `form`, and
/// the candidates each question's option ids stand for. `workspaces` are
/// the chosen host's labels, when a host is already chosen; a workspace
/// question is asked only then.
#[must_use]
pub fn selection(
    leaf: &Leaf,
    form: &Form,
    hosts: &[Host],
    workspaces: Option<&[String]>,
) -> (Questions, BTreeMap<String, Vec<String>>) {
    let mut questions = Questions::new();
    let mut candidates = BTreeMap::new();
    let command = format!("{} ({})", leaf.command(), leaf.summary);
    for param in params(form) {
        let (question, values, entries): (String, Vec<String>, Vec<String>) = match &param.kind {
            Kind::Host if !hosts.is_empty() => (
                format!(
                    "We are about to run `{command}` for the user. Which of the user's computers \
                     does the user's latest message mean as its {}? If the user names none and \
                     only one computer is listed, that one.",
                    param.metavar
                ),
                hosts.iter().map(|host| host.id.clone()).collect(),
                hosts
                    .iter()
                    .map(|host| format!("The computer called {} ({})", host.label, host.id))
                    .collect(),
            ),
            Kind::Workspace => match workspaces {
                Some(labels) if !labels.is_empty() => (
                    format!(
                        "We are about to run `{command}` for the user. Which workspace on that \
                         computer does the user's latest message mean?"
                    ),
                    labels.to_vec(),
                    labels
                        .iter()
                        .map(|label| format!("The workspace labeled {label}"))
                        .collect(),
                ),
                _ => continue,
            },
            Kind::Enum(choices) => (
                format!(
                    "We are about to run `{command}` for the user. Which value of {} does the \
                     user's latest message ask for?",
                    param.key
                ),
                choices.clone(),
                choices.iter().map(|choice| format!("`{choice}`")).collect(),
            ),
            _ => continue,
        };
        let mut criteria: IndexMap<String, Option<Entry>> = entries
            .into_iter()
            .enumerate()
            .map(|(index, entry)| (option_id(index), Some(Entry::from(entry))))
            .collect();
        criteria.insert("none".to_string(), none_entry(param.required));
        questions.insert(param.key.clone(), Choice::new(question, criteria));
        candidates.insert(param.key, values);
    }
    (questions, candidates)
}

/// The values a selection response chose: each question's argmax at
/// [`SELECT_CONFIDENCE`] or above, mapped back to its candidate.
#[must_use]
pub fn selected(
    response: &SystemOneResponse,
    candidates: &BTreeMap<String, Vec<String>>,
) -> Values {
    let mut values = Values::new();
    for (key, list) in candidates {
        let Some(Answer::Choice(answer)) = response.answers.get(key) else {
            continue;
        };
        let p = answer
            .probabilities
            .get(&answer.choice)
            .copied()
            .unwrap_or(answer.confidence);
        if p < SELECT_CONFIDENCE {
            continue;
        }
        let Some(index) = answer
            .choice
            .strip_prefix('c')
            .and_then(|index| index.parse::<usize>().ok())
        else {
            continue;
        };
        if let Some(value) = list.get(index) {
            values.insert(key.clone(), Filled::Word(value.clone()));
        }
    }
    values
}

/// The parameters the model writes: free text and switches.
#[must_use]
pub fn generated(form: &Form) -> Vec<Param> {
    params(form)
        .into_iter()
        .filter(|param| matches!(param.kind, Kind::Text { .. } | Kind::Switch))
        .collect()
}

/// Whether the model is asked at all: only when a required value is free
/// text. Optional free text and switches ride along when it is; alone
/// they are left out, so a command that needs nothing typed costs no
/// model call.
#[must_use]
pub fn needs_model(form: &Form) -> bool {
    generated(form)
        .iter()
        .any(|param| param.required && matches!(param.kind, Kind::Text { .. }))
}

/// The instructions for the model's fill of `form`'s free-text
/// parameters: the command, and the exact JSON object to return.
#[must_use]
pub fn fill_instructions(leaf: &Leaf, form: &Form) -> String {
    let mut fields = Vec::new();
    for param in generated(form) {
        let shape = match param.kind {
            Kind::Switch => "true or false".to_string(),
            Kind::Text { numeric: true } => "a whole number, or null".to_string(),
            _ if param.repeated => "an array of strings, or null".to_string(),
            _ => "a string, or null".to_string(),
        };
        let need = if param.required {
            "required"
        } else {
            "only if the user asked for it"
        };
        fields.push(format!("  \"{}\": {shape} ({need})", param.key));
    }
    let usage = leaf.usage.join(" | ");
    format!(
        "We fill in the parameters of one command from the user's chat. The command is \
         `openagents {usage}`, run as `{}`: {}\n\
         Return only a JSON object with exactly these keys:\n{}\n\
         Use only what the user said. Copy their words for a search or a name; do not \
         invent keys, identifiers, paths, or numbers. Use null for anything they did not \
         give. No prose, no code fence.",
        leaf.command(),
        leaf.summary,
        fields.join("\n")
    )
}

/// Read the model's fill into values, checking each against its shape.
///
/// # Errors
///
/// Names text that is not a JSON object, or a value of the wrong type or
/// length.
pub fn read_fill(text: &str, form: &Form) -> Result<Values, String> {
    let body = text
        .trim()
        .trim_start_matches("```json")
        .trim_start_matches("```")
        .trim_end_matches("```")
        .trim();
    let start = body.find('{').ok_or("the fill is not a JSON object")?;
    let end = body.rfind('}').ok_or("the fill is not a JSON object")?;
    let object: Map<String, Value> =
        serde_json::from_str(&body[start..=end]).map_err(|e| format!("the fill: {e}"))?;
    let mut values = Values::new();
    for param in generated(form) {
        let Some(value) = object.get(&param.key) else {
            continue;
        };
        let bounded = |text: &str| -> Result<String, String> {
            let text = text.trim();
            if text.len() > MAX_VALUE_BYTES {
                return Err(format!(
                    "{} is longer than {MAX_VALUE_BYTES} bytes",
                    param.key
                ));
            }
            Ok(text.to_string())
        };
        let filled = match (&param.kind, value) {
            (_, Value::Null) => continue,
            (Kind::Switch, Value::Bool(true)) => Filled::On,
            (Kind::Switch, Value::Bool(false)) => continue,
            (Kind::Text { numeric: true }, Value::Number(number)) => {
                let n = number
                    .as_u64()
                    .ok_or_else(|| format!("{} is not a whole number", param.key))?;
                Filled::Word(n.to_string())
            }
            (Kind::Text { numeric: true }, Value::String(text)) => {
                let n: u64 = text
                    .trim()
                    .parse()
                    .map_err(|_| format!("{} is not a whole number", param.key))?;
                Filled::Word(n.to_string())
            }
            (Kind::Text { .. }, Value::String(text)) if param.repeated => {
                Filled::Words(vec![bounded(text)?])
            }
            (Kind::Text { .. }, Value::String(text)) => Filled::Word(bounded(text)?),
            (Kind::Text { .. }, Value::Array(items)) if param.repeated => Filled::Words(
                items
                    .iter()
                    .map(|item| {
                        item.as_str()
                            .ok_or_else(|| format!("{} holds a non-string", param.key))
                            .and_then(bounded)
                    })
                    .collect::<Result<_, _>>()?,
            ),
            _ => return Err(format!("{} has the wrong type", param.key)),
        };
        if matches!(&filled, Filled::Word(word) if word.is_empty()) {
            continue;
        }
        values.insert(param.key.clone(), filled);
    }
    Ok(values)
}

/// Assemble the command line (group first, no `openagents`) from `form`
/// and `values`.
///
/// # Errors
///
/// Lists the required parameters with no value.
pub fn argv(leaf: &Leaf, form: &Form, values: &Values) -> Result<Vec<String>, Vec<String>> {
    let mut out = vec![leaf.path[0].clone()];
    let mut missing = Vec::new();
    let known = params(form);
    for token in &form.tokens {
        match token {
            Token::Literal { word } => out.push(word.clone()),
            Token::Positional { name, optional, .. } => match values.get(name) {
                Some(Filled::Word(word)) => out.push(word.clone()),
                Some(Filled::Words(words)) if !words.is_empty() => {
                    out.extend(words.iter().cloned())
                }
                _ if !optional => missing.push(name.clone()),
                _ => {}
            },
            Token::Option { name, optional, .. } => {
                let key = format!("--{name}");
                if !known.iter().any(|param| param.key == key) {
                    continue;
                }
                match values.get(&key) {
                    Some(Filled::On) => out.push(key),
                    Some(Filled::Word(word)) => {
                        out.push(key);
                        out.push(word.clone());
                    }
                    Some(Filled::Words(words)) => {
                        for word in words {
                            out.push(key.clone());
                            out.push(word.clone());
                        }
                    }
                    None if !optional => missing.push(key),
                    None => {}
                }
            }
            Token::Rest { optional } => {
                if !optional {
                    missing.push("CMD".to_string());
                }
            }
        }
    }
    if missing.is_empty() {
        Ok(out)
    } else {
        missing.dedup();
        Err(missing)
    }
}

fn switches_of<'a>(form: &'a Form, group: &'a Node) -> Vec<&'a str> {
    let valued = |name: &str| {
        form.tokens
            .iter()
            .any(|t| matches!(t, Token::Option { name: n, value: Some(_), .. } if n == name))
    };
    form.tokens
        .iter()
        .chain(group.options.iter())
        .filter_map(|token| match token {
            Token::Option {
                name, value: None, ..
            } if !valued(name) => Some(name.as_str()),
            _ => None,
        })
        .collect()
}

fn check_form(form: &Form, group: &Node, words: &[String]) -> Result<(), String> {
    let switches = switches_of(form, group);
    let args = Args::parse(words, &switches)?;
    let mut positional = args.positional().iter().peekable();
    let mut rest = false;
    for token in &form.tokens {
        match token {
            Token::Literal { word } => match positional.next() {
                Some(given) if given == word => {}
                Some(given) => return Err(format!("expected `{word}`, not `{given}`")),
                None => return Err(format!("`{word}` is missing")),
            },
            Token::Positional {
                name,
                optional,
                repeated,
            } => {
                if positional.peek().is_none() {
                    if !optional {
                        return Err(format!("{name} is required"));
                    }
                    continue;
                }
                positional.next();
                if *repeated {
                    while positional.peek().is_some() {
                        positional.next();
                    }
                }
            }
            Token::Option { .. } => {}
            Token::Rest { optional } => {
                rest = true;
                if !optional && positional.peek().is_none() {
                    return Err("CMD is required".to_string());
                }
                while positional.next().is_some() {}
            }
        }
    }
    if !rest && let Some(extra) = positional.next() {
        return Err(format!("`{extra}` is not part of this command"));
    }
    let option = |name: &str| {
        form.tokens
            .iter()
            .chain(group.options.iter())
            .find(|token| matches!(token, Token::Option { name: n, .. } if n == name))
    };
    for name in args.option_names() {
        let Some(Token::Option {
            value: Some(metavar),
            choices,
            open,
            ..
        }) = option(name)
        else {
            return Err(format!("--{name} is not an option of this command"));
        };
        for value in args.options(name) {
            if NUMERIC.contains(&metavar.as_str()) && value.parse::<u64>().is_err() {
                return Err(format!("--{name} takes a whole number, not `{value}`"));
            }
            if !open && !choices.is_empty() && !choices.iter().any(|choice| choice == value) {
                return Err(format!("--{name} takes one of {}", choices.join(", ")));
            }
        }
    }
    for switch in args.switches() {
        if option(switch).is_none() {
            return Err(format!("--{switch} is not an option of this command"));
        }
    }
    for token in &form.tokens {
        if let Token::Option {
            name,
            optional: false,
            value,
            ..
        } = token
        {
            let given = if value.is_some() {
                args.option(name).is_some()
            } else {
                args.switch(name)
            };
            if !given {
                return Err(format!("--{name} is required"));
            }
        }
    }
    Ok(())
}

/// Check `argv` (group first, under the tree's names or the wire's,
/// [`super::tree::WIRE_NAMES`]) against every form of `leaf`, with the
/// command's own argument parser: the words match, every required
/// positional and option is present, nothing unknown is given, counts are
/// numbers, and enum values are ones the usage lists.
///
/// # Errors
///
/// The first form's refusal when no form accepts it.
pub fn validate(leaf: &Leaf, group: &Node, argv: &[String]) -> Result<(), String> {
    let argv = super::tree::tree_argv(argv);
    let Some((first, words)) = argv.split_first() else {
        return Err("the command is empty".to_string());
    };
    if Some(first) != leaf.path.first() {
        return Err(format!("`{first}` is not `{}`", leaf.path[0]));
    }
    let mut first_error = None;
    for form in &leaf.forms {
        match check_form(form, group, words) {
            Ok(()) => return Ok(()),
            Err(error) => {
                first_error.get_or_insert(error);
            }
        }
    }
    Err(first_error.unwrap_or_else(|| "the command has no form".to_string()))
}

/// The JSON schema of the object [`fill_instructions`] asks for, for a
/// door that takes a response schema.
#[must_use]
pub fn fill_schema(form: &Form) -> Value {
    let mut properties = Map::new();
    for param in generated(form) {
        properties.insert(
            param.key.clone(),
            match param.kind {
                Kind::Switch => json!({ "type": ["boolean", "null"] }),
                Kind::Text { numeric: true } => json!({ "type": ["integer", "null"] }),
                _ if param.repeated => {
                    json!({ "type": ["array", "null"], "items": { "type": "string" } })
                }
                _ => json!({ "type": ["string", "null"] }),
            },
        );
    }
    json!({ "type": "object", "properties": properties, "additionalProperties": false })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli_route::tree::bundled;

    fn leaf(path: &str) -> (&'static Leaf, &'static Node) {
        let words: Vec<String> = path.split(' ').map(str::to_owned).collect();
        (
            bundled().leaf(&words).unwrap(),
            bundled().group(&words[0]).unwrap(),
        )
    }

    fn argv_of(text: &str) -> Vec<String> {
        text.split(' ').map(str::to_owned).collect()
    }

    #[test]
    fn classifies_hosts_enums_switches_and_text() {
        let (show, _) = leaf("computer show");
        assert_eq!(params(&show.forms[0])[0].kind, Kind::Host);
        let (search, _) = leaf("kb search");
        let kinds: Vec<Kind> = params(&search.forms[0])
            .into_iter()
            .map(|p| p.kind)
            .collect();
        assert_eq!(
            kinds,
            [
                Kind::Text { numeric: false },
                Kind::Text { numeric: false },
                Kind::Switch,
                Kind::Text { numeric: true }
            ]
        );
        assert!(needs_model(&search.forms[0]));
        let (who, _) = leaf("verse who");
        assert!(!needs_model(&who.forms[0]));
        let (say, _) = leaf("verse say");
        assert!(matches!(&params(&say.forms[0])[1].kind, Kind::Enum(values) if values.len() == 5));
        let (task, _) = leaf("computer task");
        assert!(
            params(&task.forms[0])
                .iter()
                .any(|p| p.kind == Kind::Workspace)
        );
    }

    #[test]
    fn validates_with_the_commands_own_parser() {
        let (search, kb) = leaf("kb search");
        assert!(validate(search, kb, &argv_of("kb search docker --limit 5")).is_ok());
        assert!(validate(search, kb, &argv_of("kb search")).is_err());
        assert!(validate(search, kb, &argv_of("kb search docker --limit five")).is_err());
        assert!(validate(search, kb, &argv_of("kb search docker --bogus x")).is_err());
        assert!(validate(search, kb, &argv_of("kb search docker cp")).is_err());
        let (list, computer) = leaf("computer list");
        assert!(validate(list, computer, &argv_of("computer list")).is_ok());
        assert!(validate(list, computer, &argv_of("computer list --same-machine")).is_ok());
        assert!(validate(list, computer, &argv_of("computer show")).is_err());
        let (say, verse) = leaf("verse say");
        assert!(validate(say, verse, &argv_of("verse say hi --to near")).is_ok());
        assert!(validate(say, verse, &argv_of("verse say hi --to everyone")).is_err());
        let (describe, cap) = leaf("cap describe");
        assert!(validate(describe, cap, &argv_of("cap describe --author abc slug")).is_ok());
        let (control, _) = leaf("verse control move");
        assert!(validate(control, verse, &argv_of("verse control e1 move 1,2,3")).is_ok());
        let (exec, _) = leaf("computer exec");
        assert!(validate(exec, computer, &argv_of("computer exec box -- ls -la")).is_ok());
        assert!(validate(exec, computer, &argv_of("computer exec box")).is_err());
    }

    /// A renamed command validates under its wire names too (#10089).
    #[test]
    fn validates_a_renamed_command_under_either_name() {
        let (list, plugin) = leaf("plugin list");
        assert!(validate(list, plugin, &argv_of("plugin list --limit 5")).is_ok());
        assert!(validate(list, plugin, &argv_of("ext list --limit 5")).is_ok());
        assert!(validate(list, plugin, &argv_of("cap list")).is_err());
        let (run, _) = leaf("plugin test run");
        assert!(validate(run, plugin, &argv_of("ext eval run DIR --trust")).is_ok());
        assert!(validate(run, plugin, &argv_of("plugin test run DIR --trust")).is_ok());
    }

    #[test]
    fn assembles_and_reports_what_is_missing() {
        let (search, kb) = leaf("kb search");
        let form = &search.forms[0];
        let values = read_fill(
            r#"{"TEXT": "docker cp", "--limit": 3, "--lexical": false}"#,
            form,
        )
        .unwrap();
        let argv = argv(search, form, &values).unwrap();
        assert_eq!(argv, ["kb", "search", "docker cp", "--limit", "3"]);
        assert!(validate(search, kb, &argv).is_ok());
        assert_eq!(
            super::argv(search, form, &Values::new()),
            Err(vec!["TEXT".to_string()])
        );
        assert!(read_fill(r#"{"--limit": "many"}"#, form).is_err());
        assert!(read_fill("no json here", form).is_err());
        let fenced = read_fill("```json\n{\"TEXT\": \"relay\"}\n```", form).unwrap();
        assert_eq!(fenced.get("TEXT"), Some(&Filled::Word("relay".into())));
    }

    #[test]
    fn selection_asks_only_for_enumerable_values() {
        let (show, _) = leaf("computer show");
        let hosts = [Host {
            id: "studio".into(),
            label: "Mac Studio".into(),
            workspaces: vec![],
        }];
        let (questions, candidates) = selection(show, &show.forms[0], &hosts, None);
        assert_eq!(questions.len(), 1);
        assert!(questions.validate().is_ok());
        assert_eq!(candidates["HOST"], ["studio"]);
        let (search, _) = leaf("kb search");
        assert!(
            selection(search, &search.forms[0], &hosts, None)
                .0
                .is_empty()
        );
        let (_, none) = selection(show, &show.forms[0], &[], None);
        assert!(none.is_empty());
    }
}
