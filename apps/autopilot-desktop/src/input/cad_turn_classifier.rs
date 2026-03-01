#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct CadTurnClassification {
    pub is_cad_turn: bool,
    pub reason: String,
}

impl CadTurnClassification {
    fn cad(reason: impl Into<String>) -> Self {
        Self {
            is_cad_turn: true,
            reason: reason.into(),
        }
    }

    fn non_cad(reason: impl Into<String>) -> Self {
        Self {
            is_cad_turn: false,
            reason: reason.into(),
        }
    }
}

pub(super) fn classify_chat_prompt(prompt: &str) -> CadTurnClassification {
    let trimmed = prompt.trim();
    if trimmed.is_empty() {
        return CadTurnClassification::non_cad("empty-prompt");
    }

    let normalized = trimmed.to_ascii_lowercase();
    for marker in EXPLICIT_CAD_MARKERS {
        if normalized.contains(marker) {
            return CadTurnClassification::cad(format!("explicit-marker:{marker}"));
        }
    }

    let verb_match = CAD_VERBS.iter().find(|verb| normalized.contains(**verb));
    let noun_match = CAD_NOUNS.iter().find(|noun| normalized.contains(**noun));
    if let (Some(verb), Some(noun)) = (verb_match, noun_match) {
        return CadTurnClassification::cad(format!("keyword-pair:{verb}+{noun}"));
    }

    CadTurnClassification::non_cad("no-cad-signals")
}

const EXPLICIT_CAD_MARKERS: &[&str] = &[
    "openagents.cad.intent",
    "openagents.cad.action",
    "\"intent_json\"",
    "\"document_id\"",
    "\"variant\"",
    "cad pane",
    "cad window",
    "step export",
    "export step",
];

const CAD_VERBS: &[&str] = &[
    "design", "model", "build", "create", "draft", "draw", "sketch", "extrude", "fillet",
    "chamfer", "revolve",
];

const CAD_NOUNS: &[&str] = &[
    "cad",
    "rack",
    "bracket",
    "enclosure",
    "mount",
    "hole",
    "vent",
    "wall mount",
    "mac studio",
    "solid",
    "parametric",
    "feature graph",
    "step",
];

#[cfg(test)]
mod tests {
    use super::classify_chat_prompt;

    #[test]
    fn classify_marks_explicit_cad_marker() {
        let result = classify_chat_prompt(
            r#"Use openagents.cad.intent with {"intent_json":{"intent":"set_parameter"}}"#,
        );
        assert!(result.is_cad_turn);
        assert!(result.reason.starts_with("explicit-marker:"));
    }

    #[test]
    fn classify_marks_keyword_pair_cad_prompt() {
        let result =
            classify_chat_prompt("Design a wall mount rack with larger vent holes for airflow");
        assert!(result.is_cad_turn);
        assert!(result.reason.starts_with("keyword-pair:"));
    }

    #[test]
    fn classify_rejects_non_cad_prompt() {
        let result = classify_chat_prompt("Summarize the last five commits in this repository");
        assert!(!result.is_cad_turn);
        assert_eq!(result.reason, "no-cad-signals");
    }
}
