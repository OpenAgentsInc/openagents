//! Real UTF-8 capture and conditional input over a source-owned Git document.
//! Registration is local operator authority; plans never select filesystem paths.
use crate::{
    Result,
    media::{Applied, CaptureContext, Captured, Device, InputGuard, InputOperation, Source},
};
use nostr::contracts::{self, digest_bytes};
use serde_json::json;
use std::path::Path;
use workspace_host::git::GitDocument;

#[derive(Clone)]
pub struct Document {
    source: GitDocument,
    owner: String,
    id: String,
    binding: Vec<u8>,
}
impl Document {
    /// Register one owner, branch, and document. This does not change Git.
    pub fn register(root: &Path, branch: &str, path: &str, owner: String) -> Result<Self> {
        owner
            .parse::<secp256k1::XOnlyPublicKey>()
            .map_err(|_| "malformed: document owner")?;
        let source = GitDocument::admit(root, branch, path)?;
        let id = digest_bytes(
            &contracts::jcs(&serde_json::to_value(&source).map_err(|e| e.to_string())?)
                .map_err(|e| e.to_string())?,
        );
        let binding = contracts::jcs(
            &json!({"profile":"openagents.live-git-document.v1","source":id,"owner":owner}),
        )
        .map_err(|e| e.to_string())?;
        Ok(Self {
            source,
            owner,
            id,
            binding,
        })
    }
    pub fn source_id(&self) -> &str {
        &self.id
    }
    pub fn binding(&self) -> &[u8] {
        &self.binding
    }
}
impl Source for Document {
    fn capture(&mut self, _: &str, _: &str, _: &str) -> Result<Captured> {
        Err("not_admitted: checked document capture required".into())
    }
    fn capture_checked(
        &mut self,
        scope: &str,
        id: &str,
        media: &str,
        context: &CaptureContext<'_>,
    ) -> Result<Captured> {
        if context.owner != self.owner || scope != "resource" || id != self.id || media != "text" {
            return Err("not_admitted: document source or owner".into());
        }
        if std::time::Instant::now() >= context.deadline {
            return Err("stale: capture deadline".into());
        }
        let capture = self.source.capture()?;
        if capture.bytes.len() > context.max_bytes.min(65_536) {
            return Err("limit_exceeded: document bytes".into());
        }
        if std::time::Instant::now() >= context.deadline {
            return Err("stale: capture deadline".into());
        }
        Ok(Captured {
            bytes: capture.bytes,
            anchor: Some((0, 0)),
            native: json!({"position":capture.position}),
            geometry: None,
            interval: None,
        })
    }
}
impl Device for Document {
    fn state(&self) -> (u64, u64) {
        (0, 0)
    }
    fn input_operation(&self) -> Option<InputOperation> {
        Some(InputOperation {
            implementation:
                b"openagents.live-git-document.v1 source-native conditional replacement".to_vec(),
            binding: self.binding.clone(),
            input_schema: json!({"type":"object","required":["replace"],"additionalProperties":false,
                "properties":{"replace":{"type":"string","maxLength":65536}}}),
            observation_schema: json!({"type":"object","required":["source","meta","content"],
                "properties":{"source":{"const":{"scope":"resource","id":self.id}},
                    "content":{"type":"object"},"meta":{"type":"object","required":["native"],
                        "properties":{"native":{"type":"object","required":["position"],"additionalProperties":false,
                            "properties":{"position":{"type":"string","minLength":1,"maxLength":8192}}}}}}}),
            preconditions_schema: json!({"type":"array","maxItems":0}),
            output_schema: json!({"type":"object","required":["position"],"additionalProperties":false,
                "properties":{"position":{"type":"string","minLength":1,"maxLength":8192}}}),
            effects: json!({"reads":["git-document"],"writes":["git-document"],"network":[],"process":false,"delegates":false,"spend":false}),
        })
    }
    fn apply(&mut self, _: &str, _: &[u8]) -> Applied {
        Applied::Refused("not_admitted")
    }
    fn apply_checked(&mut self, operation: &str, input: &[u8], guard: &InputGuard) -> Applied {
        if !operation.ends_with(":live/device-input")
            || guard.owner != self.owner
            || guard.binding != self.binding
            || guard.generation != 0
            || guard.focus != 0
            || !guard.preconditions.is_empty()
            || guard.observation["source"] != json!({"scope":"resource","id":self.id})
            || guard.observation["plan"] != guard.plan
            || std::time::Instant::now() >= guard.deadline
        {
            return Applied::Refused("not_admitted");
        }
        let Ok(input) = contracts::parse_strict_bounded(input, 1_048_576) else {
            return Applied::Refused("malformed");
        };
        let Some(replacement) = input["replace"].as_str().filter(|s| s.len() <= 65_536) else {
            return Applied::Refused("malformed");
        };
        if input.as_object().is_none_or(|m| m.len() != 1) {
            return Applied::Refused("malformed");
        }
        let Some(expected) = guard.observation["meta"]["native"]["position"].as_str() else {
            return Applied::Refused("cannot_enforce");
        };
        let fingerprint = digest_bytes(&contracts::jcs(&json!({"session":guard.session,"plan":guard.plan,"expected":expected,"input":input})).unwrap_or_default());
        let candidate = match self
            .source
            .prepare(expected, replacement.as_bytes(), &fingerprint)
        {
            Ok(candidate) => candidate,
            Err(_) => return Applied::Refused("stale"),
        };
        let mut ready = false;
        let result = self.source.dispatch_checked(expected, &candidate, || {
            if std::time::Instant::now() >= guard.deadline {
                return Err("stale: document input deadline".into());
            }
            ready = true;
            Ok(())
        });
        if result.is_err() {
            return if ready {
                Applied::Unknown
            } else {
                Applied::Refused("stale")
            };
        }
        match self.source.position_for(&candidate).and_then(|position| {
            serde_json::to_vec(&json!({"position":position})).map_err(|e| e.to_string())
        }) {
            Ok(bytes) => Applied::Applied(bytes),
            Err(_) => Applied::Unknown,
        }
    }
}
