//! Existing workbench references point to exact WS artifacts, never host paths.
use crate::{Result, artifact};
use serde_json::Value;
pub fn artifact_view(authority: &str, body: &Value) -> Result<workbench::ResourceRef> {
    let reference = artifact::json_ref(body)?;
    let digest = reference["digest"].as_str().ok_or("artifact digest")?;
    let value = workbench::ResourceRef::new(
        workbench::Kind::Artifact,
        workbench::Host::Paired {
            key: authority.into(),
        },
        digest.strip_prefix("sha256:").ok_or("artifact digest")?,
    );
    value.check().map_err(|e| e.to_string())?;
    Ok(value)
}
