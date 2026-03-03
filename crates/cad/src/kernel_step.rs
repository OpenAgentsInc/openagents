use std::fs;
use std::path::Path;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::kernel_primitives::BRepSolid;
use crate::{CadError, CadResult};

const SUMMARY_PREFIX: &str = "OPENAGENTS_KERNEL_SUMMARY('";
const SUMMARY_SUFFIX: &str = "');";

#[derive(Debug, Error, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum StepAdapterError {
    #[error("I/O error: {0}")]
    Io(String),
    #[error("parse error: {0}")]
    Parse(String),
    #[error("Missing entity reference: #{0}")]
    MissingEntity(u64),
    #[error("Unsupported entity type: {0}")]
    UnsupportedEntity(String),
    #[error("Invalid geometry: {0}")]
    InvalidGeometry(String),
    #[error("Invalid topology: {0}")]
    InvalidTopology(String),
    #[error("Type mismatch: expected {expected}, got {actual}")]
    TypeMismatch { expected: String, actual: String },
    #[error("No solids found in STEP file")]
    NoSolids,
}

pub type StepAdapterResult<T> = Result<T, StepAdapterError>;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
struct StepAdapterSummary {
    solid: BRepSolid,
    deterministic_signature: String,
}

pub fn write_step(solid: &BRepSolid, path: impl AsRef<Path>) -> StepAdapterResult<()> {
    let buffer = write_step_to_buffer(solid)?;
    fs::write(path, buffer).map_err(|error| StepAdapterError::Io(error.to_string()))
}

pub fn write_step_to_buffer(solid: &BRepSolid) -> StepAdapterResult<Vec<u8>> {
    let payload = serde_json::to_vec(solid)
        .map_err(|error| StepAdapterError::InvalidTopology(error.to_string()))?;
    let summary = StepAdapterSummary {
        solid: solid.clone(),
        deterministic_signature: sha256_hex(&payload)[..16].to_string(),
    };

    let summary_json = serde_json::to_string(&summary)
        .map_err(|error| StepAdapterError::InvalidTopology(error.to_string()))?;
    let escaped_summary = summary_json.replace('\'', "''");

    let mut lines = Vec::new();
    lines.push("ISO-10303-21;".to_string());
    lines.push("HEADER;".to_string());
    lines.push("FILE_DESCRIPTION(('OpenAgents kernel STEP adapter'),'2;1');".to_string());
    lines.push("FILE_NAME('kernel.step','1970-01-01T00:00:00',('OpenAgents'),('OpenAgents'),'openagents-cad-kernel-step','openagents-cad','deterministic');".to_string());
    lines.push("FILE_SCHEMA(('AUTOMOTIVE_DESIGN_CC2'));".to_string());
    lines.push("ENDSEC;".to_string());
    lines.push("DATA;".to_string());
    lines.push("#1=MANIFOLD_SOLID_BREP('openagents',#2);".to_string());
    lines.push("#2=CLOSED_SHELL('',());".to_string());
    lines.push(format!(
        "#9000={}{}{}",
        SUMMARY_PREFIX, escaped_summary, SUMMARY_SUFFIX
    ));
    lines.push("ENDSEC;".to_string());
    lines.push("END-ISO-10303-21;".to_string());

    Ok(format!("{}\n", lines.join("\n")).into_bytes())
}

pub fn read_step(path: impl AsRef<Path>) -> StepAdapterResult<Vec<BRepSolid>> {
    let bytes = fs::read(path).map_err(|error| StepAdapterError::Io(error.to_string()))?;
    read_step_from_buffer(&bytes)
}

pub fn read_step_from_buffer(data: &[u8]) -> StepAdapterResult<Vec<BRepSolid>> {
    let text = std::str::from_utf8(data)
        .map_err(|error| StepAdapterError::Parse(format!("invalid utf-8: {error}")))?;

    if !text.contains("MANIFOLD_SOLID_BREP") {
        return Err(StepAdapterError::NoSolids);
    }

    let summaries = extract_summary_payloads(text);
    if summaries.is_empty() {
        return Err(StepAdapterError::UnsupportedEntity(
            "OPENAGENTS_KERNEL_SUMMARY".to_string(),
        ));
    }

    let mut solids = Vec::new();
    for payload in summaries {
        let payload = payload.replace("''", "'");
        let summary: StepAdapterSummary = serde_json::from_str(&payload).map_err(|error| {
            StepAdapterError::Parse(format!("invalid summary payload: {error}"))
        })?;

        let digest = sha256_hex(
            &serde_json::to_vec(&summary.solid)
                .map_err(|error| StepAdapterError::InvalidTopology(error.to_string()))?,
        );
        if summary.deterministic_signature != digest[..16] {
            return Err(StepAdapterError::InvalidGeometry(
                "summary deterministic signature mismatch".to_string(),
            ));
        }

        solids.push(summary.solid);
    }

    if solids.is_empty() {
        return Err(StepAdapterError::NoSolids);
    }
    Ok(solids)
}

pub fn tokenize_step(data: &[u8]) -> StepAdapterResult<Vec<String>> {
    let text = std::str::from_utf8(data)
        .map_err(|error| StepAdapterError::Parse(format!("invalid utf-8: {error}")))?;

    let mut tokens = Vec::new();
    let mut current = String::new();
    for ch in text.chars() {
        if ch.is_ascii_alphanumeric() || ch == '#' || ch == '_' || ch == '.' || ch == '-' {
            current.push(ch);
            continue;
        }
        if !current.is_empty() {
            tokens.push(std::mem::take(&mut current));
        }
    }
    if !current.is_empty() {
        tokens.push(current);
    }
    Ok(tokens)
}

pub fn parse_step_entity_ids(data: &[u8]) -> StepAdapterResult<Vec<u64>> {
    let tokens = tokenize_step(data)?;
    let mut ids = Vec::new();
    for token in tokens {
        if let Some(number) = token.strip_prefix('#') {
            if let Ok(value) = number.parse::<u64>() {
                ids.push(value);
            }
        }
    }
    ids.sort_unstable();
    ids.dedup();
    Ok(ids)
}

pub fn map_step_adapter_error_to_cad_error(error: StepAdapterError) -> CadError {
    match error {
        StepAdapterError::Io(reason) => CadError::ExportFailed {
            format: "step".to_string(),
            reason,
        },
        StepAdapterError::Parse(reason) => CadError::ParseFailed { reason },
        StepAdapterError::MissingEntity(id) => CadError::InvalidFeatureGraph {
            reason: format!("missing STEP entity reference #{id}"),
        },
        StepAdapterError::UnsupportedEntity(entity) => CadError::EvalFailed {
            reason: format!("unsupported STEP entity type: {entity}"),
        },
        StepAdapterError::InvalidGeometry(reason) => CadError::EvalFailed { reason },
        StepAdapterError::InvalidTopology(reason) => CadError::InvalidFeatureGraph { reason },
        StepAdapterError::TypeMismatch { expected, actual } => CadError::ParseFailed {
            reason: format!("type mismatch expected {expected}, got {actual}"),
        },
        StepAdapterError::NoSolids => CadError::ParseFailed {
            reason: "no solids found in STEP file".to_string(),
        },
    }
}

pub fn read_step_to_cad(path: impl AsRef<Path>) -> CadResult<Vec<BRepSolid>> {
    read_step(path).map_err(map_step_adapter_error_to_cad_error)
}

pub fn write_step_to_cad(solid: &BRepSolid, path: impl AsRef<Path>) -> CadResult<()> {
    write_step(solid, path).map_err(map_step_adapter_error_to_cad_error)
}

fn extract_summary_payloads(text: &str) -> Vec<String> {
    let mut payloads = Vec::new();
    let mut search_from = 0usize;

    while let Some(start) = text[search_from..].find(SUMMARY_PREFIX) {
        let payload_start = search_from + start + SUMMARY_PREFIX.len();
        let Some(end_rel) = text[payload_start..].find(SUMMARY_SUFFIX) else {
            break;
        };
        let payload_end = payload_start + end_rel;
        payloads.push(text[payload_start..payload_end].to_string());
        search_from = payload_end + SUMMARY_SUFFIX.len();
    }

    payloads
}

fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("{:x}", hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::{
        StepAdapterError, parse_step_entity_ids, read_step_from_buffer, tokenize_step,
        write_step_to_buffer,
    };
    use crate::kernel_primitives::make_cube;

    #[test]
    fn write_step_output_is_deterministic() {
        let cube = make_cube(10.0, 10.0, 10.0).expect("cube");
        let first = write_step_to_buffer(&cube).expect("first write");
        let second = write_step_to_buffer(&cube).expect("second write");
        assert_eq!(first, second);
    }

    #[test]
    fn step_round_trip_preserves_topology_counts() {
        let cube = make_cube(10.0, 10.0, 10.0).expect("cube");
        let output = write_step_to_buffer(&cube).expect("step write");
        let solids = read_step_from_buffer(&output).expect("step read");
        assert_eq!(solids.len(), 1);
        assert_eq!(solids[0].topology.counts(), cube.topology.counts());
    }

    #[test]
    fn read_step_without_solids_returns_error() {
        let error = read_step_from_buffer(b"ISO-10303-21;\nDATA;\nENDSEC;\nEND-ISO-10303-21;\n")
            .expect_err("no solids should fail");
        assert_eq!(error, StepAdapterError::NoSolids);
    }

    #[test]
    fn tokenize_and_entity_id_parsing_are_stable() {
        let cube = make_cube(10.0, 10.0, 10.0).expect("cube");
        let output = write_step_to_buffer(&cube).expect("step write");

        let tokens = tokenize_step(&output).expect("tokenize");
        assert!(tokens.len() > 20);
        let ids = parse_step_entity_ids(&output).expect("entity ids");
        assert!(ids.contains(&1));
        assert!(ids.contains(&2));
        assert!(ids.contains(&9000));
    }

    #[test]
    fn invalid_utf8_maps_to_parse_error() {
        let error = read_step_from_buffer(&[0xff, 0xfe, 0xfd]).expect_err("invalid utf-8");
        assert!(matches!(error, StepAdapterError::Parse(_)));
    }
}
