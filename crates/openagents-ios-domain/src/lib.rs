#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CodexMessageRole {
    User = 0,
    Assistant = 1,
    Reasoning = 2,
    Tool = 3,
    System = 4,
    Error = 5,
}

impl CodexMessageRole {
    pub fn from_u8(value: u8) -> Self {
        match value {
            0 => Self::User,
            1 => Self::Assistant,
            2 => Self::Reasoning,
            3 => Self::Tool,
            5 => Self::Error,
            _ => Self::System,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CodexMessage {
    pub role: CodexMessageRole,
    pub text: String,
    pub is_streaming: bool,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum MissionEventSeverity {
    #[default]
    Info = 0,
    Warning = 1,
    Error = 2,
}

impl MissionEventSeverity {
    pub fn from_u8(value: u8) -> Self {
        match value {
            1 => Self::Warning,
            2 => Self::Error,
            _ => Self::Info,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Info => "info",
            Self::Warning => "warning",
            Self::Error => "error",
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct MissionEventRecord {
    pub topic: String,
    pub event_type: String,
    pub method: String,
    pub summary: String,
    pub severity: MissionEventSeverity,
    pub resync_marker: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MissionEventFilter {
    All,
    Control,
    Turn,
    Tool,
    Errors,
    Handshake,
    System,
}

impl MissionEventFilter {
    pub fn all() -> [Self; 7] {
        [
            Self::All,
            Self::Control,
            Self::Turn,
            Self::Tool,
            Self::Errors,
            Self::Handshake,
            Self::System,
        ]
    }

    pub fn from_u8(value: u8) -> Self {
        match value {
            1 => Self::Control,
            2 => Self::Turn,
            3 => Self::Tool,
            4 => Self::Errors,
            5 => Self::Handshake,
            6 => Self::System,
            _ => Self::All,
        }
    }

    pub fn to_u8(self) -> u8 {
        match self {
            Self::All => 0,
            Self::Control => 1,
            Self::Turn => 2,
            Self::Tool => 3,
            Self::Errors => 4,
            Self::Handshake => 5,
            Self::System => 6,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::All => "All",
            Self::Control => "Control",
            Self::Turn => "Turn",
            Self::Tool => "Tool",
            Self::Errors => "Errors",
            Self::Handshake => "Handshake",
            Self::System => "System",
        }
    }

    pub fn matches(self, event: &MissionEventRecord) -> bool {
        match self {
            Self::All => true,
            Self::Control => {
                event.topic.contains("control")
                    || event.event_type.contains("control")
                    || event.method.contains("control")
            }
            Self::Turn => event.event_type.contains("turn") || event.method.contains("turn"),
            Self::Tool => event.event_type.contains("tool") || event.method.contains("tool"),
            Self::Errors => matches!(event.severity, MissionEventSeverity::Error),
            Self::Handshake => {
                event.method.contains("handshake")
                    || event.summary.contains("handshake")
                    || event.event_type.contains("handshake")
            }
            Self::System => {
                event.topic.contains("system")
                    || event.resync_marker
                    || event.severity == MissionEventSeverity::Warning
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{CodexMessageRole, MissionEventFilter, MissionEventRecord, MissionEventSeverity};

    #[test]
    fn codex_role_from_u8_maps_expected_values() {
        assert_eq!(CodexMessageRole::from_u8(0), CodexMessageRole::User);
        assert_eq!(CodexMessageRole::from_u8(1), CodexMessageRole::Assistant);
        assert_eq!(CodexMessageRole::from_u8(5), CodexMessageRole::Error);
        assert_eq!(CodexMessageRole::from_u8(99), CodexMessageRole::System);
    }

    #[test]
    fn mission_filter_roundtrip() {
        for filter in MissionEventFilter::all() {
            assert_eq!(MissionEventFilter::from_u8(filter.to_u8()), filter);
            assert!(!filter.label().is_empty());
        }
    }

    #[test]
    fn mission_filter_matches_errors() {
        let event = MissionEventRecord {
            severity: MissionEventSeverity::Error,
            ..MissionEventRecord::default()
        };
        assert!(MissionEventFilter::Errors.matches(&event));
    }

    #[test]
    fn mission_filter_matches_handshake_markers() {
        let event = MissionEventRecord {
            summary: "handshake complete".to_string(),
            ..MissionEventRecord::default()
        };
        assert!(MissionEventFilter::Handshake.matches(&event));
    }

    #[test]
    fn severity_string_labels_are_stable() {
        assert_eq!(MissionEventSeverity::Info.as_str(), "info");
        assert_eq!(MissionEventSeverity::Warning.as_str(), "warning");
        assert_eq!(MissionEventSeverity::Error.as_str(), "error");
    }
}
