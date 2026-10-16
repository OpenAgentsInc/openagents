//! Explicit operator authority for exact same-owner child relationships.
use super::*;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ChildGrant {
    pub parent: String,
    pub turn: String,
    pub child: String,
    pub expires_at: u64,
}

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Book {
    generation: u64,
    grants: BTreeMap<String, Value>,
}
impl Book {
    pub(super) fn invalidate(&mut self) -> Result<()> {
        self.generation = self
            .generation
            .checked_add(1)
            .filter(|v| i128::from(*v) <= contracts::SAFE_INTEGER)
            .ok_or("limit_exceeded: child authority generation")?;
        self.grants.clear();
        Ok(())
    }
}

impl SessionHost {
    fn child_participants(
        &self,
        state: &State,
        parent: &Session,
        turn: &str,
        child: &Session,
    ) -> Result<()> {
        if parent.id == child.id
            || parent.owner != child.owner
            || !state.policy.owners.contains(&parent.owner)
            || parent
                .active
                .as_ref()
                .is_none_or(|t| t.id != turn || t.interrupted || t.terminal.is_some())
            || parent.read_only
            || child.read_only
            || [parent, child].iter().any(|s| {
                s.opening
                    || s.closing_native
                    || s.replacing
                    || s.protocol_fault.is_some()
                    || !matches!(s.phase.as_str(), "ready" | "running" | "blocked")
            })
        {
            return Err(
                "not_admitted: child relationship needs current same-owner native sessions".into(),
            );
        }
        for session in [parent, child] {
            self.native_session(session, &state.adapters[&session.adapter].engine)?;
        }
        Ok(())
    }

    /// Host-key administration. These grants permit metadata links only;
    /// every child execution still requires its own admitted authority.
    pub fn set_child_grants(&self, grants: Vec<ChildGrant>, now: u64) -> Result<Vec<Value>> {
        let _guard = self.store.lock()?;
        let mut state = self.state()?;
        let references = self.replace_child_grants(&mut state, grants, now)?;
        self.save(&state)?;
        Ok(references)
    }

    /// Replace exact link grants in the live host, using its own signature.
    pub fn child_policy(&self, event: &Event, now: u64) -> Result<Value> {
        let _guard = self.store.lock()?;
        let mut state = self.state()?;
        let (actor, body, input) = self.authenticate(&mut state, event, now)?;
        if actor != self.pubkey() || body["v"] != "openagents.session-child-policy.v1" {
            return Err("not_admitted: child policy requires the host key".into());
        }
        sess::validate(&body).map_err(contract)?;
        let key = format!(
            "operator:child-policy:{}",
            body["command"].as_str().ok_or("command")?
        );
        if let Some(result) = Self::prior(&state, &key, digest_of(&input))? {
            return Ok(result);
        }
        if body["expected_generation"] != state.children_authority.generation {
            return Err("conflict: child policy generation changed".into());
        }
        let grants = serde_json::from_value(body["grants"].clone())
            .map_err(|_| "malformed: child grants")?;
        let references = self.replace_child_grants(&mut state, grants, now)?;
        let result = json!({"status":"accepted","generation":state.children_authority.generation,"authorizations":references});
        state.priors.insert(
            key,
            Prior {
                fingerprint: digest_of(&input).into(),
                result: result.clone(),
            },
        );
        self.save(&state)?;
        Ok(result)
    }

    fn replace_child_grants(
        &self,
        state: &mut State,
        grants: Vec<ChildGrant>,
        now: u64,
    ) -> Result<Vec<Value>> {
        if grants.len() > 64 {
            return Err("limit_exceeded: child grants".into());
        }
        let mut bodies = Vec::new();
        let mut seen = BTreeSet::new();
        for grant in grants {
            if grant.expires_at <= now || i128::from(grant.expires_at) > contracts::SAFE_INTEGER {
                return Err("stale: child authorization expiry".into());
            }
            let parent = state
                .sessions
                .get(&grant.parent)
                .ok_or("not_admitted: parent session")?;
            let child = state
                .sessions
                .get(&grant.child)
                .ok_or("not_admitted: child session")?;
            self.child_participants(&state, parent, &grant.turn, child)?;
            if !seen.insert((
                grant.parent.clone(),
                grant.turn.clone(),
                grant.child.clone(),
            )) {
                return Err("conflict: duplicate child authorization".into());
            }
            bodies.push(
                json!({"v":"openagents.session-child-authorization.v1","requires":[],
                "owner":parent.owner,"scope":parent.scope(self.pubkey()),"turn":grant.turn,
                "parent_admission":parent.admission,"child_scope":child.scope(self.pubkey()),
                "child_admission":child.admission,"starts_at":now,"expires_at":grant.expires_at}),
            );
        }
        state.children_authority.invalidate()?;
        let mut references = Vec::new();
        for mut body in bodies {
            body["generation"] = json!(state.children_authority.generation);
            let owner = body["owner"].as_str().ok_or("owner")?.to_string();
            let reference = self.sign(state, &body, &owner, now)?;
            state
                .children_authority
                .grants
                .insert(digest_of(&reference).to_string(), reference.clone());
            references.push(reference);
        }
        Ok(references)
    }

    pub(super) fn check_child_authority(
        &self,
        state: &State,
        parent: &Session,
        turn: &str,
        child: &Session,
        authorization: &Value,
        now: u64,
    ) -> Result<()> {
        let Some(retained) = state
            .children_authority
            .grants
            .get(digest_of(authorization))
        else {
            if state
                .signers
                .get(digest_of(authorization))
                .is_some_and(|signers| signers.contains(self.pubkey()))
                && self.store.json(authorization)?["v"]
                    == "openagents.session-child-authorization.v1"
            {
                return Err("stale: the host revoked this child authorization".into());
            }
            return self.bindings.check_child(
                &parent.owner,
                &parent.admission,
                &child.admission,
                authorization,
                now,
            );
        };
        self.child_participants(state, parent, turn, child)?;
        let body = self.store.json(retained)?;
        sess::validate(&body).map_err(contract)?;
        if retained != authorization
            || body["generation"] != state.children_authority.generation
            || body["owner"] != parent.owner
            || body["scope"] != parent.scope(self.pubkey())
            || body["turn"] != turn
            || body["parent_admission"] != parent.admission
            || body["child_scope"] != child.scope(self.pubkey())
            || body["child_admission"] != child.admission
            || body["starts_at"].as_u64().is_none_or(|v| v > now)
            || body["expires_at"].as_u64().is_none_or(|v| v <= now)
        {
            return Err("stale: child authorization differs or expired".into());
        }
        Ok(())
    }
}
