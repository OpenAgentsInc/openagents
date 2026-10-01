//! Typed task client shared by device links and the local owner broker.
use super::*;

/// Task titles follow the first nonempty line, with controls replaced and
/// at most 80 characters and 200 bytes, across every client transport.
pub fn title(prompt: &str) -> String {
    let line = prompt
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .unwrap_or("Task");
    let title: String = line
        .chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .take(80)
        .scan(0, |bytes, c| {
            *bytes += c.len_utf8();
            (*bytes <= 200).then_some(c)
        })
        .collect();
    let title = title.trim();
    if title.is_empty() {
        "Task".into()
    } else {
        title.into()
    }
}
pub fn input(prompt: &str, workspace: &str) -> TaskCreate {
    TaskCreate {
        title: title(prompt),
        prompt: prompt.into(),
        workspace: workspace.into(),
        images: Vec::new(),
        engine: None,
    }
}

/// Transport supplies an admitted operation outcome. It owns credentials,
/// exact request retries, and the network or local connection.
pub struct Tasks<F>(F);
impl<F: FnMut(Operation) -> Result<Outcome>> Tasks<F> {
    pub fn new(call: F) -> Self {
        Self(call)
    }
    pub fn create(&mut self, task: TaskCreate) -> Result<String> {
        self.dispatch(Operation::CreateTask { task })
    }
    pub fn cancel(&mut self, task: &str, revision: u64, reason: &str) -> Result<()> {
        self.dispatch(Operation::CancelTask {
            task: task.into(),
            revision,
            reason: reason.into(),
        })
        .map(|_| ())
    }
    pub fn command(&mut self, command: TaskCommand) -> Result<()> {
        self.dispatch(Operation::CommandTask { command })
            .map(|_| ())
    }
    pub fn dispatch(&mut self, operation: Operation) -> Result<String> {
        operation.validate()?;
        let outcome = (self.0)(operation.clone())?;
        if !outcome.answers(&operation) {
            return fail(Code::Malformed, "the host answered another task operation");
        }
        outcome.validate()?;
        match outcome {
            Outcome::Dispatched { receipt } => Ok(receipt.reference),
            _ => fail(
                Code::Malformed,
                "the host did not dispatch the task operation",
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn every_transport_uses_a_valid_unicode_title() {
        let prompt = format!("\n\n{}\nBody", "長".repeat(80));
        let task = input(&prompt, "openagents");
        assert_eq!(task.title.len(), 198);
        Operation::CreateTask { task }.validate().unwrap();
        assert_eq!(title("\n  \n"), "Task");
        assert_eq!(title("  Fix\ttest\nDetails"), "Fix test");
    }

    /// The engine the person asked for is an additive, closed field: a
    /// create without one encodes exactly as before it existed, and a word
    /// outside the closed set is refused (#10081).
    #[test]
    fn a_requested_engine_is_additive_and_closed() {
        use nostr::cj_conversation::Engine;
        let plain = input("Fix the parser", "openagents");
        let bytes = serde_json::to_value(&plain).unwrap();
        assert_eq!(
            bytes,
            serde_json::json!({"title": "Fix the parser", "prompt": "Fix the parser", "workspace": "openagents"})
        );
        let mut asked = plain.clone();
        asked.engine = Some(Engine::ClaudeCode);
        let value = serde_json::to_value(&asked).unwrap();
        assert_eq!(value["engine"], "claude_code");
        assert_eq!(
            serde_json::from_value::<TaskCreate>(value.clone()).unwrap(),
            asked
        );
        Operation::CreateTask { task: asked }.validate().unwrap();
        let mut unknown = value;
        unknown["engine"] = serde_json::json!("cursor");
        assert!(serde_json::from_value::<TaskCreate>(unknown).is_err());
    }
}
