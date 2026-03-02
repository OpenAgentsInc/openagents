use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::CadResult;
use crate::kernel::{
    CadKernelAdapter, CadKernelAdapterV2, KernelAdapterV2Bridge, KernelOperationContext,
    openagents_kernel_adapter_v2_descriptor,
};
use crate::parity::scorecard::ParityScorecard;
use crate::primitives::{
    BoxPrimitive, ConePrimitive, CylinderPrimitive, PrimitiveSpec, SpherePrimitive, build_primitive,
};

pub const PARITY_PRIMITIVE_CONTRACTS_ISSUE_ID: &str = "VCAD-PARITY-026";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PrimitiveContractsParityManifest {
    pub manifest_version: u64,
    pub issue_id: String,
    pub vcad_commit: String,
    pub openagents_commit: String,
    pub generated_from_scorecard: String,
    pub routing_counts: PrimitiveRoutingCounts,
    pub cube_snapshot: PrimitiveSnapshot,
    pub cylinder_snapshot: PrimitiveSnapshot,
    pub sphere_snapshot: PrimitiveSnapshot,
    pub cone_pointed_snapshot: PrimitiveSnapshot,
    pub cone_frustum_snapshot: PrimitiveSnapshot,
    pub deterministic_replay_match: bool,
    pub bridge_receipts: Vec<PrimitiveReceiptSnapshot>,
    pub invalid_sphere_error: String,
    pub invalid_cone_error: String,
    pub deterministic_signature: String,
    pub parity_contracts: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct PrimitiveRoutingCounts {
    pub box_calls: usize,
    pub cylinder_calls: usize,
    pub sphere_calls: usize,
    pub cone_calls: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PrimitiveSnapshot {
    pub kind: String,
    pub solid_handle: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PrimitiveReceiptSnapshot {
    pub operation: String,
    pub engine_id: String,
    pub deterministic: bool,
    pub diagnostics: Vec<String>,
}

pub fn build_primitive_contracts_parity_manifest(
    scorecard: &ParityScorecard,
    scorecard_path: &str,
) -> PrimitiveContractsParityManifest {
    let cube = PrimitiveSpec::Box(BoxPrimitive {
        width_mm: 10.0,
        depth_mm: 20.0,
        height_mm: 30.0,
    });
    let cylinder = PrimitiveSpec::Cylinder(CylinderPrimitive {
        radius_mm: 5.0,
        height_mm: 15.0,
    });
    let sphere = PrimitiveSpec::Sphere(SpherePrimitive { radius_mm: 7.5 });
    let cone_pointed = PrimitiveSpec::Cone(ConePrimitive {
        radius_bottom_mm: 5.0,
        radius_top_mm: 0.0,
        height_mm: 12.0,
    });
    let cone_frustum = PrimitiveSpec::Cone(ConePrimitive {
        radius_bottom_mm: 5.0,
        radius_top_mm: 3.0,
        height_mm: 12.0,
    });

    let mut kernel = PrimitiveParityKernel::default();
    let cube_handle = build_primitive(&mut kernel, cube).expect("cube build");
    let cylinder_handle = build_primitive(&mut kernel, cylinder).expect("cylinder build");
    let sphere_handle = build_primitive(&mut kernel, sphere).expect("sphere build");
    let pointed_handle = build_primitive(&mut kernel, cone_pointed).expect("pointed cone build");
    let frustum_handle = build_primitive(&mut kernel, cone_frustum).expect("frustum cone build");

    let replay_match = {
        let mut replay = PrimitiveParityKernel::default();
        let first = vec![
            build_primitive(&mut replay, cube).expect("replay cube"),
            build_primitive(&mut replay, cylinder).expect("replay cylinder"),
            build_primitive(&mut replay, sphere).expect("replay sphere"),
            build_primitive(&mut replay, cone_pointed).expect("replay pointed cone"),
            build_primitive(&mut replay, cone_frustum).expect("replay frustum cone"),
        ];
        first
            == vec![
                cube_handle.clone(),
                cylinder_handle.clone(),
                sphere_handle.clone(),
                pointed_handle.clone(),
                frustum_handle.clone(),
            ]
    };

    let invalid_sphere_error = build_primitive(
        &mut PrimitiveParityKernel::default(),
        PrimitiveSpec::Sphere(SpherePrimitive { radius_mm: 0.0 }),
    )
    .expect_err("invalid sphere should fail")
    .to_string();

    let invalid_cone_error = build_primitive(
        &mut PrimitiveParityKernel::default(),
        PrimitiveSpec::Cone(ConePrimitive {
            radius_bottom_mm: 5.0,
            radius_top_mm: -1.0,
            height_mm: 10.0,
        }),
    )
    .expect_err("invalid cone should fail")
    .to_string();

    let bridge_receipts = build_bridge_receipts();

    let routing_counts = PrimitiveRoutingCounts {
        box_calls: kernel.box_calls,
        cylinder_calls: kernel.cylinder_calls,
        sphere_calls: kernel.sphere_calls,
        cone_calls: kernel.cone_calls,
    };

    let cube_snapshot = PrimitiveSnapshot {
        kind: "cube".to_string(),
        solid_handle: cube_handle,
    };
    let cylinder_snapshot = PrimitiveSnapshot {
        kind: "cylinder".to_string(),
        solid_handle: cylinder_handle,
    };
    let sphere_snapshot = PrimitiveSnapshot {
        kind: "sphere".to_string(),
        solid_handle: sphere_handle,
    };
    let cone_pointed_snapshot = PrimitiveSnapshot {
        kind: "cone_pointed".to_string(),
        solid_handle: pointed_handle,
    };
    let cone_frustum_snapshot = PrimitiveSnapshot {
        kind: "cone_frustum".to_string(),
        solid_handle: frustum_handle,
    };

    let deterministic_signature = parity_signature(
        &routing_counts,
        &cube_snapshot,
        &cylinder_snapshot,
        &sphere_snapshot,
        &cone_pointed_snapshot,
        &cone_frustum_snapshot,
        replay_match,
        &bridge_receipts,
        &invalid_sphere_error,
        &invalid_cone_error,
    );

    PrimitiveContractsParityManifest {
        manifest_version: 1,
        issue_id: PARITY_PRIMITIVE_CONTRACTS_ISSUE_ID.to_string(),
        vcad_commit: scorecard.vcad_commit.clone(),
        openagents_commit: scorecard.openagents_commit.clone(),
        generated_from_scorecard: scorecard_path.to_string(),
        routing_counts,
        cube_snapshot,
        cylinder_snapshot,
        sphere_snapshot,
        cone_pointed_snapshot,
        cone_frustum_snapshot,
        deterministic_replay_match: replay_match,
        bridge_receipts,
        invalid_sphere_error,
        invalid_cone_error,
        deterministic_signature,
        parity_contracts: vec![
            "PrimitiveSpec now supports cube/cylinder/sphere/cone parity contracts".to_string(),
            "build_primitive routes each primitive to deterministic kernel adapter calls"
                .to_string(),
            "v2 adapter bridge emits deterministic receipts for sphere and cone operations"
                .to_string(),
            "invalid primitive dimensions map to stable CadError::InvalidPrimitive semantics"
                .to_string(),
            "primitive routing replay is deterministic across repeated runs".to_string(),
        ],
    }
}

#[derive(Default)]
struct PrimitiveParityKernel {
    box_calls: usize,
    cylinder_calls: usize,
    sphere_calls: usize,
    cone_calls: usize,
}

impl CadKernelAdapter for PrimitiveParityKernel {
    type Solid = String;

    fn create_box(&mut self, primitive: &BoxPrimitive) -> CadResult<Self::Solid> {
        self.box_calls = self.box_calls.saturating_add(1);
        Ok(primitive_handle(
            "cube",
            &[primitive.width_mm, primitive.depth_mm, primitive.height_mm],
        ))
    }

    fn create_cylinder(&mut self, primitive: &CylinderPrimitive) -> CadResult<Self::Solid> {
        self.cylinder_calls = self.cylinder_calls.saturating_add(1);
        Ok(primitive_handle(
            "cylinder",
            &[primitive.radius_mm, primitive.height_mm],
        ))
    }

    fn create_sphere(&mut self, primitive: &SpherePrimitive) -> CadResult<Self::Solid> {
        self.sphere_calls = self.sphere_calls.saturating_add(1);
        Ok(primitive_handle("sphere", &[primitive.radius_mm]))
    }

    fn create_cone(&mut self, primitive: &ConePrimitive) -> CadResult<Self::Solid> {
        self.cone_calls = self.cone_calls.saturating_add(1);
        Ok(primitive_handle(
            "cone",
            &[
                primitive.radius_bottom_mm,
                primitive.radius_top_mm,
                primitive.height_mm,
            ],
        ))
    }
}

fn primitive_handle(kind: &str, values: &[f64]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(kind.as_bytes());
    for value in values {
        hasher.update(format!("{value:.9}").as_bytes());
    }
    format!("{kind}:{}", &format!("{:x}", hasher.finalize())[..12])
}

fn build_bridge_receipts() -> Vec<PrimitiveReceiptSnapshot> {
    let mut bridge = KernelAdapterV2Bridge::new(
        PrimitiveParityKernel::default(),
        openagents_kernel_adapter_v2_descriptor(),
    );

    let sphere_context = KernelOperationContext::new("req-sphere", "primitive.sphere", 0.001);
    let cone_context = KernelOperationContext::new("req-cone", "primitive.cone", 0.001);

    let sphere_receipt = bridge
        .create_sphere_v2(&SpherePrimitive { radius_mm: 8.0 }, &sphere_context)
        .expect("sphere receipt")
        .receipt;
    let cone_receipt = bridge
        .create_cone_v2(
            &ConePrimitive {
                radius_bottom_mm: 6.0,
                radius_top_mm: 2.0,
                height_mm: 14.0,
            },
            &cone_context,
        )
        .expect("cone receipt")
        .receipt;

    vec![
        PrimitiveReceiptSnapshot {
            operation: sphere_receipt.operation,
            engine_id: sphere_receipt.engine_id,
            deterministic: sphere_receipt.deterministic,
            diagnostics: sphere_receipt.diagnostics,
        },
        PrimitiveReceiptSnapshot {
            operation: cone_receipt.operation,
            engine_id: cone_receipt.engine_id,
            deterministic: cone_receipt.deterministic,
            diagnostics: cone_receipt.diagnostics,
        },
    ]
}

#[allow(clippy::too_many_arguments)]
fn parity_signature(
    routing_counts: &PrimitiveRoutingCounts,
    cube_snapshot: &PrimitiveSnapshot,
    cylinder_snapshot: &PrimitiveSnapshot,
    sphere_snapshot: &PrimitiveSnapshot,
    cone_pointed_snapshot: &PrimitiveSnapshot,
    cone_frustum_snapshot: &PrimitiveSnapshot,
    replay_match: bool,
    bridge_receipts: &[PrimitiveReceiptSnapshot],
    invalid_sphere_error: &str,
    invalid_cone_error: &str,
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(
        serde_json::to_vec(&(
            routing_counts,
            cube_snapshot,
            cylinder_snapshot,
            sphere_snapshot,
            cone_pointed_snapshot,
            cone_frustum_snapshot,
            replay_match,
            bridge_receipts,
            invalid_sphere_error,
            invalid_cone_error,
        ))
        .expect("serialize primitive parity signature payload"),
    );
    format!("{:x}", hasher.finalize())[..16].to_string()
}

#[cfg(test)]
mod tests {
    use super::{PARITY_PRIMITIVE_CONTRACTS_ISSUE_ID, build_primitive_contracts_parity_manifest};
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
    fn build_manifest_tracks_primitive_contracts() {
        let manifest =
            build_primitive_contracts_parity_manifest(&mock_scorecard(), "scorecard.json");
        assert_eq!(manifest.issue_id, PARITY_PRIMITIVE_CONTRACTS_ISSUE_ID);
        assert_eq!(manifest.routing_counts.box_calls, 1);
        assert_eq!(manifest.routing_counts.cylinder_calls, 1);
        assert_eq!(manifest.routing_counts.sphere_calls, 1);
        assert_eq!(manifest.routing_counts.cone_calls, 2);
        assert!(manifest.deterministic_replay_match);
        assert_eq!(manifest.bridge_receipts.len(), 2);
        assert!(manifest.invalid_sphere_error.contains("sphere radius"));
        assert!(manifest.invalid_cone_error.contains("radius_top"));
    }
}
