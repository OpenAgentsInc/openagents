use serde::{Deserialize, Serialize};

pub const PSION_ACTUAL_PRETRAINING_LANE_ID: &str = "psion_actual_pretraining_v1";
pub const PSION_APPLE_WINDOWED_TRAINING_LANE_ID: &str = "psion_apple_windowed_training_v1";
pub const PSION_CS336_A1_DEMO_LANE_ID: &str = "psion_cs336_a1_demo_v1";
pub const PSION_A1_MINIMAL_DISTRIBUTED_LM_LANE_ID: &str = "a1_minimal_distributed_lm_001";
pub const PSION_QWEN_LEGAL_ADAPTER_SFT_LANE_ID: &str = "qwen_legal_adapter_sft_v1";

const PSIONIC_TRAIN_ACTUAL_PRETRAINING_RELEASE_ID: &str =
    "psionic-train.psion_actual_pretraining.release.v1";
const PSIONIC_TRAIN_ACTUAL_PRETRAINING_ENVIRONMENT_REF: &str =
    "psionic.environment.psion_actual_pretraining.cuda_h100.operator@v1";
const PSIONIC_TRAIN_ACTUAL_PRETRAINING_BACKEND_FAMILY: &str = "cuda";
const PSIONIC_TRAIN_ACTUAL_PRETRAINING_TOPOLOGY_CLASS: &str =
    "homogeneous_four_node_h100_tensor_parallel";

const PSIONIC_TRAIN_APPLE_WINDOWED_TRAINING_RELEASE_ID: &str =
    "psionic-train.psion_apple_windowed_training.release.v1";
const PSIONIC_TRAIN_APPLE_WINDOWED_TRAINING_ENVIRONMENT_REF: &str =
    "psionic.environment.psion_apple_windowed_training.metal_mlx.operator@v1";
const PSIONIC_TRAIN_APPLE_WINDOWED_TRAINING_BACKEND_FAMILY: &str = "metal";
const PSIONIC_TRAIN_APPLE_WINDOWED_TRAINING_TOPOLOGY_CLASS: &str =
    "homogeneous_apple_silicon_data_parallel";

const PSIONIC_TRAIN_CS336_A1_DEMO_RELEASE_ID: &str = "psionic-train.psion_cs336_a1_demo.release.v1";
const PSIONIC_TRAIN_CS336_A1_DEMO_ENVIRONMENT_REF: &str =
    "psionic.environment.psion_cs336_a1_demo.host_cpu.operator@v1";
const PSIONIC_TRAIN_CS336_A1_DEMO_BACKEND_FAMILY: &str = "cpu";
const PSIONIC_TRAIN_CS336_A1_DEMO_TOPOLOGY_CLASS: &str = "single_host_cpu_reference";

const PSIONIC_TRAIN_A1_MINIMAL_DISTRIBUTED_LM_RELEASE_ID: &str =
    "psionic-train.a1_minimal_distributed_lm.release.v1";
const PSIONIC_TRAIN_A1_MINIMAL_DISTRIBUTED_LM_ENVIRONMENT_REF: &str =
    "psionic.environment.a1_minimal_distributed_lm.tiny_lm.operator@v1";
const PSIONIC_TRAIN_A1_MINIMAL_DISTRIBUTED_LM_BACKEND_FAMILY: &str = "cpu";
const PSIONIC_TRAIN_A1_MINIMAL_DISTRIBUTED_LM_TOPOLOGY_CLASS: &str =
    "multi_pylon_tiny_lm_local_update";
const PSIONIC_TRAIN_QWEN_LEGAL_ADAPTER_SFT_RELEASE_ID: &str =
    "psionic-train.qwen_legal_adapter_sft.release.v1";
const PSIONIC_TRAIN_QWEN_LEGAL_ADAPTER_SFT_ENVIRONMENT_REF: &str =
    "psionic.environment.qwen_legal_adapter_sft.cuda.operator@v1";
const PSIONIC_TRAIN_QWEN_LEGAL_ADAPTER_SFT_BACKEND_FAMILY: &str = "cuda";
const PSIONIC_TRAIN_QWEN_LEGAL_ADAPTER_SFT_TOPOLOGY_CLASS: &str = "single_node_cuda_lora_sft";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PsionicTrainMinimumMachineClass {
    ReferenceHostCpuOperator,
    CrossPlatformCpuCompatibleOperator,
    AppleSiliconOperator,
    StrongCudaTrainer,
}

impl PsionicTrainMinimumMachineClass {
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::ReferenceHostCpuOperator => "reference_host_cpu_operator",
            Self::CrossPlatformCpuCompatibleOperator => "cross_platform_cpu_compatible_operator",
            Self::AppleSiliconOperator => "apple_silicon_operator",
            Self::StrongCudaTrainer => "strong_cuda_trainer",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionicTrainLaneContract {
    pub lane_id: String,
    pub release_id: String,
    pub environment_ref: String,
    pub backend_family: String,
    pub topology_class: String,
    pub minimum_machine_class: PsionicTrainMinimumMachineClass,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct PsionicTrainLaneContractStatic {
    lane_id: &'static str,
    release_id: &'static str,
    environment_ref: &'static str,
    backend_family: &'static str,
    topology_class: &'static str,
    minimum_machine_class: PsionicTrainMinimumMachineClass,
}

impl PsionicTrainLaneContract {
    pub fn for_lane(lane_id: &str) -> Result<Self, String> {
        let contract = canonical_lane_contract_for_lane(lane_id)?;
        Ok(Self {
            lane_id: String::from(contract.lane_id),
            release_id: String::from(contract.release_id),
            environment_ref: String::from(contract.environment_ref),
            backend_family: String::from(contract.backend_family),
            topology_class: String::from(contract.topology_class),
            minimum_machine_class: contract.minimum_machine_class,
        })
    }
}

fn canonical_lane_contract_for_lane(
    lane_id: &str,
) -> Result<PsionicTrainLaneContractStatic, String> {
    match lane_id {
        PSION_ACTUAL_PRETRAINING_LANE_ID => Ok(PsionicTrainLaneContractStatic {
            lane_id: PSION_ACTUAL_PRETRAINING_LANE_ID,
            release_id: PSIONIC_TRAIN_ACTUAL_PRETRAINING_RELEASE_ID,
            environment_ref: PSIONIC_TRAIN_ACTUAL_PRETRAINING_ENVIRONMENT_REF,
            backend_family: PSIONIC_TRAIN_ACTUAL_PRETRAINING_BACKEND_FAMILY,
            topology_class: PSIONIC_TRAIN_ACTUAL_PRETRAINING_TOPOLOGY_CLASS,
            minimum_machine_class: PsionicTrainMinimumMachineClass::StrongCudaTrainer,
        }),
        PSION_CS336_A1_DEMO_LANE_ID => Ok(PsionicTrainLaneContractStatic {
            lane_id: PSION_CS336_A1_DEMO_LANE_ID,
            release_id: PSIONIC_TRAIN_CS336_A1_DEMO_RELEASE_ID,
            environment_ref: PSIONIC_TRAIN_CS336_A1_DEMO_ENVIRONMENT_REF,
            backend_family: PSIONIC_TRAIN_CS336_A1_DEMO_BACKEND_FAMILY,
            topology_class: PSIONIC_TRAIN_CS336_A1_DEMO_TOPOLOGY_CLASS,
            minimum_machine_class:
                PsionicTrainMinimumMachineClass::CrossPlatformCpuCompatibleOperator,
        }),
        PSION_A1_MINIMAL_DISTRIBUTED_LM_LANE_ID => Ok(PsionicTrainLaneContractStatic {
            lane_id: PSION_A1_MINIMAL_DISTRIBUTED_LM_LANE_ID,
            release_id: PSIONIC_TRAIN_A1_MINIMAL_DISTRIBUTED_LM_RELEASE_ID,
            environment_ref: PSIONIC_TRAIN_A1_MINIMAL_DISTRIBUTED_LM_ENVIRONMENT_REF,
            backend_family: PSIONIC_TRAIN_A1_MINIMAL_DISTRIBUTED_LM_BACKEND_FAMILY,
            topology_class: PSIONIC_TRAIN_A1_MINIMAL_DISTRIBUTED_LM_TOPOLOGY_CLASS,
            minimum_machine_class:
                PsionicTrainMinimumMachineClass::CrossPlatformCpuCompatibleOperator,
        }),
        PSION_APPLE_WINDOWED_TRAINING_LANE_ID => Ok(PsionicTrainLaneContractStatic {
            lane_id: PSION_APPLE_WINDOWED_TRAINING_LANE_ID,
            release_id: PSIONIC_TRAIN_APPLE_WINDOWED_TRAINING_RELEASE_ID,
            environment_ref: PSIONIC_TRAIN_APPLE_WINDOWED_TRAINING_ENVIRONMENT_REF,
            backend_family: PSIONIC_TRAIN_APPLE_WINDOWED_TRAINING_BACKEND_FAMILY,
            topology_class: PSIONIC_TRAIN_APPLE_WINDOWED_TRAINING_TOPOLOGY_CLASS,
            minimum_machine_class: PsionicTrainMinimumMachineClass::AppleSiliconOperator,
        }),
        PSION_QWEN_LEGAL_ADAPTER_SFT_LANE_ID => Ok(PsionicTrainLaneContractStatic {
            lane_id: PSION_QWEN_LEGAL_ADAPTER_SFT_LANE_ID,
            release_id: PSIONIC_TRAIN_QWEN_LEGAL_ADAPTER_SFT_RELEASE_ID,
            environment_ref: PSIONIC_TRAIN_QWEN_LEGAL_ADAPTER_SFT_ENVIRONMENT_REF,
            backend_family: PSIONIC_TRAIN_QWEN_LEGAL_ADAPTER_SFT_BACKEND_FAMILY,
            topology_class: PSIONIC_TRAIN_QWEN_LEGAL_ADAPTER_SFT_TOPOLOGY_CLASS,
            minimum_machine_class: PsionicTrainMinimumMachineClass::StrongCudaTrainer,
        }),
        other => Err(format!(
            "lane `{other}` has no canonical machine-runtime lane contract"
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::{
        PSION_A1_MINIMAL_DISTRIBUTED_LM_LANE_ID, PSION_ACTUAL_PRETRAINING_LANE_ID,
        PSION_CS336_A1_DEMO_LANE_ID, PSION_QWEN_LEGAL_ADAPTER_SFT_LANE_ID,
        PsionicTrainLaneContract, PsionicTrainMinimumMachineClass,
    };

    #[test]
    fn resolves_cs336_demo_lane_contract() {
        let contract = PsionicTrainLaneContract::for_lane(PSION_CS336_A1_DEMO_LANE_ID)
            .expect("CS336 demo lane contract should resolve");
        assert_eq!(contract.lane_id, PSION_CS336_A1_DEMO_LANE_ID);
        assert_eq!(
            contract.environment_ref,
            "psionic.environment.psion_cs336_a1_demo.host_cpu.operator@v1"
        );
        assert_eq!(contract.backend_family, "cpu");
        assert_eq!(contract.topology_class, "single_host_cpu_reference");
        assert_eq!(
            contract.minimum_machine_class,
            PsionicTrainMinimumMachineClass::CrossPlatformCpuCompatibleOperator
        );
    }

    #[test]
    fn resolves_actual_pretraining_lane_contract() {
        let contract = PsionicTrainLaneContract::for_lane(PSION_ACTUAL_PRETRAINING_LANE_ID)
            .expect("actual pretraining lane contract should resolve");
        assert_eq!(contract.lane_id, PSION_ACTUAL_PRETRAINING_LANE_ID);
        assert_eq!(contract.backend_family, "cuda");
        assert_eq!(
            contract.minimum_machine_class,
            PsionicTrainMinimumMachineClass::StrongCudaTrainer
        );
    }

    #[test]
    fn resolves_a1_minimal_distributed_lm_lane_contract() {
        let contract = PsionicTrainLaneContract::for_lane(PSION_A1_MINIMAL_DISTRIBUTED_LM_LANE_ID)
            .expect("A1 minimal distributed LM lane contract should resolve");
        assert_eq!(contract.lane_id, PSION_A1_MINIMAL_DISTRIBUTED_LM_LANE_ID);
        assert_eq!(
            contract.environment_ref,
            "psionic.environment.a1_minimal_distributed_lm.tiny_lm.operator@v1"
        );
        assert_eq!(contract.backend_family, "cpu");
        assert_eq!(contract.topology_class, "multi_pylon_tiny_lm_local_update");
        assert_eq!(
            contract.minimum_machine_class,
            PsionicTrainMinimumMachineClass::CrossPlatformCpuCompatibleOperator
        );
    }

    #[test]
    fn resolves_qwen_legal_adapter_sft_lane_contract() {
        let contract = PsionicTrainLaneContract::for_lane(PSION_QWEN_LEGAL_ADAPTER_SFT_LANE_ID)
            .expect("Qwen legal adapter lane contract should resolve");
        assert_eq!(contract.lane_id, PSION_QWEN_LEGAL_ADAPTER_SFT_LANE_ID);
        assert_eq!(
            contract.environment_ref,
            "psionic.environment.qwen_legal_adapter_sft.cuda.operator@v1"
        );
        assert_eq!(contract.backend_family, "cuda");
        assert_eq!(contract.topology_class, "single_node_cuda_lora_sft");
        assert_eq!(
            contract.minimum_machine_class,
            PsionicTrainMinimumMachineClass::StrongCudaTrainer
        );
    }

    #[test]
    fn rejects_unknown_lane() {
        let error = PsionicTrainLaneContract::for_lane("unknown")
            .expect_err("unknown lanes should be rejected");
        assert!(error.contains("unknown"));
    }
}
