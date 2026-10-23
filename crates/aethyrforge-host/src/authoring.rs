//! Creator-owned editing state, shared by controls, Kiln, and semantic commands.
use crate::{Error, MAX_SAFE_INTEGER, protocol::id};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub const AUTHORING_SCHEMA: &str = "openagents.verse.authoring.v1";
pub const EFFECT_ASSET: &str = "world-effect";
pub const DEFAULT_EFFECT: &str = "fn oa_material(base:vec3<f32>, position:vec3<f32>, normal:vec3<f32>, uv:vec2<f32>, time:f32)->vec3<f32> {\n  return base;\n}";
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct MaterialBinding {
    pub id: String,
    pub objects: Vec<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct AuthoringView {
    pub schema: String,
    pub revision: u64,
    pub effect_scope: String,
    pub can_undo: bool,
    pub can_redo: bool,
    pub draft: String,
    pub active_source: String,
    pub diagnostics: Vec<String>,
    pub materials: Vec<MaterialBinding>,
    pub environment: BTreeMap<String, f64>,
}
impl AuthoringView {
    pub fn validate(&self) -> Result<(), Error> {
        if self.schema != AUTHORING_SCHEMA
            || self.effect_scope != "world"
            || self.revision > MAX_SAFE_INTEGER
            || self.draft.len() > 4096
            || self.active_source.len() > 4096
            || self.draft.contains('\0')
            || self.active_source.contains('\0')
            || self.diagnostics.len() > 8
            || self.diagnostics.iter().any(|s| s.len() > 2048)
            || self.materials.len() > 256
            || self
                .materials
                .iter()
                .any(|m| !id(&m.id) || m.objects.len() > 128 || m.objects.iter().any(|s| !id(s)))
            || self.environment.len() > 24
            || self
                .environment
                .iter()
                .any(|(k, v)| !id(k) || !v.is_finite())
        {
            return Err(Error::Invalid("authoring view"));
        }
        Ok(())
    }
}
