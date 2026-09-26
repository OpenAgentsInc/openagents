//! Explicit local closure admission, independent from commercial agreement.
use crate::*;
use nostr::contracts::{DefinitionRef, parse_context, parse_definition, parse_lock};
use nostr::market_contracts::labor::{
    AcceptancePolicy, ClosureAdmission, LaborTerms, Parties, Rights,
};

/// An operator-selected contract set. Neither relay content nor a seller can
/// choose the host's target, checker, source capture, or disclosure recipients.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Admission {
    pub target: Value,
    pub checker: Value,
    pub task_frame: Value,
    pub input: Value,
    pub context: Value,
    pub requirements: Value,
    pub rights: Value,
    pub blobs: Blobs,
}
impl ClosureAdmission for Admission {
    fn resolve(&self, reference: &ArtifactRef) -> std::result::Result<Vec<u8>, ContractError> {
        jcs(self.blobs.resolve(reference).map_err(contract)?)
    }
    fn check(
        &self,
        parties: &Parties,
        terms: &LaborTerms,
        checker: &AcceptancePolicy,
        rights: &Rights,
        capability: Option<&DefinitionRef>,
    ) -> std::result::Result<(), ContractError> {
        self.check_inner(parties, terms, checker, rights, capability)
            .map_err(contract)
    }
}
impl Admission {
    fn check_inner(
        &self,
        parties: &Parties,
        terms: &LaborTerms,
        checker: &AcceptancePolicy,
        rights: &Rights,
        capability: Option<&DefinitionRef>,
    ) -> Result<()> {
        let target = parse_definition(&self.target).map_err(|e| e.to_string())?;
        let expected_checker = parse_definition(&self.checker).map_err(|e| e.to_string())?;
        if target != terms.execution.target
            || checker.checker != expected_checker
            || capability.is_some_and(|cap| cap != &target)
            || artifact_value(&terms.task_frame) != self.task_frame
            || artifact_value(&terms.execution.input) != self.input
            || artifact_value(&terms.execution.context) != self.context
            || artifact_value(&terms.execution.requirements) != self.requirements
            || artifact_value(&terms.rights) != self.rights
        {
            return Err("labor closure differs from the operator's admitted pins".into());
        }
        for (definition, lock_ref) in [
            (&target, &terms.execution.lock),
            (&expected_checker, &checker.lock),
        ] {
            self.blobs.resolve(&definition.artifact)?;
            let lock = parse_lock(self.blobs.resolve(lock_ref)?).map_err(|e| e.to_string())?;
            if lock.root != definition.id
                || lock.entries.len() != 1
                || lock.entries[0].definition != *definition
                || !lock.entries[0].dependencies.is_empty()
            {
                return Err(
                    "this host supports only its explicitly admitted single-component locks".into(),
                );
            }
        }
        let frame = self.blobs.get(&self.task_frame)?;
        exact(
            frame,
            "openagents.task-frame.v1",
            &[
                "task",
                "owner",
                "controller",
                "revision",
                "previous",
                "objective",
                "origin",
                "constraints",
                "acceptance",
                "snapshot",
                "instructions",
                "variables",
                "attempts",
                "unresolved",
                "corrections",
            ],
        )?;
        if frame["owner"] != parties.buyer
            || frame["controller"] != parties.buyer
            || frame["revision"] != 0
            || frame["previous"] != Value::Null
            || frame["origin"] != "user"
            || frame["variables"] != json!([])
            || frame["attempts"] != json!([])
            || frame["unresolved"] != json!([])
            || frame["corrections"] != json!([])
        {
            return Err("this labor host requires an original buyer-owned frozen frame".into());
        }
        for field in ["objective", "snapshot", "instructions"] {
            self.blobs.get(&frame[field])?;
        }
        for field in ["constraints", "acceptance"] {
            for r in frame[field].as_array().ok_or("task frame reference list")? {
                self.blobs.get(r)?;
            }
        }
        let snapshot = self.blobs.get(&frame["snapshot"])?;
        exact(
            snapshot,
            "openagents.snapshot.v1",
            &["scope", "captured_at", "sources", "coverage"],
        )?;
        let sources = snapshot["sources"].as_array().ok_or("snapshot sources")?;
        if snapshot["scope"] != "labor-workspace"
            || snapshot["coverage"] != "complete"
            || snapshot["captured_at"].as_u64().is_none()
            || sources.len() != 1
            || sources[0]["id"] != "workspace"
            || sources[0]["kind"] != "repository"
            || sources[0]["availability"] != "retained"
        {
            return Err("unsupported labor source scope or coverage".into());
        }
        let source = self.blobs.get(&sources[0]["version"])?;
        exact(
            source,
            "coder.free-labor.source.v1",
            &["input", "revision", "snapshot"],
        )?;
        let input = self.blobs.get(&self.input)?;
        if source["input"] != self.input
            || source["snapshot"] != input["source_snapshot"]
            || source["revision"]
                .as_str()
                .is_none_or(|r| r.len() != 40 || !r.bytes().all(|b| b.is_ascii_hexdigit()))
            || (!input["intent"].is_null()
                && source["revision"] != input["intent"]["workspace"]["source_revision"])
        {
            return Err("labor snapshot differs from its pinned source input".into());
        }
        let instructions = self.blobs.get(&frame["instructions"])?;
        exact(
            instructions,
            "openagents.instructions.v1",
            &[
                "task",
                "revision",
                "resolver",
                "precedence",
                "snapshot",
                "entries",
            ],
        )?;
        let resolver = parse_definition(&instructions["resolver"]).map_err(|e| e.to_string())?;
        let resolver_body = self.blobs.resolve(&resolver.artifact)?;
        let precedence = self.blobs.get(&instructions["precedence"])?;
        if instructions["task"] != frame["task"]
            || instructions["revision"] != frame["revision"]
            || instructions["snapshot"] != frame["snapshot"]
            || instructions["entries"] != json!([])
            || resolver_body["id"] != resolver.id
            || resolver_body["binding_contract"]["operation"]
                != "coder.free-labor.empty-instructions.v1"
            || precedence
                != &json!({"v":"coder.free-labor.empty-instructions.v1","requires":[],"rule":"explicit-command-only"})
        {
            return Err("this host supports only its pinned empty instruction profile".into());
        }
        let context_body = self.blobs.get(&self.context)?;
        let context = parse_context(context_body).map_err(|e| e.to_string())?;
        let disclosure = self.blobs.resolve(&context.policy)?;
        if disclosure
            != &json!({"v":"coder.free-labor.disclosure.v1","requires":[],"buyer":parties.buyer,"worker":parties.worker,"mode":"trusted-explicit-command"})
            || context_body["task"] != frame["task"]
            || !context.entries.is_empty()
            || !context.omissions.is_empty()
        {
            return Err("unsupported labor disclosure or context profile".into());
        }
        if context.recipient != parties.worker {
            return Err("labor context has a different worker recipient".into());
        }
        self.blobs.get(&self.input)?;
        self.blobs.get(&self.requirements)?;
        self.blobs.resolve(&rights.license)?;
        for d in &terms.deliverables {
            self.blobs.resolve(&d.schema.0)?;
        }
        // The first operational lane supports no rework. A rework request remains
        // a retained refusal rather than extending a grant or moving a deadline.
        if terms.max_reworks != 0 {
            return Err("this host does not admit executable rework".into());
        }
        Ok(())
    }
}
