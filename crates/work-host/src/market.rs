//! LAB remains the owner of commercial admission, checks, and payment evidence.
use crate::Result;
use serde_json::Value;
/// An embedding host mounts its actual admitted LAB book. Wire declarations
/// cannot choose or construct this port.
pub trait Market {
    fn disclosure(&self, actor: &str, now: u64) -> Result<()>;
    fn evidence(&self, reference: &Value, frame: &Value, acceptance: &Value) -> Result<()>;
    fn execution_grant(&self, grant: &Value, order: &Value, until: u64) -> Result<()>;
    fn reassign(&self, order: &Value) -> Result<()>;
    fn frame(&self, frame: &Value, acceptance: &Value) -> Result<()>;
    fn delegation(
        &self,
        order: &Value,
        frame: &Value,
        acceptance: &Value,
        worker: &str,
        links: &[Value],
    ) -> Result<Value>;
    fn acceptance(
        &self,
        order: &Value,
        frame: &Value,
        acceptance: &Value,
        worker: &str,
        evidence: &Value,
    ) -> Result<Value>;
}
