//! Exact outbound inputs admitted by the host, scoped to one live binding.

use super::*;
use serde_json::{Value, json};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

const MAX_ADMISSIONS: usize = 32;
const ADMISSION_MS: u64 = 300_000;

pub(super) struct Admission {
    reference: String,
    command: Command,
    recipient: String,
    generation: u64,
    expires_at_ms: u64,
    subjects: Vec<String>,
}

/// A registered native snapshot. Its retired binding never becomes enabled again.
#[derive(Clone)]
pub struct Native {
    pub(super) generation: u64,
    pub(super) origin: String,
    pub(super) binding: Arc<Binding>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Search {
    query: String,
    #[serde(default)]
    input_ref: Option<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Rank {
    pubkeys: Vec<String>,
    #[serde(default)]
    input_ref: Option<String>,
}

pub(crate) fn tool_definition(rank: bool) -> Value {
    let (name, description, properties, required) = if rank {
        (
            "brainstorm_rank",
            "Read Brainstorm house influence for approved public keys. An input_ref may name the exact approved rank input or a fresh approved search that returned all requested keys. Without one, the host asks the owner to confirm the exact keys and recipient.",
            json!({"pubkeys":{"type":"array","minItems":1,"maxItems":20,"uniqueItems":true,"items":{"type":"string"}},"input_ref":{"type":"string","maxLength":80}}),
            json!(["pubkeys"]),
        )
    } else {
        (
            "brainstorm_search_people",
            "Search public Nostr profiles in Brainstorm house perspective. The host asks the owner to confirm the exact query and configured recipient. Reuse an input_ref only with its identical query. No files or conversation are added.",
            json!({"query":{"type":"string","minLength":1,"maxLength":512},"input_ref":{"type":"string","maxLength":80}}),
            json!(["query"]),
        )
    };
    json!({"type":"function","function":{"name":name,"description":description,
        "parameters":{"type":"object","properties":properties,"required":required,"additionalProperties":false}}})
}

pub(super) fn validate(command: Command) -> Result<Command, String> {
    match command {
        Command::Search(query)
            if !query.trim().is_empty() && query.chars().count() <= 512 && query.len() <= 1024 =>
        {
            Ok(Command::Search(query))
        }
        Command::Search(_) => {
            Err("Enter a public query using at most 512 characters and 1 KiB.".into())
        }
        Command::Rank(keys) if !keys.is_empty() && keys.len() <= 20 => {
            let mut found = HashSet::new();
            let mut canonical = Vec::new();
            for key in keys {
                let key = public_key(&key).ok_or("Enter a valid public hex key or npub. Secret keys and profile URLs are refused.")?;
                if !found.insert(key.clone()) {
                    return Err("Each rank subject must be a different public key.".into());
                }
                canonical.push(key);
            }
            Ok(Command::Rank(canonical))
        }
        Command::Rank(_) => Err("Rank between 1 and 20 public keys.".into()),
        Command::Test => Ok(Command::Test),
    }
}

pub(super) fn admit(binding: &Binding, command: Command) -> Result<String, String> {
    if !*binding.enabled.borrow() {
        return Err("Brainstorm is disabled.".into());
    }
    let command = validate(command)?;
    let mut random = [0; 32];
    getrandom::fill(&mut random)
        .map_err(|_| "The host could not create a disclosure reference.")?;
    let reference: String = random.iter().map(|byte| format!("{byte:02x}")).collect();
    let now = atif::now_ms();
    let mut admissions = binding
        .admissions
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    admissions.retain(|admission| admission.expires_at_ms > now);
    while admissions.len() >= MAX_ADMISSIONS {
        admissions.pop_front();
    }
    admissions.push_back(Admission {
        reference: reference.clone(),
        command,
        recipient: binding.origin.clone(),
        generation: binding.generation,
        expires_at_ms: now.saturating_add(ADMISSION_MS),
        subjects: Vec::new(),
    });
    Ok(reference)
}

pub(super) fn check(binding: &Binding, reference: &str, command: &Command) -> Result<(), String> {
    validate(command.clone())?;
    if !*binding.enabled.borrow() {
        return Err("Brainstorm is disabled; this registration has retired.".into());
    }
    if reference.len() != 64 {
        return Err("The Brainstorm input reference is invalid.".into());
    }
    let mut admissions = binding
        .admissions
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    admissions.retain(|record| record.expires_at_ms > atif::now_ms());
    let record = admissions
        .iter()
        .find(|record| {
            record.reference == reference
                && record.expires_at_ms > atif::now_ms()
                && record.recipient == binding.origin
                && record.generation == binding.generation
        })
        .ok_or(
            "The Brainstorm input reference is unknown or expired. Admit the exact input again.",
        )?;
    let matches = record.command == *command
        || matches!((&record.command, command), (Command::Search(_), Command::Rank(keys))
        if !record.subjects.is_empty() && keys.iter().all(|key| record.subjects.contains(key)));
    if !matches {
        return Err(
            "This input differs from the approved input or the public keys it returned.".into(),
        );
    }
    Ok(())
}

pub(super) fn retain_subjects(binding: &Binding, reference: &str, observation: &Observation) {
    let mut admissions = binding
        .admissions
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    if let Some(record) = admissions
        .iter_mut()
        .find(|record| record.reference == reference)
    {
        // A rank of search subjects does not replace the original search's provenance.
        if matches!(record.command, Command::Search(_))
            && observation.operation == brainstorm_client::Operation::SearchPeople
        {
            record.subjects = observation
                .subjects
                .iter()
                .take(10)
                .filter_map(|subject| public_key(&subject.pubkey))
                .collect();
            record.expires_at_ms = record.expires_at_ms.min(observation.expires_at_ms);
        }
    }
}

impl Native {
    pub fn available(&self) -> bool {
        *self.binding.enabled.borrow()
            && self.origin == self.binding.origin
            && self.generation == self.binding.generation
    }

    pub async fn execute(
        &self,
        name: &str,
        arguments: Value,
        desk: Option<&crate::approval::Desk>,
        cancel: &Arc<AtomicBool>,
    ) -> Result<Value, String> {
        if cancel.load(Ordering::Relaxed) || !self.available() {
            return Err("Brainstorm is disabled or canceled.".into());
        }
        let (command, reference) = match name {
            "brainstorm_search_people" => {
                let args: Search = serde_json::from_value(arguments)
                    .map_err(|_| "Brainstorm search requires only query and optional input_ref.")?;
                (Command::Search(args.query), args.input_ref)
            }
            "brainstorm_rank" => {
                let args: Rank = serde_json::from_value(arguments)
                    .map_err(|_| "Brainstorm rank requires only pubkeys and optional input_ref.")?;
                (Command::Rank(args.pubkeys), args.input_ref)
            }
            _ => return Err("This Brainstorm native operation is unavailable.".into()),
        };
        let command = validate(command)?;
        let reference = if let Some(reference) = reference {
            check(&self.binding, &reference, &command)?;
            reference
        } else {
            let desk = desk.ok_or("Brainstorm needs the owner's exact disclosure confirmation. Use /brainstorm or a chat with an approval desk.")?;
            if !desk
                .disclose(&self.origin, command.input(&self.origin), cancel, || {
                    self.available()
                })
                .await
            {
                return Err("The owner did not approve this exact Brainstorm disclosure. Continue without it; do not work around the refusal.".into());
            }
            if cancel.load(Ordering::Relaxed) || !self.available() {
                return Err("Brainstorm is disabled or canceled.".into());
            }
            admit(&self.binding, command.clone())?
        };
        // Check the immutable host-held input again immediately before the shared client.
        check(&self.binding, &reference, &command)?;
        let job = Job {
            generation: self.generation,
            command,
            origin: self.origin.clone(),
            cancellation: Cancellation::default(),
            binding: self.binding.clone(),
            input_ref: Some(reference.clone()),
        };
        let canceled = async {
            loop {
                if cancel.load(Ordering::Relaxed) {
                    break;
                }
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        };
        let result = tokio::select! {
            biased;
            _ = canceled => { job.cancellation.cancel(); Err(Error::Cancelled) },
            result = job.run() => result,
        };
        let mut value = match result {
            Ok(outcome) => output(outcome),
            Err(error) => {
                json!({"error":error.to_string(),"state":error,"recipient":self.origin,"operation":job.command.name(),"completed_at_ms":atif::now_ms()})
            }
        };
        value["input_ref"] = json!(reference);
        if value.to_string().len() > Limits::default().normalized_bytes {
            return Ok(
                json!({"error":Error::OutputTooLarge.to_string(),"state":Error::OutputTooLarge,
                "recipient":self.origin,"operation":job.command.name(),"completed_at_ms":atif::now_ms()}),
            );
        }
        Ok(value)
    }
}

#[cfg(test)]
mod tests;
