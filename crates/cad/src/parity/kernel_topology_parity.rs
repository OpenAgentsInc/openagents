use serde::{Deserialize, Serialize};

use crate::kernel_math::Point3;
use crate::kernel_topology::{Orientation, ShellType, Topology, TopologyCounts};
use crate::parity::scorecard::ParityScorecard;

pub const PARITY_KERNEL_TOPOLOGY_ISSUE_ID: &str = "VCAD-PARITY-013";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct KernelTopologyParityManifest {
    pub manifest_version: u64,
    pub issue_id: String,
    pub vcad_commit: String,
    pub openagents_commit: String,
    pub generated_from_scorecard: String,
    pub sample_counts: TopologyCounts,
    pub sample_invariants: KernelTopologyInvariants,
    pub topology_contracts: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct KernelTopologyInvariants {
    pub loop_ring_valid: bool,
    pub face_orientation: String,
    pub shell_type: String,
    pub solid_has_outer_shell: bool,
}

pub fn build_kernel_topology_parity_manifest(
    scorecard: &ParityScorecard,
    scorecard_path: &str,
) -> KernelTopologyParityManifest {
    let mut topology = Topology::new();
    let v1 = topology.add_vertex(Point3::new(0.0, 0.0, 0.0));
    let v2 = topology.add_vertex(Point3::new(1.0, 0.0, 0.0));
    let v3 = topology.add_vertex(Point3::new(1.0, 1.0, 0.0));
    let v4 = topology.add_vertex(Point3::new(0.0, 1.0, 0.0));

    let he1 = topology.add_half_edge(v1).expect("he1");
    let he2 = topology.add_half_edge(v2).expect("he2");
    let he3 = topology.add_half_edge(v3).expect("he3");
    let he4 = topology.add_half_edge(v4).expect("he4");

    let _edge_a = topology.add_edge(he1, he2).expect("edge a");
    let _edge_b = topology.add_edge(he3, he4).expect("edge b");

    let loop_id = topology.add_loop(&[he1, he2, he3, he4]).expect("loop");
    let loop_ring_valid = topology.validate_loop_ring(loop_id).is_ok();
    let face = topology
        .add_face(loop_id, 0, Orientation::Forward)
        .expect("face");
    let shell = topology
        .add_shell(vec![face], ShellType::Outer)
        .expect("shell");
    let solid = topology.add_solid(shell).expect("solid");

    let face_orientation = match topology.faces.get(&face).expect("face exists").orientation {
        Orientation::Forward => "Forward".to_string(),
        Orientation::Reversed => "Reversed".to_string(),
    };
    let shell_type = match topology
        .shells
        .get(&shell)
        .expect("shell exists")
        .shell_type
    {
        ShellType::Outer => "Outer".to_string(),
        ShellType::Void => "Void".to_string(),
    };
    let solid_has_outer_shell = topology
        .solids
        .get(&solid)
        .map(|solid| solid.outer_shell == shell)
        .unwrap_or(false);

    KernelTopologyParityManifest {
        manifest_version: 1,
        issue_id: PARITY_KERNEL_TOPOLOGY_ISSUE_ID.to_string(),
        vcad_commit: scorecard.vcad_commit.clone(),
        openagents_commit: scorecard.openagents_commit.clone(),
        generated_from_scorecard: scorecard_path.to_string(),
        sample_counts: topology.counts(),
        sample_invariants: KernelTopologyInvariants {
            loop_ring_valid,
            face_orientation,
            shell_type,
            solid_has_outer_shell,
        },
        topology_contracts: vec![
            "Vertex/HalfEdge/Edge/Loop/Face/Shell/Solid IDs are stable typed handles".to_string(),
            "Loop creation links next/prev pointers in a closed ring".to_string(),
            "Face references outer loop and optional inner loops".to_string(),
            "Shell references face set and shell type (Outer/Void)".to_string(),
            "Solid references outer shell and optional void shells".to_string(),
        ],
    }
}

#[cfg(test)]
mod tests {
    use super::{PARITY_KERNEL_TOPOLOGY_ISSUE_ID, build_kernel_topology_parity_manifest};
    use crate::parity::scorecard::{
        ParityScorecard, ScorecardCurrent, ScorecardEvaluation, ScorecardThresholdProfile,
    };

    fn mock_scorecard() -> ParityScorecard {
        ParityScorecard {
            manifest_version: 1,
            issue_id: "VCAD-PARITY-005".to_string(),
            vcad_commit: "vcad".to_string(),
            openagents_commit: "openagents".to_string(),
            generated_from_gap_matrix: "gap".to_string(),
            current: ScorecardCurrent {
                docs_match_rate: 0.0,
                crates_match_rate: 0.0,
                commands_match_rate: 0.0,
                overall_match_rate: 0.0,
                docs_reference_count: 0,
                crates_reference_count: 0,
                commands_reference_count: 0,
            },
            threshold_profiles: vec![ScorecardThresholdProfile {
                profile_id: "phase_a_baseline_v1".to_string(),
                docs_match_rate_min: 0.0,
                crates_match_rate_min: 0.0,
                commands_match_rate_min: 0.0,
                overall_match_rate_min: 0.0,
            }],
            evaluations: vec![ScorecardEvaluation {
                profile_id: "phase_a_baseline_v1".to_string(),
                docs_pass: true,
                crates_pass: true,
                commands_pass: true,
                overall_pass: true,
                pass: true,
            }],
        }
    }

    #[test]
    fn build_manifest_has_expected_issue_and_counts() {
        let manifest = build_kernel_topology_parity_manifest(&mock_scorecard(), "scorecard.json");
        assert_eq!(manifest.issue_id, PARITY_KERNEL_TOPOLOGY_ISSUE_ID);
        assert_eq!(manifest.sample_counts.vertex_count, 4);
        assert!(manifest.sample_invariants.loop_ring_valid);
        assert!(manifest.sample_invariants.solid_has_outer_shell);
    }
}
