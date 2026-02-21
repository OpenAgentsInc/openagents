//! OpenAgents proto wire contracts.
//!
//! This crate owns generated Rust wire types for all `proto/openagents/*/v1/*` packages.
//! It intentionally separates wire types from richer domain models.

/// Proto-generated wire contracts.
pub mod wire {
    include!(concat!(env!("OUT_DIR"), "/openagents.rs"));
}

/// Domain-layer wrappers and conversions over generated wire contracts.
pub mod domain {
    use thiserror::Error;

    use crate::wire::openagents::sync::v1::{KhalaFrame as WireKhalaFrame, KhalaFrameKind};

    /// Domain-level representation for a Khala frame.
    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct KhalaFrame {
        pub topic: String,
        pub seq: u64,
        pub kind: KhalaFrameDomainKind,
        pub payload_bytes: Vec<u8>,
        pub schema_version: u32,
    }

    /// Stable domain enum mapped from proto `KhalaFrameKind`.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum KhalaFrameDomainKind {
        Subscribed,
        UpdateBatch,
        Heartbeat,
        Error,
        Unknown(i32),
    }

    /// Conversion failures from wire-level payloads.
    #[derive(Debug, Clone, Error, PartialEq, Eq)]
    pub enum ConversionError {
        #[error("khala frame topic is required")]
        MissingTopic,
    }

    impl TryFrom<WireKhalaFrame> for KhalaFrame {
        type Error = ConversionError;

        fn try_from(value: WireKhalaFrame) -> Result<Self, Self::Error> {
            if value.topic.trim().is_empty() {
                return Err(ConversionError::MissingTopic);
            }

            Ok(Self {
                topic: value.topic,
                seq: value.seq,
                kind: kind_from_i32(value.kind),
                payload_bytes: value.payload_bytes,
                schema_version: value.schema_version,
            })
        }
    }

    impl From<KhalaFrame> for WireKhalaFrame {
        fn from(value: KhalaFrame) -> Self {
            Self {
                topic: value.topic,
                seq: value.seq,
                kind: kind_to_i32(value.kind),
                payload_bytes: value.payload_bytes,
                schema_version: value.schema_version,
            }
        }
    }

    fn kind_from_i32(raw: i32) -> KhalaFrameDomainKind {
        if raw == KhalaFrameKind::Subscribed as i32 {
            KhalaFrameDomainKind::Subscribed
        } else if raw == KhalaFrameKind::UpdateBatch as i32 {
            KhalaFrameDomainKind::UpdateBatch
        } else if raw == KhalaFrameKind::Heartbeat as i32 {
            KhalaFrameDomainKind::Heartbeat
        } else if raw == KhalaFrameKind::Error as i32 {
            KhalaFrameDomainKind::Error
        } else {
            KhalaFrameDomainKind::Unknown(raw)
        }
    }

    fn kind_to_i32(kind: KhalaFrameDomainKind) -> i32 {
        match kind {
            KhalaFrameDomainKind::Subscribed => KhalaFrameKind::Subscribed as i32,
            KhalaFrameDomainKind::UpdateBatch => KhalaFrameKind::UpdateBatch as i32,
            KhalaFrameDomainKind::Heartbeat => KhalaFrameKind::Heartbeat as i32,
            KhalaFrameDomainKind::Error => KhalaFrameKind::Error as i32,
            KhalaFrameDomainKind::Unknown(raw) => raw,
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::domain::{ConversionError, KhalaFrame, KhalaFrameDomainKind};
    use crate::wire::openagents::codex::v1::CodexWorkerSummary;
    use crate::wire::openagents::control::v1::AuthSession;
    use crate::wire::openagents::lightning::v1::ControlPlaneSnapshotResponse;
    use crate::wire::openagents::runtime::v1::RuntimeRun;
    use crate::wire::openagents::sync::v1::KhalaFrame as WireKhalaFrame;

    #[test]
    fn generated_wire_contracts_are_importable() {
        let _ = AuthSession::default();
        let _ = RuntimeRun::default();
        let _ = CodexWorkerSummary::default();
        let _ = ControlPlaneSnapshotResponse::default();
    }

    #[test]
    fn khala_frame_wire_domain_round_trip() {
        let wire = WireKhalaFrame {
            topic: "runtime.codex_worker_events".to_string(),
            seq: 42,
            kind: 2,
            payload_bytes: vec![1, 2, 3],
            schema_version: 1,
        };

        let domain = KhalaFrame::try_from(wire.clone()).expect("wire to domain should succeed");
        assert_eq!(domain.topic, "runtime.codex_worker_events");
        assert_eq!(domain.seq, 42);
        assert_eq!(domain.kind, KhalaFrameDomainKind::UpdateBatch);

        let round_trip = WireKhalaFrame::from(domain);
        assert_eq!(round_trip, wire);
    }

    #[test]
    fn khala_frame_domain_rejects_missing_topic() {
        let wire = WireKhalaFrame {
            topic: "  ".to_string(),
            seq: 1,
            kind: 1,
            payload_bytes: vec![],
            schema_version: 1,
        };

        let error = KhalaFrame::try_from(wire).expect_err("missing topic should fail");
        assert_eq!(error, ConversionError::MissingTopic);
    }
}
