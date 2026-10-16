//! Local AUTO binding inspection. Remote callers must use authenticated handlers.
use crate::{Result, book::Coordinator, operations::Limits};
use nostr::contracts;
use serde_json::{Value, json};
use std::collections::BTreeMap;
impl Coordinator {
    /// Validate the exact current container and explicit controller/worker grants.
    /// This local method grants no remote digest lookup or dispatch authority.
    pub fn automation_parent(
        &self,
        plan: &Value,
        worker: &str,
        checker: &str,
        files: &BTreeMap<String, Vec<u8>>,
        now: u64,
    ) -> Result<(Value, BTreeMap<String, Vec<u8>>)> {
        let _guard = self.store.lock()?;
        let state = self.state()?;
        let id = self.scope_key(&state, &plan["coordinator"])?;
        let scope = &state.scopes[&id];
        if state.scopes.values().any(|s| {
            s.descriptor["run"] == plan["run"]
                || s.claims
                    .values()
                    .any(|l| l.input.as_ref().is_some_and(|p| p["run"] == plan["run"]))
        }) {
            return Err(
                "conflict: scheduler RUN identity already belongs to coordinator execution".into(),
            );
        }

        let controller = plan["controller"].as_str().ok_or("plan controller")?;
        if plan["owner"] != scope.owner || controller == self.authority() {
            return Err(
                "not_admitted: local scope owner and separate AUTO controller required".into(),
            );
        }
        for role in ["read", "propose", "claim", "control"] {
            self.check(scope, controller, role, now)?;
        }
        let worker_grant = self.check(scope, worker, "claim", now)?;
        self.check(scope, checker, "finding", now)?;
        if worker == checker || !scope.policy.checkers.contains(checker) {
            return Err("not_admitted: independent admitted checker required".into());
        }
        self.check_frame(scope, plan)?;
        let frame = self.store.json(&plan["frame"])?;
        if frame["owner"] != plan["owner"] {
            return Err("not_admitted: frame owner differs".into());
        }
        if plan["target"] != self.automation_operation()?
            || scope
                .targets
                .get(plan["target"]["id"].as_str().ok_or("target")?)
                != Some(&plan["target"])
        {
            return Err("cannot_enforce: exact native AUTO adapter required".into());
        }
        let controller_grant = self.check(scope, controller, "claim", now)?;
        for grant in [controller_grant, worker_grant] {
            if !grant.all
                && !grant
                    .targets
                    .contains(plan["target"]["id"].as_str().ok_or("target")?)
            {
                return Err("not_admitted: AUTO target outside current grant".into());
            }
        }
        let (parent_key, parent) = scope
            .claims
            .iter()
            .find(|(_, l)| l.body["reservation"] == plan["reservation"])
            .ok_or("not_admitted: independently admitted container required")?;
        self.current_lease(scope, parent, now)?;
        let proposal = scope
            .proposals
            .values()
            .find(|p| p.reference == parent.body["proposal"])
            .ok_or("parent proposal")?;
        if parent.actor != controller
            || proposal.body["task"] != plan["task"]
            || self.operation(&proposal.body["target"])?["binding_contract"]["operation"]
                != "coord-container"
            || proposal.body["frame"] != plan["frame"]
            || plan["expires_at"].as_u64().ok_or("expiry")?
                > parent.body["expires_at"].as_u64().ok_or("parent expiry")?
        {
            return Err("not_admitted: plan exceeds its exact current parent".into());
        }
        self.effects_subset(&plan["effects"], &proposal.body["effects"])?;
        if plan["effects"] != self.operation(&plan["target"])?["effects"] {
            return Err("cannot_enforce: AUTO data adapter requires its exact effects".into());
        }

        self.effects_subset(
            &self.operation(&plan["target"])?["effects"],
            &plan["effects"],
        )?;
        let per = Limits::parse(&plan["per_occurrence"], &scope.policy)?;
        let ceiling = Limits::parse(&proposal.body["bounds"], &scope.policy)?;
        if per.calls > ceiling.calls
            || per.wall_ms > ceiling.wall_ms
            || per.input_bytes > ceiling.input_bytes
            || per.output_bytes > ceiling.output_bytes
            || per.depth > ceiling.depth
            || per.concurrency > ceiling.concurrency
        {
            return Err("not_admitted: occurrence exceeds parent limits".into());
        }
        let roots = [
            "frame",
            "lock",
            "payload",
            "context",
            "requirements",
            "acceptance",
            "reservation",
        ]
        .map(|field| plan[field].clone());
        self.attachments(files)?;
        // An admitted local owner may supply its own bytes. The fixed scope audience
        // must still be allowed to read every evidence object in the closure.
        let closure = self.closure(scope, &scope.owner, &roots, files)?;
        let lock = self.store.json(&plan["lock"])?;
        let parsed_lock = contracts::parse_lock(&lock).map_err(|e| e.to_string())?;
        if lock["root"] != plan["target"] || parsed_lock.entries.len() != 1 {
            return Err("cannot_enforce: exact single-component AUTO lock required".into());
        }
        let context = self.store.json(&plan["context"])?;
        let parsed_context = contracts::parse_context(&context).map_err(|e| e.to_string())?;
        if context["recipient"] != worker
            || context["task"] != frame["task"]
            || context["coverage"] != "complete"
            || !parsed_context.entries.is_empty()
            || !parsed_context.omissions.is_empty()
        {
            return Err("cannot_enforce: explicit-input worker context required".into());
        }
        let mut result = BTreeMap::new();
        for root in &roots {
            self.collect_automation_bytes(root, &closure, &mut result, 0)?;
        }
        Ok((
            json!({"claim":parent.reference,"reservation":parent.body["reservation"],"task":proposal.body["task"],"policy":scope.policy,"held":crate::claims::allocated(scope,Some(parent_key)),"spent":parent.own_calls,"calls":parent.reserved_calls}),
            result,
        ))
    }
    fn collect_automation_bytes(
        &self,
        reference: &Value,
        allowed: &std::collections::BTreeSet<String>,
        result: &mut BTreeMap<String, Vec<u8>>,
        depth: usize,
    ) -> Result<()> {
        if depth > 32 || result.len() > 256 {
            return Err("artifact closure bound".into());
        }
        let parsed = contracts::parse_artifact(reference).map_err(|e| e.to_string())?;
        if !allowed.contains(&parsed.digest) || result.contains_key(&parsed.digest) {
            return Ok(());
        }
        let bytes = self.store.get(reference)?;
        result.insert(parsed.digest, bytes.clone());
        if matches!(
            parsed.media_type.as_str(),
            "application/json" | "application/schema+json"
        ) {
            let body = contracts::parse_strict(&bytes).map_err(|e| e.to_string())?;
            let mut stack = vec![&body];
            while let Some(value) = stack.pop() {
                if contracts::parse_artifact(value).is_ok() {
                    self.collect_automation_bytes(value, allowed, result, depth + 1)?;
                    continue;
                }
                match value {
                    Value::Object(fields) => stack.extend(
                        fields
                            .iter()
                            .filter(|(k, _)| k.as_str() != "meta")
                            .map(|(_, v)| v),
                    ),
                    Value::Array(items) => stack.extend(items),
                    _ => {}
                }
            }
        }
        Ok(())
    }
}
