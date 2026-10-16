//! Registered Rust bindings. Definition bytes cannot select arbitrary code.
use crate::{
    Result,
    book::{Coordinator, Policy},
};
use nostr::contracts::{self, Assurance, Bound};
use serde_json::{Value, json};
use std::collections::BTreeSet;
#[derive(Clone, Debug)]
pub(crate) struct Limits {
    pub calls: u64,
    pub wall_ms: u64,
    pub input_bytes: u64,
    pub output_bytes: u64,
    pub concurrency: u64,
    pub depth: u64,
}
impl Limits {
    pub(crate) fn parse(value: &Value, policy: &Policy) -> Result<Self> {
        nostr::coordination::bounds_valid(value).map_err(|e| e.to_string())?;
        let mut limits = Self {
            calls: 1,
            wall_ms: policy.max_claim_seconds * 1000,
            input_bytes: 1_048_576,
            output_bytes: 1_048_576,
            concurrency: policy.concurrency,
            depth: policy.max_depth,
        };
        for body in value.as_array().ok_or("bound assignments")? {
            let bound = contracts::parse_bound(body).map_err(|e| e.to_string())?;
            if bound.assurance != Assurance::Host
                || bound.minimum.is_some_and(|v| v > bound.ceiling)
            {
                return Err("cannot_enforce: unsupported bound assurance".into());
            }
            let (maximum, mechanism) = match bound.bound {
                Bound::WallMs => (policy.max_claim_seconds * 1000, "native-deadline"),
                Bound::Calls => (policy.calls, "native-ledger"),
                Bound::Attempts => (1, "native-ledger"),
                Bound::Concurrency => (policy.concurrency, "native-ledger"),
                Bound::Depth => (policy.max_depth, "native-ledger"),
                Bound::InputBytes | Bound::OutputBytes => (1_048_576, "native-bytes"),
                Bound::SpendMicrounits => (0, "no-spend"),
                _ => return Err("cannot_enforce: no native mechanism for this bound".into()),
            };
            if bound.ceiling > maximum
                || bound.mechanism != mechanism
                || bound.bound == Bound::SpendMicrounits && bound.currency.as_deref() != Some("USD")
            {
                return Err("cannot_enforce: native bound or mechanism differs".into());
            }
            match bound.bound {
                Bound::WallMs => limits.wall_ms = bound.ceiling,
                Bound::Calls => limits.calls = bound.ceiling,
                Bound::InputBytes => limits.input_bytes = bound.ceiling,
                Bound::OutputBytes => limits.output_bytes = bound.ceiling,
                Bound::Concurrency => limits.concurrency = bound.ceiling,
                Bound::Depth => limits.depth = bound.ceiling,
                _ => {}
            }
        }
        if limits.calls == 0 || limits.wall_ms == 0 || limits.concurrency == 0 {
            return Err(
                "native dispatch needs a positive call, time, and concurrency allowance".into(),
            );
        }
        Ok(limits)
    }
}
impl Coordinator {
    /// Local registration of a total data-copy operation. Input remains data.
    pub fn data_operation(&self) -> Result<Value> {
        let schema = self.store.put(
            &contracts::jcs(&json!(true)).map_err(|e| e.to_string())?,
            "application/schema+json",
            None,
        )?;
        let id = format!("{}:coord/data", self.authority());
        let body = json!({"v":1,"requires":[],"id":id,"profile":"native","summary":"Return the exact admitted JSON input as data.","input":schema,"output":schema,"effects":{"reads":["artifact"],"writes":[],"network":[],"process":false,"delegates":false,"spend":false},"minimum":{},"support":{"bounds":{},"cancellation":"before_dispatch","idempotency":"idempotent","evidence":[]},"binding_contract":{"operation":"openagents.coord-native-data.v1","interface":"openagents.coord-native-data.v1"}});
        let bytes = contracts::jcs(&body).map_err(|e| e.to_string())?;
        let artifact = self
            .store
            .put(&bytes, "application/json", Some("openagents.cap.v1"))?;
        Ok(json!({"id":id,"artifact":artifact}))
    }
    pub(crate) fn operation(&self, definition: &Value) -> Result<Value> {
        let def = contracts::parse_definition(definition).map_err(|e| e.to_string())?;
        let body = contracts::parse_strict(&self.store.get(&definition["artifact"])?)
            .map_err(|e| e.to_string())?;
        if body["v"] != 1
            || body["requires"] != json!([])
            || body["id"] != def.id
            || def.artifact.schema.as_deref() != Some("openagents.cap.v1")
        {
            return Err("unsupported native operation definition".into());
        }
        match body["binding_contract"]["operation"].as_str() {
            Some(
                "openagents.coord-native-data.v1"
                | "openagents.coord-native-container.v1"
                | "openagents.coord-native-git-change.v1",
            ) => {}
            _ => return Err("unsupported native operation binding".into()),
        }
        contracts::parse_schema_ref(&body["input"]).map_err(|e| e.to_string())?;
        contracts::parse_schema_ref(&body["output"]).map_err(|e| e.to_string())?;
        contracts::parse_effects(&body["effects"]).map_err(|e| e.to_string())?;
        if !def.id.starts_with(&format!("{}:coord/", self.authority())) {
            return Err(
                "unsupported native namespace; foreign components need their own admitted binding"
                    .into(),
            );
        }
        let expected = match body["binding_contract"]["operation"].as_str() {
            Some("openagents.coord-native-data.v1") => self.data_operation()?,
            Some("openagents.coord-native-container.v1") => {
                self.container_operation(body["effects"].clone())?
            }
            Some("openagents.coord-native-git-change.v1") => self.git_operation(
                body["binding_contract"]["resource"]
                    .as_str()
                    .ok_or("registered resource")?,
            )?,
            _ => return Err("unsupported native binding".into()),
        };
        if expected != *definition {
            return Err("native binding differs from its registered bytes".into());
        }
        Ok(body)
    }
    pub(crate) fn effects_subset(&self, required: &Value, ceiling: &Value) -> Result<()> {
        let requested = contracts::parse_effects(required).map_err(|e| e.to_string())?;
        let limit = contracts::parse_effects(ceiling).map_err(|e| e.to_string())?;
        for (requested, limit) in [
            (&requested.reads, &limit.reads),
            (&requested.writes, &limit.writes),
            (&requested.network, &limit.network),
        ] {
            let limit: BTreeSet<_> = limit.iter().collect();
            if requested.iter().any(|scope| !limit.contains(scope)) {
                return Err("not_admitted: operation widens its effect scope".into());
            }
        }
        if requested.process && !limit.process
            || requested.delegates && !limit.delegates
            || requested.spend && !limit.spend
        {
            return Err("not_admitted: operation widens its effect authority".into());
        }
        if requested.spend || !requested.network.is_empty() {
            return Err(
                "cannot_enforce: this native profile has no network or spending operation".into(),
            );
        }
        Ok(())
    }
}
impl Coordinator {
    /// A bounded envelope for child work, with no code dispatch of its own.
    pub fn container_operation(&self, effects: Value) -> Result<Value> {
        self.effects_subset(&effects, &effects)?;
        if effects["delegates"] != true {
            return Err("container operation must explicitly permit child work".into());
        }
        self.native_operation("container", effects, serde_json::json!(true), Value::Null)
    }
    /// Register patch preparation for one exact coordinator resource ID.
    /// Applying it remains a separate conditional source-owner operation.
    pub fn git_operation(&self, resource: &str) -> Result<Value> {
        if resource.is_empty() || resource.len() > 1024 {
            return Err("native Git resource ID".into());
        }
        let definitions: Value =
            serde_json::from_slice(nostr::coordination::SCHEMA_BYTES).map_err(|e| e.to_string())?;
        let schema = json!({"$defs":{"artifact":definitions["$defs"]["artifact"],"event":definitions["$defs"]["event"]},"type":"object","additionalProperties":false,"required":["v","requires","resource","snapshot","content"],"properties":{"v":{"const":"openagents.coordination-native-patch.v1"},"requires":{"type":"array","maxItems":0},"resource":{"const":resource},"snapshot":{"$ref":"#/$defs/artifact"},"content":{"$ref":"#/$defs/artifact"}}});
        let effects = json!({"reads":["artifact",resource],"writes":[resource],"network":[],"process":false,"delegates":false,"spend":false});
        self.native_operation("git-change", effects, schema, json!(resource))
    }
    fn native_operation(
        &self,
        name: &str,
        effects: Value,
        schema: Value,
        resource: Value,
    ) -> Result<Value> {
        let schema = self.store.put(
            &contracts::jcs(&schema).map_err(|e| e.to_string())?,
            "application/schema+json",
            None,
        )?;
        let suffix = if resource.is_null() {
            name.to_string()
        } else {
            contracts::digest_value(&json!({"operation":name,"resource":resource}))
                .map_err(|e| e.to_string())?
                .trim_start_matches("sha256:")
                .to_string()
        };
        let id = format!("{}:coord/{suffix}", self.authority());
        let body = json!({"v":1,"requires":[],"id":id,"profile":"native","summary":"Prepare admitted data for a separately authorized operation.","input":schema,"output":schema,"effects":effects,"minimum":{},"support":{"bounds":{},"cancellation":"before_dispatch","idempotency":"idempotent","evidence":[]},"binding_contract":{"operation":format!("openagents.coord-native-{name}.v1"),"interface":format!("openagents.coord-native-{name}.v1"),"resource":resource}});
        let artifact = self.store.put(
            &contracts::jcs(&body).map_err(|e| e.to_string())?,
            "application/json",
            Some("openagents.cap.v1"),
        )?;
        Ok(json!({"id":id,"artifact":artifact}))
    }
}
