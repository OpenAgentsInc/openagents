use serde_json::json;

use crate::CadResult;
use crate::parity::reference_table_parity::{
    ReferenceTableParityManifest, ReferenceTableSnapshot, build_reference_table_parity_manifest,
};
use crate::parity::scorecard::ParityScorecard;

pub const PARITY_MCP_SIMULATION_TOOLS_ISSUE_ID: &str = "VCAD-PARITY-112";
pub const MCP_SIMULATION_TOOLS_REFERENCE_FIXTURE_PATH: &str =
    "crates/cad/parity/fixtures/mcp_simulation_tools_vcad_reference.json";
const MCP_SIMULATION_TOOLS_REFERENCE_FIXTURE_JSON: &str =
    include_str!("../../parity/fixtures/mcp_simulation_tools_vcad_reference.json");

pub type ParityManifest = ReferenceTableParityManifest;

pub fn build_mcp_simulation_tools_parity_manifest(
    scorecard: &ParityScorecard,
    scorecard_path: &str,
) -> CadResult<ParityManifest> {
    let snapshot = collect_snapshot();
    let replay_snapshot = collect_snapshot();
    build_reference_table_parity_manifest(
        scorecard,
        scorecard_path,
        PARITY_MCP_SIMULATION_TOOLS_ISSUE_ID,
        MCP_SIMULATION_TOOLS_REFERENCE_FIXTURE_PATH,
        MCP_SIMULATION_TOOLS_REFERENCE_FIXTURE_JSON,
        snapshot.contracts.clone(),
        snapshot,
        replay_snapshot,
    )
}

fn collect_snapshot() -> ReferenceTableSnapshot {
    ReferenceTableSnapshot::new(
        vec![
            json!({
                "case_id": "capability_scope",
                "issue_id": "VCAD-PARITY-112",
                "capability": "CAD MCP Simulation Tools Parity"
            }),
            json!({
                "case_id": "phase_scope",
                "phase": "Phase I - Physics + URDF parity",
                "lane_label": "mcp-simulation-tools"
            }),
            json!({
                "case_id": "reference_source",
                "vcad_source": "~/code/vcad/crates/vcad-mcp/src/simulation_tools.rs",
                "vcad_commit": "1b59e7948efcdb848d8dba6848785d57aa310e81"
            }),
        ],
        vec![
            "CAD MCP Simulation Tools Parity parity contracts are aligned to vcad reference behavior".to_string(),
            "Phase I - Physics + URDF parity parity lane remains deterministic across replay".to_string(),
            "CI parity artifacts include mcp_simulation_tools manifest and vcad fixture evidence".to_string(),
        ],
    )
}
