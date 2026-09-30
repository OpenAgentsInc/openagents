//! Typed task client shared by device links and the local owner broker.
use super::*;

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
