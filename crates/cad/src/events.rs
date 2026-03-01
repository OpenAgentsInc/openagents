use serde::{Deserialize, Serialize};

/// Typed CAD event kinds for pane and activity feed integration.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum CadEventKind {
    DocumentCreated,
    VariantGenerated,
    SelectionChanged,
    WarningRaised,
    ParameterUpdated,
    RebuildCompleted,
    AnalysisUpdated,
    ExportCompleted,
    ExportFailed,
}

impl CadEventKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::DocumentCreated => "document.created",
            Self::VariantGenerated => "variant.generated",
            Self::SelectionChanged => "selection.changed",
            Self::WarningRaised => "warning.raised",
            Self::ParameterUpdated => "parameter.updated",
            Self::RebuildCompleted => "rebuild.completed",
            Self::AnalysisUpdated => "analysis.updated",
            Self::ExportCompleted => "export.completed",
            Self::ExportFailed => "export.failed",
        }
    }
}

/// Deterministic CAD event payload.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct CadEvent {
    pub event_id: String,
    pub kind: CadEventKind,
    pub session_id: String,
    pub document_id: String,
    pub document_revision: u64,
    pub variant_id: Option<String>,
    pub summary: String,
    pub detail: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CadEventMessage {
    pub summary: String,
    pub detail: String,
    pub key: Option<String>,
}

impl CadEventMessage {
    pub fn new(summary: impl Into<String>, detail: impl Into<String>) -> Self {
        Self {
            summary: summary.into(),
            detail: detail.into(),
            key: None,
        }
    }

    pub fn with_key(mut self, key: impl Into<String>) -> Self {
        self.key = Some(key.into());
        self
    }

    pub fn with_optional_key(mut self, key: Option<String>) -> Self {
        self.key = key;
        self
    }
}

impl CadEvent {
    pub fn new(
        kind: CadEventKind,
        session_id: impl Into<String>,
        document_id: impl Into<String>,
        document_revision: u64,
        variant_id: Option<String>,
        summary: impl Into<String>,
        detail: impl Into<String>,
    ) -> Self {
        Self::new_with_key(
            kind,
            session_id,
            document_id,
            document_revision,
            variant_id,
            CadEventMessage::new(summary, detail),
        )
    }

    pub fn new_with_key(
        kind: CadEventKind,
        session_id: impl Into<String>,
        document_id: impl Into<String>,
        document_revision: u64,
        variant_id: Option<String>,
        message: CadEventMessage,
    ) -> Self {
        let session_id = session_id.into();
        let document_id = document_id.into();
        let event_id = build_cad_event_id(
            &kind,
            &session_id,
            &document_id,
            document_revision,
            variant_id.as_deref(),
            message.key.as_deref().or(Some(message.summary.as_str())),
        );
        Self {
            event_id,
            kind,
            session_id,
            document_id,
            document_revision,
            variant_id,
            summary: message.summary,
            detail: message.detail,
        }
    }
}

/// Build deterministic, dedupe-safe CAD event ID.
pub fn build_cad_event_id(
    kind: &CadEventKind,
    session_id: &str,
    document_id: &str,
    document_revision: u64,
    variant_id: Option<&str>,
    key: Option<&str>,
) -> String {
    let variant = variant_id.unwrap_or("none");
    let key = key.unwrap_or("none");
    let digest = fnv1a64(
        format!(
            "kind={}|session={}|doc={}|rev={}|variant={}|key={}",
            kind.as_str(),
            session_id,
            document_id,
            document_revision,
            variant,
            key
        )
        .as_bytes(),
    );
    format!(
        "cad:{}:{}:{}:{}",
        kind.as_str(),
        document_revision,
        variant,
        digest
    )
}

fn fnv1a64(bytes: &[u8]) -> u64 {
    const FNV_OFFSET_BASIS: u64 = 0xcbf29ce484222325;
    const FNV_PRIME: u64 = 0x100000001b3;

    let mut hash = FNV_OFFSET_BASIS;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(FNV_PRIME);
    }
    hash
}

#[cfg(test)]
mod tests {
    use super::{CadEvent, CadEventKind, build_cad_event_id};

    #[test]
    fn cad_event_id_is_deterministic() {
        let first = build_cad_event_id(
            &CadEventKind::RebuildCompleted,
            "cad.session.a",
            "cad.doc.demo-rack",
            12,
            Some("variant.stiffness"),
            Some("mesh:abcd"),
        );
        let second = build_cad_event_id(
            &CadEventKind::RebuildCompleted,
            "cad.session.a",
            "cad.doc.demo-rack",
            12,
            Some("variant.stiffness"),
            Some("mesh:abcd"),
        );
        assert_eq!(first, second);
    }

    #[test]
    fn cad_event_payload_serde_is_stable() {
        let event = CadEvent::new(
            CadEventKind::AnalysisUpdated,
            "cad.session.a",
            "cad.doc.demo-rack",
            22,
            Some("variant.lightweight".to_string()),
            "analysis updated",
            "mass_kg=2.3",
        );
        let encoded = serde_json::to_string(&event).expect("event should serialize");
        assert!(encoded.contains("analysis.updated"));
        assert!(encoded.contains("variant.lightweight"));
    }

    #[test]
    fn cad_event_ids_change_when_key_changes() {
        let left = build_cad_event_id(
            &CadEventKind::SelectionChanged,
            "cad.session.a",
            "cad.doc.demo-rack",
            1,
            Some("variant.baseline"),
            Some("sel:face.a"),
        );
        let right = build_cad_event_id(
            &CadEventKind::SelectionChanged,
            "cad.session.a",
            "cad.doc.demo-rack",
            1,
            Some("variant.baseline"),
            Some("sel:face.b"),
        );
        assert_ne!(left, right);
    }
}
