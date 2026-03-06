#![allow(
    clippy::all,
    clippy::expect_used,
    clippy::panic,
    clippy::pedantic,
    clippy::print_stderr,
    clippy::print_stdout,
    clippy::unwrap_used
)]

use openagents_cad::compact_ir::{from_compact, to_compact};
use openagents_cad::mcp_tools::{
    CadMcpCreateFormat, CadMcpCreateInput, CadMcpOperation, CadMcpPartInput, CadMcpPrimitive,
    CadMcpPrimitiveType, CadMcpVec3, cad_document_from_text, create_cad_document,
};

fn sample_create_input() -> CadMcpCreateInput {
    CadMcpCreateInput {
        parts: vec![CadMcpPartInput {
            name: "compact_test_part".to_string(),
            primitive: CadMcpPrimitive {
                primitive_type: CadMcpPrimitiveType::Cube,
                size: Some(CadMcpVec3 {
                    x: 10.0,
                    y: 12.0,
                    z: 14.0,
                }),
                radius: None,
                height: None,
                segments: None,
                radius_bottom: None,
                radius_top: None,
            },
            operations: vec![CadMcpOperation::Translate {
                offset: CadMcpVec3 {
                    x: 5.0,
                    y: 0.0,
                    z: 0.0,
                },
            }],
            material: Some("aluminum".to_string()),
        }],
        format: Some(CadMcpCreateFormat::Compact),
    }
}

#[test]
fn compact_create_output_parses_and_roundtrips() {
    let response = create_cad_document(sample_create_input()).expect("create compact output");
    let compact = &response.content[0].text;
    let document = from_compact(compact).expect("parse compact create output");
    let roundtrip = to_compact(&document).expect("serialize compact roundtrip");
    let reparsed = from_compact(&roundtrip).expect("parse compact roundtrip output");
    assert_eq!(document, reparsed);
}

#[test]
fn cad_document_from_text_parses_json_and_compact() {
    let json_response = create_cad_document(CadMcpCreateInput {
        format: Some(CadMcpCreateFormat::Json),
        ..sample_create_input()
    })
    .expect("create json output");
    let compact_response = create_cad_document(sample_create_input()).expect("create compact");

    let from_json =
        cad_document_from_text(&json_response.content[0].text).expect("parse json text");
    let from_compact =
        cad_document_from_text(&compact_response.content[0].text).expect("parse compact text");

    assert!(!from_json.nodes.is_empty());
    assert!(!from_compact.nodes.is_empty());
    assert_eq!(from_json.roots.len(), 1);
    assert_eq!(from_compact.roots.len(), 1);
}

#[test]
fn compact_parser_reports_invalid_reference_errors() {
    let err = from_compact("C 10 10 10\nD 0 5").expect_err("invalid ref should fail");
    assert!(err.to_string().contains("nodes defined"));
}
