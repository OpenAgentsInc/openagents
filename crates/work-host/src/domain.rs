//! Domain checks preserve independent requirements, evidence, and disposition.
use crate::{
    Result,
    authority::{Authority, Scope, State},
};
use nostr::{
    contracts,
    work::{self, WorkRef},
};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};
impl Authority {
    pub(crate) fn snapshot(&self, reference: &Value) -> Result<Value> {
        let body = self.store.json(reference)?;
        work::validate(&body).map_err(|e| e.to_string())?;
        if body["v"] != "openagents.work-item.v1" {
            return Err("work item snapshot required".into());
        }
        Ok(body)
    }
    pub(crate) fn head(&self, scope: &Scope, item: &str) -> Result<Option<(Value, Value, Value)>> {
        let Some(item) = scope.items.get(item) else {
            return Ok(None);
        };
        let reference = item
            .history
            .last()
            .ok_or("work history is incomplete")?
            .clone();
        let revision = self.store.json(&reference["artifact"])?;
        work::verify_revision(&reference, &revision).map_err(|e| e.to_string())?;
        let snapshot = self.snapshot(&revision["snapshot"])?;
        Ok(Some((reference, revision, snapshot)))
    }
    pub(crate) fn validate_frame(
        &self,
        state: &mut State,
        scope: &Scope,
        item: &Value,
    ) -> Result<()> {
        if item["kind"] != "work" {
            return Ok(());
        }
        let frame = self.store.json(&item["frame"])?;
        work::validate(&frame).map_err(|e| e.to_string())?;
        if frame["v"] != "openagents.task-frame.v1"
            || frame["owner"] != scope.descriptor["owner"]
            || frame["controller"] != scope.descriptor["authority"]
            || !frame["acceptance"]
                .as_array()
                .is_some_and(|a| a.contains(&item["acceptance"]))
        {
            return Err("work frame, controller, or acceptance differs".into());
        }
        let frame_key = format!(
            "{}:{}",
            frame["task"].as_str().ok_or("task ID")?,
            frame["revision"].as_u64().ok_or("frame revision")?
        );
        let digest = item["frame"]["digest"]
            .as_str()
            .ok_or("frame digest")?
            .to_string();
        if state
            .frames
            .get(&frame_key)
            .is_some_and(|known| known != &digest)
        {
            return Err("conflict: task frame revision has different bytes".into());
        }
        if item["acceptance"]["schema"] == "openagents.labor-acceptance-policy.v1" {
            self.market
                .as_ref()
                .ok_or("LAB task frame requires the admitted LAB book")?
                .frame(&item["frame"], &item["acceptance"])?;
            self.acceptance_policy(&self.store.json(&item["acceptance"])?)?;
            state.frames.insert(frame_key, digest);
            return Ok(());
        }
        if !frame["previous"].is_null() {
            let previous = self.store.json(&frame["previous"])?;
            work::validate(&previous).map_err(|e| e.to_string())?;
            if previous["task"] != frame["task"]
                || previous["owner"] != frame["owner"]
                || previous["controller"] != frame["controller"]
                || previous["revision"].as_u64().and_then(|v| v.checked_add(1))
                    != frame["revision"].as_u64()
            {
                return Err("task frame predecessor differs".into());
            }
        }
        let snapshot = self.store.json(&frame["snapshot"])?;
        work::validate(&snapshot).map_err(|e| e.to_string())?;
        if snapshot["v"] != "openagents.snapshot.v1" {
            return Err("task source snapshot required".into());
        }
        for source in snapshot["sources"].as_array().ok_or("snapshot sources")? {
            if source["availability"] != "retained" {
                continue;
            }
            let body = self.store.json(&source["version"])?;
            if source["kind"] == "external" {
                let observation = contracts::parse_observation(&body).map_err(|e| e.to_string())?;
                if observation.resource_scope != snapshot["scope"]
                    || observation.resource_id != source["id"]
                    || source["adapter"] != body["adapter"]
                {
                    return Err("task external source identity differs".into());
                }
            } else {
                return Err(
                    "unsupported task source adapter; use an admitted external observation".into(),
                );
            }
        }
        let instructions = self.store.json(&frame["instructions"])?;
        crate::authority::closed(
            &instructions,
            &[
                "v",
                "requires",
                "task",
                "revision",
                "resolver",
                "precedence",
                "snapshot",
                "entries",
            ],
        )?;
        if instructions["v"] != "openagents.instructions.v1"
            || instructions["task"] != frame["task"]
            || instructions["revision"] != frame["revision"]
            || instructions["snapshot"] != frame["snapshot"]
            || instructions["entries"] != json!([])
        {
            return Err(
                "unsupported task instruction resolution; nonempty sets need a POL adapter".into(),
            );
        }
        let precedence = self.store.json(&instructions["precedence"])?;
        if precedence != json!({"v":"openagents.work-empty-instruction-policy.v1","requires":[]}) {
            return Err("unsupported instruction precedence".into());
        }
        contracts::parse_definition(&instructions["resolver"]).map_err(|e| e.to_string())?;
        self.store.get(&frame["objective"])?;
        for reference in frame["constraints"]
            .as_array()
            .ok_or("frame constraints")?
            .iter()
            .chain(
                frame["unresolved"]
                    .as_array()
                    .ok_or("unresolved questions")?,
            )
        {
            self.store.get(reference)?;
        }
        for variable in frame["variables"].as_array().ok_or("variables")? {
            let schema = self.store.get(&variable["schema"])?;
            let value = self.store.json(&variable["value"])?;
            let closure = contracts::prepare_closure(&BTreeMap::from([(
                contracts::digest_bytes(&schema),
                schema.clone(),
            )]))
            .map_err(|e| e.to_string())?;
            contracts::validate_instance(&closure, &contracts::digest_bytes(&schema), &value)
                .map_err(|e| e.to_string())?;
            for evidence in variable["evidence"].as_array().ok_or("variable evidence")? {
                contracts::parse_evidence(&self.store.json(evidence)?)
                    .map_err(|e| e.to_string())?;
            }
        }
        if !frame["corrections"].as_array().is_some_and(Vec::is_empty) {
            return Err(
                "unsupported correction source; a POL correction adapter is required".into(),
            );
        }
        let policy = self.store.json(&item["acceptance"])?;
        self.acceptance_policy(&policy)?;
        state.frames.insert(frame_key, digest);
        Ok(())
    }
    pub(crate) fn acceptance_policy(&self, policy: &Value) -> Result<BTreeSet<String>> {
        if policy["v"] == "openagents.labor-acceptance-policy.v1" {
            let parsed = nostr::market_contracts::labor::parse_acceptance_policy(
                &contracts::jcs(policy).map_err(|e| e.to_string())?,
            )
            .map_err(|e| e.to_string())?;
            if self.market.is_none() {
                return Err("LAB acceptance policy requires the admitted LAB book".into());
            }
            return Ok(parsed.criteria.into_iter().collect());
        }
        crate::authority::closed(policy, &["v", "requires", "checks", "independent"])?;
        if policy["v"] != "openagents.work-check-policy.v1" || policy["independent"] != true {
            return Err("unsupported work acceptance policy".into());
        }
        let checks = policy["checks"]
            .as_array()
            .filter(|a| !a.is_empty() && a.len() <= 64)
            .ok_or("acceptance checks")?;
        let mut ids = BTreeSet::new();
        for check in checks {
            let check = check
                .as_str()
                .filter(|s| {
                    !s.is_empty()
                        && s.len() <= 64
                        && s.bytes().all(|b| {
                            b.is_ascii_lowercase() || b.is_ascii_digit() || matches!(b, b'_' | b'-')
                        })
                })
                .ok_or("check ID")?;
            if check != "exact-output" {
                return Err("unsupported native work check".into());
            }
            if !ids.insert(check.into()) {
                return Err("acceptance check repeated".into());
            }
        }
        Ok(ids)
    }
    pub(crate) fn relations(
        &self,
        state: &State,
        scope: &Scope,
        w: &WorkRef,
        item: &Value,
        actor: &str,
        now: u64,
    ) -> Result<()> {
        for relation in item["relations"].as_array().ok_or("relations")? {
            let target = work::work_ref(&relation["target"]).map_err(|e| e.to_string())?;
            if target == *w {
                return Err("work cannot link to itself".into());
            }
            if matches!(relation["type"].as_str(), Some("parent" | "blocks"))
                && (target.scope != w.scope || target.authority != w.authority)
            {
                return Err("planning parent/blocks must stay in one scope".into());
            }
            let target_scope = state
                .scopes
                .get(&target.scope)
                .filter(|s| s.descriptor["authority"] == target.authority)
                .ok_or("planning target scope is unavailable")?;
            self.check(target_scope, actor, Some(&target.item), "read", now)?;
            if !target_scope.items.contains_key(&target.item) {
                return Err("planning target is unavailable".into());
            }
        }
        let began = std::time::Instant::now();
        for relation_kind in ["parent", "blocks"] {
            let mut stack = vec![(w.item.clone(), false, 0usize)];
            let mut colors = BTreeMap::<String, u8>::new();
            while let Some((id, exiting, depth)) = stack.pop() {
                if began.elapsed() > std::time::Duration::from_secs(2) || depth > 256 {
                    return Err("planning graph traversal bound exceeded".into());
                }
                if exiting {
                    colors.insert(id, 2);
                    continue;
                }
                match colors.get(&id) {
                    Some(1) => return Err("planning relation cycle".into()),
                    Some(2) => continue,
                    _ => {}
                }
                if colors.len() >= 256 {
                    return Err("planning graph node limit exceeded".into());
                }
                colors.insert(id.clone(), 1);
                stack.push((id.clone(), true, depth));
                let body = if id == w.item {
                    item.clone()
                } else {
                    self.head(scope, &id)?
                        .ok_or("planning target unavailable")?
                        .2
                };
                for relation in body["relations"]
                    .as_array()
                    .ok_or("relations")?
                    .iter()
                    .filter(|r| r["type"] == relation_kind)
                {
                    let target = work::work_ref(&relation["target"]).map_err(|e| e.to_string())?;
                    if target.scope != w.scope || target.authority != w.authority {
                        return Err("planning graph scope differs".into());
                    }
                    stack.push((target.item, false, depth + 1));
                }
            }
        }
        Ok(())
    }
    pub(crate) fn changed_rights(
        &self,
        scope: &Scope,
        actor: &str,
        w: &WorkRef,
        prior: Option<&Value>,
        item: &Value,
        now: u64,
    ) -> Result<()> {
        if let Some(prior) = prior {
            for field in [
                "kind",
                "title",
                "body",
                "state",
                "relations",
                "resources",
                "labels",
                "due_at",
                "timebox",
                "meta",
            ] {
                if prior[field] != item[field] {
                    self.check(scope, actor, Some(&w.item), "revise", now)?;
                }
            }
            for field in ["frame", "acceptance"] {
                if prior[field] != item[field] {
                    self.check(scope, actor, Some(&w.item), "requirements", now)?;
                }
            }
            for field in ["accountable", "delegation"] {
                if prior[field] != item[field] {
                    self.check(scope, actor, Some(&w.item), "assign", now)?;
                }
            }
            if prior["disposition"] != item["disposition"] {
                self.check(scope, actor, Some(&w.item), "dispose", now)?;
            }
            if prior["evidence"] != item["evidence"] {
                self.check(scope, actor, Some(&w.item), "evidence", now)?;
                let next = item["evidence"].as_array().ok_or("evidence list")?;
                if prior["evidence"]
                    .as_array()
                    .ok_or("previous evidence")?
                    .iter()
                    .any(|r| !next.contains(r))
                {
                    self.check(scope, actor, Some(&w.item), "revise", now)?;
                }
            }
        } else {
            let grant = self.check(scope, actor, None, "create", now)?;
            if !grant.all || !grant.roles.contains("read") {
                return Err("creation requires scope-wide create/read admission".into());
            }
            if item["state"] != "open" || !item["disposition"].is_null() {
                return Err("new work starts open without a disposition".into());
            }
            if item["kind"] == "work" {
                self.check(scope, actor, None, "requirements", now)?;
            }
            if item["accountable"] != scope.descriptor["owner"] || !item["delegation"].is_null() {
                self.check(scope, actor, None, "assign", now)?;
            }
        }
        Ok(())
    }
    pub(crate) fn evidence(
        &self,
        state: &State,
        scope: &Scope,
        w: &WorkRef,
        item: &Value,
        expected: Option<&Value>,
        actor: &str,
        now: u64,
    ) -> Result<()> {
        for reference in item["evidence"].as_array().ok_or("work evidence")? {
            if expected.is_some_and(|prior| {
                prior["evidence"]
                    .as_array()
                    .is_some_and(|refs| refs.contains(reference))
            }) {
                continue;
            }
            let (issuer, evidence) = self.original(state, reference)?;
            work::validate(&evidence).map_err(|e| e.to_string())?;
            if evidence["v"] != "openagents.work-evidence.v1"
                || work::work_ref(&evidence["work"]).map_err(|e| e.to_string())? != *w
            {
                return Err("evidence belongs to another work item".into());
            }
            self.check(scope, &issuer, Some(&w.item), "evidence", now)?;
            let basis = self.store.json(&evidence["basis"]["artifact"])?;
            work::verify_revision(&evidence["basis"], &basis).map_err(|e| e.to_string())?;
            let retained = scope
                .items
                .get(&w.item)
                .is_some_and(|i| i.history.contains(&evidence["basis"]));
            if !retained {
                return Err("evidence basis is not retained work history".into());
            }
            self.store.get(&evidence["artifact"])?;
            if evidence["type"] == "commercial" {
                self.market
                    .as_ref()
                    .ok_or("commercial evidence requires the admitted LAB book")?
                    .evidence(&evidence["artifact"], &item["frame"], &item["acceptance"])?;
            }
            if evidence["type"] == "execution_result" {
                let found = scope.jobs.values().any(|j| {
                    j.work == *w
                        && j.basis == evidence["basis"]
                        && j.result_event.is_some()
                        && j.result.as_ref().is_some_and(|r| {
                            workspace_host::artifact::json_ref(r)
                                .is_ok_and(|reference| reference == evidence["artifact"])
                        })
                });
                if !found {
                    return Err(
                        "execution evidence must name this work's authenticated recorded CJ result"
                            .into(),
                    );
                }
            }
            if evidence["type"] == "deliverable" && item["kind"] == "work" {
                let found = scope.jobs.values().any(|j| {
                    j.work == *w
                        && j.basis == evidence["basis"]
                        && j.confirmed
                        && j.result
                            .as_ref()
                            .is_some_and(|r| r["output"] == evidence["artifact"])
                });
                if !found {
                    return Err("native work deliverables must name the actual completed job output; LAB deliverables use commercial evidence".into());
                }
            }

            for reference in evidence["sources"].as_array().ok_or("evidence sources")? {
                let (_, source) = self.original(state, reference)?;
                let observation = self.store.json(&source["assurance"])?;
                work::verify_source(&source, &observation).map_err(|e| e.to_string())?;
            }
            if evidence["type"] == "verification" {
                self.verification(state, scope, w, item, &evidence)?;
            }
            if evidence["type"] == "integration" {
                self.check(scope, &issuer, Some(&w.item), "integrate", now)?;
                let receipt = self.store.json(&evidence["artifact"])?;
                contracts::parse_receipt(&receipt).map_err(|e| e.to_string())?;
            }
        }
        let _ = actor;
        Ok(())
    }
    pub(crate) fn verification(
        &self,
        state: &State,
        scope: &Scope,
        w: &WorkRef,
        item: &Value,
        evidence: &Value,
    ) -> Result<()> {
        let (issuer, report) = self.original(state, &evidence["artifact"])?;
        crate::authority::closed(
            &report,
            &[
                "v",
                "requires",
                "issuer",
                "work",
                "basis",
                "frame",
                "acceptance",
                "checks",
                "execution",
            ],
        )?;
        if report["v"] != "openagents.work-check-report.v1"
            || work::work_ref(&report["work"]).map_err(|e| e.to_string())? != *w
            || report["basis"] != evidence["basis"]
            || report["frame"] != item["frame"]
            || report["acceptance"] != item["acceptance"]
            || !scope.policy["verifiers"]
                .as_array()
                .is_some_and(|keys| keys.iter().any(|k| *k == issuer))
        {
            return Err("verification report or admitted verifier differs".into());
        }
        let checks = self.acceptance_policy(&self.store.json(&item["acceptance"])?)?;
        let mut supplied = BTreeSet::new();
        for result in report["checks"].as_array().ok_or("check results")? {
            let object = result.as_object().ok_or("check result")?;
            if object.len() != 3
                || object
                    .keys()
                    .any(|k| !matches!(k.as_str(), "id" | "passed" | "receipt"))
            {
                return Err("unsupported check result".into());
            }
            let id = result["id"].as_str().ok_or("check ID")?;
            if !supplied.insert(id.to_string()) || result["passed"].as_bool().is_none() {
                return Err("check result duplicated or unknown".into());
            }
            let receipt = self.store.json(&result["receipt"])?;
            contracts::parse_receipt(&receipt).map_err(|e| e.to_string())?;
            if receipt["verification"]
                != if result["passed"] == true {
                    "passed"
                } else {
                    "failed"
                }
            {
                return Err("check receipt contradicts its result".into());
            }
        }
        if supplied != checks {
            return Err("verification must report the exact pinned check set".into());
        }
        let job_key = report["execution"]
            .as_str()
            .ok_or("verified execution ID")?;
        let job = scope
            .jobs
            .get(job_key)
            .ok_or("verified execution is unavailable")?;
        if job.delegate == issuer
            || job.frame != item["frame"]
            || job.work != *w
            || job.basis != report["basis"]
            || !job.confirmed
        {
            return Err(
                "verification is not independent of the matching completed execution".into(),
            );
        }
        for result in report["checks"].as_array().ok_or("check results")? {
            self.check_receipt_identity(&report, result, job, scope)?;
        }
        Ok(())
    }
}

impl Authority {
    pub(crate) fn disposition(
        &self,
        state: &State,
        scope: &Scope,
        w: &WorkRef,
        prior: Option<(&Value, &Value)>,
        item: &Value,
        actor: &str,
        now: u64,
    ) -> Result<()> {
        let same =
            prior.is_some_and(|(_, snapshot)| snapshot["disposition"] == item["disposition"]);
        let terminal = |value: &Value| {
            matches!(
                value["state"].as_str(),
                Some("resolved" | "cancelled" | "superseded" | "archived")
            )
        };
        if let Some((_, old)) = prior {
            if old["kind"] != item["kind"] {
                return Err("changing an existing planning/work kind requires a new item".into());
            }
            let requirements_changed =
                old["frame"] != item["frame"] || old["acceptance"] != item["acceptance"];
            if terminal(old)
                && (old["state"] != item["state"] || requirements_changed)
                && (item["state"] != "archived" || requirements_changed)
            {
                if item["state"] != "open" || same {
                    return Err(
                        "terminal work must be explicitly reopened before changing requirements"
                            .into(),
                    );
                }
            }
            if same {
                if old["state"] != item["state"] && terminal(item) {
                    return Err("terminal transition requires a new exact disposition".into());
                }
                return Ok(());
            }
        }
        if item["disposition"].is_null() {
            if prior.is_some_and(|(_, old)| !old["disposition"].is_null()) {
                return Err(
                    "clearing a disposition requires a new revise or reopened record".into(),
                );
            }
            return Ok(());
        }
        self.check(scope, actor, Some(&w.item), "dispose", now)?;
        let (issuer, disposition) = self.original(state, &item["disposition"])?;
        work::validate(&disposition).map_err(|e| e.to_string())?;
        self.check(scope, &issuer, Some(&w.item), "dispose", now)?;
        let (basis, old) = prior.ok_or("new work cannot start with a disposition")?;
        if disposition["v"] != "openagents.work-disposition.v1"
            || work::work_ref(&disposition["work"]).map_err(|e| e.to_string())? != *w
            || disposition["basis"] != *basis
        {
            return Err("work disposition basis differs".into());
        }
        let outcome = disposition["outcome"]
            .as_str()
            .ok_or("disposition outcome")?;
        let valid = match item["state"].as_str() {
            Some("resolved") => matches!(outcome, "accepted" | "rejected"),
            Some("cancelled") => outcome == "cancelled",
            Some("superseded") => outcome == "superseded",
            Some("archived") => {
                outcome == "archived"
                    && matches!(
                        old["state"].as_str(),
                        Some("resolved" | "cancelled" | "superseded")
                    )
            }
            Some("open") => matches!(outcome, "reopened" | "revise"),
            Some("in_progress" | "blocked") => outcome == "revise",
            _ => false,
        };
        if !valid {
            return Err("work state contradicts its disposition".into());
        }
        if terminal(old) && item["state"] == "open" && outcome != "reopened" {
            return Err("terminal work requires a reopened disposition".into());
        }
        if outcome == "superseded"
            && !item["relations"].as_array().is_some_and(|a| {
                a.iter()
                    .any(|r| r["type"] == "supersedes" && r["target"] == disposition["successor"])
            })
        {
            return Err("superseded work requires its exact successor relation".into());
        }
        let reason = self.store.get(&disposition["reason"])?;
        if reason.is_empty() {
            return Err("disposition reason is empty".into());
        }
        let evidence = disposition["evidence"]
            .as_array()
            .ok_or("disposition evidence")?;
        for reference in evidence {
            if !item["evidence"]
                .as_array()
                .is_some_and(|refs| refs.contains(reference))
            {
                return Err("disposition evidence must be linked by this item".into());
            }
        }
        if outcome == "accepted" {
            if item["kind"] == "work"
                && item["acceptance"]["schema"] == "openagents.labor-acceptance-policy.v1"
            {
                let delegation = self.store.json(&item["delegation"])?;
                let port = self
                    .market
                    .as_ref()
                    .ok_or("LAB acceptance requires the admitted LAB book")?;
                let mut accepted = false;
                for reference in evidence {
                    if !item["evidence"]
                        .as_array()
                        .is_some_and(|refs| refs.contains(reference))
                    {
                        return Err(
                            "acceptance evidence must be linked by the proposed item".into()
                        );
                    }
                    let (_, link) = self.original(state, reference)?;
                    if link["type"] == "commercial" {
                        port.acceptance(
                            &delegation["order"],
                            &item["frame"],
                            &item["acceptance"],
                            delegation["delegate"].as_str().ok_or("LAB worker")?,
                            &link["artifact"],
                        )?;
                        accepted = true;
                    }
                }
                if !accepted {
                    return Err("WORK acceptance requires the independently admitted exact LAB acceptance evidence".into());
                }
            } else if item["kind"] == "work" {
                let expected = self.acceptance_policy(&self.store.json(&item["acceptance"])?)?;
                let mut verified = BTreeSet::new();
                for reference in evidence {
                    if !item["evidence"]
                        .as_array()
                        .is_some_and(|refs| refs.contains(reference))
                    {
                        return Err(
                            "acceptance evidence must be linked by the proposed item".into()
                        );
                    }
                    let (_, link) = self.original(state, reference)?;
                    if link["type"] != "verification" {
                        continue;
                    }
                    self.verification(state, scope, w, item, &link)?;
                    let (_, report) = self.original(state, &link["artifact"])?;
                    for check in report["checks"].as_array().ok_or("verification checks")? {
                        if check["passed"] != true {
                            return Err("failed verification cannot be accepted".into());
                        }
                        verified.insert(check["id"].as_str().ok_or("check ID")?.to_string());
                    }
                }
                if verified != expected {
                    return Err(
                        "work acceptance requires the exact independently verified check set"
                            .into(),
                    );
                }
            } else if evidence.is_empty() {
                return Err("planning container acceptance requires its own evidence".into());
            }
        }
        Ok(())
    }
}
