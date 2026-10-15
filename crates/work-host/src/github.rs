//! Stable external identities are observations, never authority to close work.
use crate::{
    Result,
    authority::{Authority, Scope, State},
};
use nostr::work::{self, WorkRef};
use serde_json::Value;
impl Authority {
    fn github_adapter(&self) -> Result<Value> {
        let body = serde_json::json!({"v":"openagents.work-github-tracker-adapter.v1","requires":[],"source":"coder::tracker::Snapshot","identity":"repository-and-node-id","consistency":"observational","body":"bounded_excerpt_with_full_body_digest","external_writes":false});
        Ok(
            serde_json::json!({"id":format!("{}:work/github-tracker",self.signer.pubkey()),"artifact":self.store.put_json(&body)?}),
        )
    }
    pub(crate) fn import_identity(
        &self,
        state: &State,
        scope: &mut Scope,
        w: &WorkRef,
        command: &Value,
        prior: Option<&Value>,
        _item: &Value,
        actor: &str,
        now: u64,
    ) -> Result<()> {
        if command["source"].is_null() {
            return Ok(());
        }
        self.check(scope, actor, Some(&w.item), "revise", now)?;
        let (issuer, source) = self.original(state, &command["source"])?;
        self.check(scope, &issuer, Some(&w.item), "read", now)?;
        let observation = self.store.json(&source["assurance"])?;
        if issuer != self.signer.pubkey()
            || source["adapter"] != self.github_adapter()?
            || observation["consistency"] != "observational"
        {
            return Err("unsupported source adapter; native imports require the admitted GitHub tracker capture".into());
        }

        work::verify_source(&source, &observation).map_err(|e| e.to_string())?;
        self.store.get(&source["content"])?;
        let key = serde_json::to_string(&(&source["system"], &source["subject"]))
            .map_err(|e| e.to_string())?;
        if let Some(existing) = scope.imports.get(&key) {
            if existing != &w.item {
                return Err("conflict: external object already has a native work identity".into());
            }
            // Every later observation is an explicit conditional replace. Native
            // revisions are never overwritten by a background import.
            if command["action"] != "replace" || prior != Some(&command["expected"]) {
                return Err("conflict: repeat import requires the exact native revision".into());
            }
            if command["reason"].is_null() {
                return Err(
                    "conflict: repeat import requires an explicit resolution reason".into(),
                );
            }
            if self.store.get(&command["reason"])?.is_empty() {
                return Err("import resolution reason is empty".into());
            }
        } else {
            if scope.imports.len() >= 256 {
                return Err("import identity limit exceeded".into());
            }
            scope.imports.insert(key, w.item.clone());
        }
        Ok(())
    }
}

impl Authority {
    /// Local import planning over a capture already validated by the tracker
    /// adapter. Returning a bundle admits no native change or external write.
    pub fn plan_github(
        &self,
        scope_ref: &Value,
        capture: Value,
        item: Value,
        expected: Option<Value>,
        resolution: Option<Vec<u8>>,
        observed_at: u64,
        now: u64,
    ) -> Result<Value> {
        use nostr::contracts;
        use serde_json::json;
        use workspace_host::artifact;
        let repo = capture["repo"]
            .as_str()
            .filter(|s| !s.is_empty() && s.len() <= 480)
            .ok_or("GitHub repository identity")?;
        let node = capture["node"]
            .as_str()
            .filter(|s| !s.is_empty() && s.len() <= 512)
            .ok_or("GitHub node identity is required; an issue number is insufficient")?
            .to_string();
        let version = capture["updated"]
            .as_str()
            .filter(|s| !s.is_empty() && s.len() <= 1024)
            .ok_or("GitHub observation version")?
            .to_string();
        let system = format!("github:{repo}");
        let identity = serde_json::to_string(&(&system, &node)).map_err(|e| e.to_string())?;
        let _lock = self.store.lock()?;
        let mut state = self.state()?;
        let (scope_id, scope) = state
            .scopes
            .iter()
            .find(|(_, s)| s.reference == *scope_ref)
            .ok_or("work scope unavailable")?;
        self.check(scope, self.signer.pubkey(), None, "create", now)?;
        let existing = scope.imports.get(&identity).cloned();
        let item_id = existing.clone().unwrap_or_else(artifact::token);
        let scope_id = scope_id.clone();
        if existing.is_some() && (expected.is_none() || resolution.is_none()) {
            return Err("conflict: an imported issue already exists; provide its exact revision and an explicit resolution reason".into());
        }
        if existing.is_none() && expected.is_some() {
            return Err("conflict: a first import cannot replace an unrelated item".into());
        }
        let w = json!({"authority":self.signer.pubkey(),"scope":scope_id,"item":item_id});
        let adapter = self.github_adapter()?;
        let content=self.store.put_json(&json!({"v":"openagents.work-github-capture.v1","requires":[],"issue":capture,"body_coverage":"excerpt_only"}))?;
        let observation = json!({"v":"openagents.observation.v1","requires":[],"resource":{"scope":system,"id":node},"adapter":adapter,"captured_at":observed_at,"provider_revision":version,"content":content,"consistency":"observational","valid_until":null});
        let observation = self.store.put_json(&observation)?;
        let source = json!({"v":"openagents.work-source.v1","requires":[],"issuer":self.signer.pubkey(),"system":system,"subject":node,"adapter":adapter,"observed_at":observed_at,"version":version,"content":content,"assurance":observation});
        let source = self.declare(&mut state, &source, self.signer.pubkey(), now)?;
        let reason = resolution
            .map(|bytes| self.store.put(&bytes, "text/plain", None))
            .transpose()?;
        let command = json!({"v":"openagents.work-command.v1","requires":[],"issuer":self.signer.pubkey(),"command_id":artifact::token(),"scope":scope_ref,"work":w,"action":if existing.is_some(){"replace"}else{"create"},"expected":expected,"snapshot":self.store.put_json(&item)?,"source":source,"reason":reason,"expires_at":now+300});
        work::validate(&command).map_err(|e| e.to_string())?;
        let event =
            workspace_host::host::request(&self.secret, self.signer.pubkey(), command, now)?;
        self.save(&state)?;
        let _ = contracts::parse_artifact(&source).map_err(|e| e.to_string())?;
        Ok(
            json!({"event":event,"attachments":{},"work":w,"source":source,"native_change":"proposed"}),
        )
    }
}
