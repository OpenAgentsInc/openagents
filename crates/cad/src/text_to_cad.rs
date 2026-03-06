use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::CadResult;
use crate::hash::stable_hex_digest;
use crate::mcp_tools::{
    CadMcpDocument, CadMcpMaterial, CadMcpNode, CadMcpNodeOp, CadMcpRoot, CadMcpVec3,
    cad_document_from_compact, cad_document_to_compact,
};

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TextToCadModelProfile {
    #[default]
    Cad0,
    Cad0Mini,
}

impl TextToCadModelProfile {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Cad0 => "cad0",
            Self::Cad0Mini => "cad0-mini",
        }
    }

    fn cylinder_segments(self) -> u32 {
        match self {
            Self::Cad0 => 32,
            Self::Cad0Mini => 16,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct TextToCadRequest {
    pub prompt: String,
    #[serde(default)]
    pub model: TextToCadModelProfile,
}

impl TextToCadRequest {
    pub fn new(prompt: impl Into<String>) -> Self {
        Self {
            prompt: prompt.into(),
            model: TextToCadModelProfile::Cad0,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct TextToCadClarification {
    pub code: String,
    pub message: String,
    pub questions: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TextToCadGeneration {
    pub model: TextToCadModelProfile,
    pub prompt: String,
    pub compact_ir: String,
    pub ir: CadMcpDocument,
    pub operation_count: usize,
    pub deterministic_signature: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum TextToCadOutcome {
    Generated(TextToCadGeneration),
    Clarification(TextToCadClarification),
}

pub fn text_to_cad_from_prompt(prompt: &str) -> CadResult<TextToCadOutcome> {
    text_to_cad(TextToCadRequest::new(prompt))
}

pub fn text_to_cad(request: TextToCadRequest) -> CadResult<TextToCadOutcome> {
    let prompt = request.prompt.trim();
    if prompt.is_empty() {
        return Ok(TextToCadOutcome::Clarification(TextToCadClarification {
            code: "CAD0-EMPTY-PROMPT".to_string(),
            message: "prompt is empty".to_string(),
            questions: vec![
                "What part should be generated (for example: bracket, enclosure, stand)?"
                    .to_string(),
                "What are the key dimensions?".to_string(),
            ],
        }));
    }

    if prompt_requires_clarification(prompt) {
        return Ok(TextToCadOutcome::Clarification(TextToCadClarification {
            code: "CAD0-AMBIGUOUS-PROMPT".to_string(),
            message: "prompt is under-specified for deterministic CAD generation".to_string(),
            questions: vec![
                "What part family should be generated (bracket, enclosure, stand, plate)?"
                    .to_string(),
                "What dimensions should be used?".to_string(),
                "Are there required hole counts or hole diameters?".to_string(),
            ],
        }));
    }

    let document = build_document_for_prompt(prompt, request.model);
    let compact_ir = cad_document_to_compact(&document)?;
    let canonical_ir = cad_document_from_compact(&compact_ir)?;
    let operation_count = canonical_ir.nodes.len();
    let deterministic_signature = stable_hex_digest(
        format!(
            "{}|{}|{}",
            request.model.as_str(),
            operation_count,
            compact_ir
        )
        .as_bytes(),
    );

    Ok(TextToCadOutcome::Generated(TextToCadGeneration {
        model: request.model,
        prompt: prompt.to_string(),
        compact_ir,
        ir: canonical_ir,
        operation_count,
        deterministic_signature,
    }))
}

fn prompt_requires_clarification(prompt: &str) -> bool {
    let lower = prompt.to_ascii_lowercase();
    if lower.split_whitespace().count() < 3 {
        return true;
    }
    let has_part_keyword = [
        "bracket",
        "stand",
        "enclosure",
        "box",
        "plate",
        "flange",
        "mount",
        "gear",
        "hinge",
    ]
    .iter()
    .any(|token| lower.contains(token));
    !has_part_keyword
}

fn build_document_for_prompt(prompt: &str, model: TextToCadModelProfile) -> CadMcpDocument {
    let lower = prompt.to_ascii_lowercase();
    if lower.contains("bracket") {
        return build_bracket_document(prompt, model);
    }
    if lower.contains("stand") {
        return build_stand_document(prompt, model);
    }
    if lower.contains("enclosure") || lower.contains("box") {
        return build_enclosure_document(prompt, model);
    }
    build_block_document(prompt, model)
}

fn build_bracket_document(prompt: &str, model: TextToCadModelProfile) -> CadMcpDocument {
    let numbers = extract_numeric_tokens(prompt);
    let length = clamp_dim(numbers.first().copied().unwrap_or(100.0), 40.0);
    let width = clamp_dim(numbers.get(1).copied().unwrap_or(50.0), 20.0);
    let thickness = clamp_dim(numbers.get(2).copied().unwrap_or(8.0), 2.0);
    let hole_diameter = clamp_dim(numbers.get(3).copied().unwrap_or(6.0), 2.0);
    let segments = model.cylinder_segments();

    let mut nodes = BTreeMap::new();
    nodes.insert(
        "10".to_string(),
        CadMcpNode {
            id: 10,
            name: Some("base".to_string()),
            op: CadMcpNodeOp::Cube {
                size: CadMcpVec3 {
                    x: length,
                    y: width,
                    z: thickness,
                },
            },
        },
    );
    nodes.insert(
        "11".to_string(),
        CadMcpNode {
            id: 11,
            name: Some("hole_template".to_string()),
            op: CadMcpNodeOp::Cylinder {
                radius: hole_diameter / 2.0,
                height: thickness * 2.0,
                segments,
            },
        },
    );
    nodes.insert(
        "12".to_string(),
        CadMcpNode {
            id: 12,
            name: None,
            op: CadMcpNodeOp::Translate {
                child: 11,
                offset: CadMcpVec3 {
                    x: -length * 0.3,
                    y: -width * 0.3,
                    z: 0.0,
                },
            },
        },
    );
    nodes.insert(
        "13".to_string(),
        CadMcpNode {
            id: 13,
            name: None,
            op: CadMcpNodeOp::Translate {
                child: 11,
                offset: CadMcpVec3 {
                    x: length * 0.3,
                    y: -width * 0.3,
                    z: 0.0,
                },
            },
        },
    );
    nodes.insert(
        "14".to_string(),
        CadMcpNode {
            id: 14,
            name: None,
            op: CadMcpNodeOp::Union {
                left: 12,
                right: 13,
            },
        },
    );

    let root_id = if model == TextToCadModelProfile::Cad0Mini {
        nodes.insert(
            "15".to_string(),
            CadMcpNode {
                id: 15,
                name: Some("bracket".to_string()),
                op: CadMcpNodeOp::Difference {
                    left: 10,
                    right: 14,
                },
            },
        );
        15
    } else {
        nodes.insert(
            "15".to_string(),
            CadMcpNode {
                id: 15,
                name: None,
                op: CadMcpNodeOp::Translate {
                    child: 11,
                    offset: CadMcpVec3 {
                        x: -length * 0.3,
                        y: width * 0.3,
                        z: 0.0,
                    },
                },
            },
        );
        nodes.insert(
            "16".to_string(),
            CadMcpNode {
                id: 16,
                name: None,
                op: CadMcpNodeOp::Union {
                    left: 14,
                    right: 15,
                },
            },
        );
        nodes.insert(
            "17".to_string(),
            CadMcpNode {
                id: 17,
                name: None,
                op: CadMcpNodeOp::Translate {
                    child: 11,
                    offset: CadMcpVec3 {
                        x: length * 0.3,
                        y: width * 0.3,
                        z: 0.0,
                    },
                },
            },
        );
        nodes.insert(
            "18".to_string(),
            CadMcpNode {
                id: 18,
                name: None,
                op: CadMcpNodeOp::Union {
                    left: 16,
                    right: 17,
                },
            },
        );
        nodes.insert(
            "19".to_string(),
            CadMcpNode {
                id: 19,
                name: Some("bracket".to_string()),
                op: CadMcpNodeOp::Difference {
                    left: 10,
                    right: 18,
                },
            },
        );
        19
    };

    document_with_root(nodes, root_id, "bracket")
}

fn build_stand_document(prompt: &str, model: TextToCadModelProfile) -> CadMcpDocument {
    let numbers = extract_numeric_tokens(prompt);
    let base_length = clamp_dim(numbers.first().copied().unwrap_or(120.0), 40.0);
    let base_width = clamp_dim(numbers.get(1).copied().unwrap_or(70.0), 25.0);
    let base_thickness = clamp_dim(numbers.get(2).copied().unwrap_or(8.0), 2.0);
    let back_height = clamp_dim(numbers.get(3).copied().unwrap_or(90.0), 30.0);

    let mut nodes = BTreeMap::new();
    nodes.insert(
        "10".to_string(),
        CadMcpNode {
            id: 10,
            name: Some("stand_base".to_string()),
            op: CadMcpNodeOp::Cube {
                size: CadMcpVec3 {
                    x: base_length,
                    y: base_width,
                    z: base_thickness,
                },
            },
        },
    );
    nodes.insert(
        "11".to_string(),
        CadMcpNode {
            id: 11,
            name: Some("stand_back".to_string()),
            op: CadMcpNodeOp::Cube {
                size: CadMcpVec3 {
                    x: base_length * 0.7,
                    y: base_thickness * 1.2,
                    z: back_height,
                },
            },
        },
    );
    nodes.insert(
        "12".to_string(),
        CadMcpNode {
            id: 12,
            name: None,
            op: CadMcpNodeOp::Translate {
                child: 11,
                offset: CadMcpVec3 {
                    x: 0.0,
                    y: base_width * 0.35,
                    z: back_height * 0.45,
                },
            },
        },
    );
    let root_id = if model == TextToCadModelProfile::Cad0 {
        nodes.insert(
            "13".to_string(),
            CadMcpNode {
                id: 13,
                name: None,
                op: CadMcpNodeOp::Rotate {
                    child: 12,
                    angles: CadMcpVec3 {
                        x: 0.0,
                        y: -15.0,
                        z: 0.0,
                    },
                },
            },
        );
        nodes.insert(
            "14".to_string(),
            CadMcpNode {
                id: 14,
                name: Some("phone_stand".to_string()),
                op: CadMcpNodeOp::Union {
                    left: 10,
                    right: 13,
                },
            },
        );
        14
    } else {
        nodes.insert(
            "13".to_string(),
            CadMcpNode {
                id: 13,
                name: Some("phone_stand".to_string()),
                op: CadMcpNodeOp::Union {
                    left: 10,
                    right: 12,
                },
            },
        );
        13
    };

    document_with_root(nodes, root_id, "phone_stand")
}

fn build_enclosure_document(prompt: &str, model: TextToCadModelProfile) -> CadMcpDocument {
    let numbers = extract_numeric_tokens(prompt);
    let outer_x = clamp_dim(numbers.first().copied().unwrap_or(120.0), 40.0);
    let outer_y = clamp_dim(numbers.get(1).copied().unwrap_or(80.0), 30.0);
    let outer_z = clamp_dim(numbers.get(2).copied().unwrap_or(50.0), 20.0);
    let wall = clamp_dim(numbers.get(3).copied().unwrap_or(3.0), 1.0);

    let mut nodes = BTreeMap::new();
    nodes.insert(
        "10".to_string(),
        CadMcpNode {
            id: 10,
            name: Some("outer".to_string()),
            op: CadMcpNodeOp::Cube {
                size: CadMcpVec3 {
                    x: outer_x,
                    y: outer_y,
                    z: outer_z,
                },
            },
        },
    );
    nodes.insert(
        "11".to_string(),
        CadMcpNode {
            id: 11,
            name: Some("inner".to_string()),
            op: CadMcpNodeOp::Cube {
                size: CadMcpVec3 {
                    x: (outer_x - wall * 2.0).max(outer_x * 0.3),
                    y: (outer_y - wall * 2.0).max(outer_y * 0.3),
                    z: (outer_z - wall * 2.0).max(outer_z * 0.3),
                },
            },
        },
    );
    nodes.insert(
        "12".to_string(),
        CadMcpNode {
            id: 12,
            name: None,
            op: CadMcpNodeOp::Translate {
                child: 11,
                offset: CadMcpVec3 {
                    x: 0.0,
                    y: 0.0,
                    z: wall,
                },
            },
        },
    );
    let root_id = if model == TextToCadModelProfile::Cad0 {
        nodes.insert(
            "13".to_string(),
            CadMcpNode {
                id: 13,
                name: Some("enclosure".to_string()),
                op: CadMcpNodeOp::Difference {
                    left: 10,
                    right: 12,
                },
            },
        );
        13
    } else {
        10
    };

    document_with_root(nodes, root_id, "enclosure")
}

fn build_block_document(prompt: &str, _model: TextToCadModelProfile) -> CadMcpDocument {
    let numbers = extract_numeric_tokens(prompt);
    let x = clamp_dim(numbers.first().copied().unwrap_or(60.0), 10.0);
    let y = clamp_dim(numbers.get(1).copied().unwrap_or(40.0), 10.0);
    let z = clamp_dim(numbers.get(2).copied().unwrap_or(20.0), 5.0);

    let mut nodes = BTreeMap::new();
    nodes.insert(
        "10".to_string(),
        CadMcpNode {
            id: 10,
            name: Some("generated_block".to_string()),
            op: CadMcpNodeOp::Cube {
                size: CadMcpVec3 { x, y, z },
            },
        },
    );
    document_with_root(nodes, 10, "generated_block")
}

fn document_with_root(
    nodes: BTreeMap<String, CadMcpNode>,
    root_id: u64,
    part_name: &str,
) -> CadMcpDocument {
    CadMcpDocument {
        version: "0.1".to_string(),
        nodes,
        materials: BTreeMap::from([("default".to_string(), CadMcpMaterial { density: None })]),
        roots: vec![CadMcpRoot {
            root: root_id,
            material: "default".to_string(),
        }],
        part_materials: BTreeMap::from([(part_name.to_string(), "default".to_string())]),
    }
}

fn clamp_dim(value: f64, min_value: f64) -> f64 {
    if !value.is_finite() {
        min_value
    } else {
        value.abs().max(min_value)
    }
}

fn extract_numeric_tokens(prompt: &str) -> Vec<f64> {
    let mut numbers = Vec::new();
    let mut current = String::new();

    for ch in prompt.chars() {
        if ch.is_ascii_digit() || ch == '.' || (ch == '-' && current.is_empty()) {
            current.push(ch);
            continue;
        }
        if !current.is_empty() {
            if let Ok(value) = current.parse::<f64>() {
                numbers.push(value);
            }
            current.clear();
        }
    }

    if !current.is_empty()
        && let Ok(value) = current.parse::<f64>()
    {
        numbers.push(value);
    }

    numbers
}

#[cfg(test)]
mod tests {
    use super::{
        TextToCadModelProfile, TextToCadOutcome, TextToCadRequest, text_to_cad,
        text_to_cad_from_prompt,
    };

    #[test]
    fn bracket_prompt_generates_compact_ir_document() {
        let outcome = text_to_cad_from_prompt("Design a bracket 100 60 8 with 6mm holes")
            .expect("text-to-cad generation should succeed");
        match outcome {
            TextToCadOutcome::Generated(generated) => {
                assert_eq!(generated.model, TextToCadModelProfile::Cad0);
                assert!(generated.compact_ir.contains("ROOT"));
                assert!(generated.operation_count >= 6);
                assert_eq!(generated.ir.roots.len(), 1);
            }
            other => panic!("expected generated outcome, got {other:?}"),
        }
    }

    #[test]
    fn cad0_mini_profile_is_more_compact_than_cad0() {
        let prompt = "Design a bracket 100 60 8 with 6mm holes";
        let full = text_to_cad(TextToCadRequest {
            prompt: prompt.to_string(),
            model: TextToCadModelProfile::Cad0,
        })
        .expect("cad0 generation should succeed");
        let mini = text_to_cad(TextToCadRequest {
            prompt: prompt.to_string(),
            model: TextToCadModelProfile::Cad0Mini,
        })
        .expect("cad0-mini generation should succeed");

        let full_ops = match full {
            TextToCadOutcome::Generated(generated) => generated.operation_count,
            other => panic!("expected generated outcome, got {other:?}"),
        };
        let mini_ops = match mini {
            TextToCadOutcome::Generated(generated) => generated.operation_count,
            other => panic!("expected generated outcome, got {other:?}"),
        };
        assert!(mini_ops < full_ops);
    }

    #[test]
    fn ambiguous_prompt_returns_clarification_questions() {
        let outcome = text_to_cad_from_prompt("make it better")
            .expect("text-to-cad call should succeed with clarification");
        match outcome {
            TextToCadOutcome::Clarification(clarification) => {
                assert_eq!(clarification.code, "CAD0-AMBIGUOUS-PROMPT");
                assert!(clarification.questions.len() >= 2);
            }
            other => panic!("expected clarification outcome, got {other:?}"),
        }
    }

    #[test]
    fn empty_prompt_returns_empty_prompt_code() {
        let outcome = text_to_cad_from_prompt("   ").expect("empty prompt should clarify");
        match outcome {
            TextToCadOutcome::Clarification(clarification) => {
                assert_eq!(clarification.code, "CAD0-EMPTY-PROMPT");
            }
            other => panic!("expected clarification outcome, got {other:?}"),
        }
    }

    #[test]
    fn same_prompt_replays_deterministically() {
        let prompt = "Design a phone stand 120 70 8";
        let first = text_to_cad_from_prompt(prompt).expect("first generation should succeed");
        let second = text_to_cad_from_prompt(prompt).expect("second generation should succeed");
        match (first, second) {
            (TextToCadOutcome::Generated(first), TextToCadOutcome::Generated(second)) => {
                assert_eq!(first.compact_ir, second.compact_ir);
                assert_eq!(
                    first.deterministic_signature,
                    second.deterministic_signature
                );
            }
            other => panic!("expected generated outcomes, got {other:?}"),
        }
    }
}
