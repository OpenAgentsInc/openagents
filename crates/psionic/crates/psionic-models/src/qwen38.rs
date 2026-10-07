use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use thiserror::Error;

pub const QWEN38_ARTIFACT_FACTS_SCHEMA_VERSION: &str = "psionic.qwen38.artifact_facts.v1";
pub const QWEN38_ARTIFACT_ADMISSION_SCHEMA_VERSION: &str = "psionic.qwen38.artifact_admission.v1";
pub const QWEN38_PRODUCT_FAMILY: &str = "qwen38";
pub const QWEN38_27B_MODEL_ID: &str = "Qwen/Qwen3.8-27B";
pub const QWEN38_27B_SHORT_MODEL_ID: &str = "Qwen3.8-27B";
pub const QWEN38_27B_SERVED_MODEL_ID: &str = "qwen3.8-27b";
pub const QWEN38_27B_UPSTREAM_REVISION: &str = "1d4bf0f2ff6012fd82039f2fa52739d0dd7c60c0";
pub const QWEN38_27B_ARTIFACT_FACTS_PATH: &str =
    "fixtures/qwen38/qwen38_27b_artifact_facts_v1.json";

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Qwen38ProductIdentity {
    pub product_family: String,
    pub official_model_id: String,
    pub short_model_id: String,
    pub served_model_id: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Qwen38ArchitectureFacts {
    pub wrapper_architecture: String,
    pub root_model_type: String,
    pub decoder_architecture: String,
    pub hidden_size: usize,
    pub intermediate_size: usize,
    pub layer_count: usize,
    pub vocabulary_size: usize,
    pub full_attention_interval: usize,
    pub full_attention_head_count: usize,
    pub full_attention_kv_head_count: usize,
    pub full_attention_head_size: usize,
    pub linear_attention_qk_head_count: usize,
    pub linear_attention_value_head_count: usize,
    pub linear_attention_head_size: usize,
    pub linear_convolution_width: usize,
    pub mtp_layer_count: usize,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Qwen38ProcessorFacts {
    pub multimodal_processor: String,
    pub image_processor: String,
    pub video_processor: String,
    pub vision_layer_count: usize,
    pub vision_hidden_size: usize,
    pub vision_output_size: usize,
    pub vision_attention_head_count: usize,
    pub vision_intermediate_size: usize,
    pub spatial_patch_size: usize,
    pub temporal_patch_size: usize,
    pub spatial_merge_size: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Qwen38CapabilityStatus {
    Planned,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Qwen38ContextPosture {
    pub native_context_tokens: usize,
    pub native_runtime_status: Qwen38CapabilityStatus,
    pub extended_context_tokens: usize,
    pub extended_context_strategy: String,
    pub extended_context_factor: f32,
    pub extended_context_runtime_status: Qwen38CapabilityStatus,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Qwen38ArtifactDigests {
    pub readme_sha256: String,
    pub config_sha256: String,
    pub generation_config_sha256: String,
    pub tokenizer_sha256: String,
    pub tokenizer_config_sha256: String,
    pub chat_template_sha256: String,
    pub image_preprocessor_sha256: String,
    pub video_preprocessor_sha256: String,
    pub safetensors_index_sha256: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Qwen38ShardInventoryEntry {
    pub filename: String,
    pub file_bytes: u64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Qwen38ArtifactFacts {
    pub schema_version: String,
    pub upstream_repository_id: String,
    pub upstream_revision: String,
    pub license: String,
    pub pipeline_tag: String,
    pub library: String,
    pub weight_format: String,
    pub identity: Qwen38ProductIdentity,
    pub architecture: Qwen38ArchitectureFacts,
    pub processors: Qwen38ProcessorFacts,
    pub context: Qwen38ContextPosture,
    pub digests: Qwen38ArtifactDigests,
    pub indexed_tensor_count: usize,
    pub indexed_tensor_data_bytes: u64,
    pub shard_file_bytes: u64,
    pub shards: Vec<Qwen38ShardInventoryEntry>,
}

impl Qwen38ArtifactFacts {
    pub fn from_json_bytes(bytes: &[u8]) -> Result<Self, Qwen38ArtifactFactsError> {
        Ok(serde_json::from_slice(bytes)?)
    }

    pub fn validate_canonical_27b(&self) -> Result<(), Qwen38ArtifactFactsError> {
        let expected = canonical_qwen38_27b_artifact_facts();
        if self == &expected {
            return Ok(());
        }

        let expected = serde_json::to_value(expected)?;
        let actual = serde_json::to_value(self)?;
        let (field, expected, actual) = first_json_difference("", &expected, &actual)
            .unwrap_or_else(|| {
                (
                    String::from("$"),
                    String::from("canonical Qwen3.8-27B facts"),
                    String::from("different Qwen3.8-27B facts"),
                )
            });
        Err(Qwen38ArtifactFactsError::FieldDrift {
            field,
            expected,
            actual,
        })
    }

    pub fn canonical_sha256(&self) -> String {
        let bytes = serde_json::to_vec(self)
            .expect("serializing a Qwen3.8 artifact-facts struct cannot fail");
        hex::encode(Sha256::digest(bytes))
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Qwen38ArtifactAdmissionStatus {
    Admitted,
    Refused,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Qwen38ArtifactRefusalCode {
    UnsupportedModelVariant,
    ArtifactFactsDrift,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Qwen38ArtifactAdmissionResult {
    pub schema_version: String,
    pub status: Qwen38ArtifactAdmissionStatus,
    pub requested_model_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub normalized_model_id: Option<String>,
    pub artifact_facts_sha256: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub refusal_code: Option<Qwen38ArtifactRefusalCode>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub refusal_detail: Option<String>,
}

impl Qwen38ArtifactAdmissionResult {
    pub fn is_admitted(&self) -> bool {
        self.status == Qwen38ArtifactAdmissionStatus::Admitted
    }
}

#[derive(Debug, Error, Eq, PartialEq)]
pub enum Qwen38ModelIdError {
    #[error("unsupported Qwen3.8 model variant `{0}`")]
    UnsupportedModelVariant(String),
}

#[derive(Debug, Error)]
pub enum Qwen38ArtifactFactsError {
    #[error("failed to parse Qwen3.8 artifact facts: {0}")]
    Json(#[from] serde_json::Error),
    #[error("Qwen3.8 artifact fixture drift at `{field}`: expected {expected}, found {actual}")]
    FieldDrift {
        field: String,
        expected: String,
        actual: String,
    },
}

pub fn normalize_qwen38_27b_model_id(model: &str) -> Result<String, Qwen38ModelIdError> {
    match model {
        QWEN38_27B_MODEL_ID | QWEN38_27B_SHORT_MODEL_ID => Ok(String::from(QWEN38_27B_MODEL_ID)),
        other => Err(Qwen38ModelIdError::UnsupportedModelVariant(String::from(
            other,
        ))),
    }
}

pub fn admit_qwen38_27b_artifact(
    requested_model_id: &str,
    facts: &Qwen38ArtifactFacts,
) -> Qwen38ArtifactAdmissionResult {
    let artifact_facts_sha256 = facts.canonical_sha256();
    let normalized_model_id = match normalize_qwen38_27b_model_id(requested_model_id) {
        Ok(model_id) => model_id,
        Err(error) => {
            return Qwen38ArtifactAdmissionResult {
                schema_version: String::from(QWEN38_ARTIFACT_ADMISSION_SCHEMA_VERSION),
                status: Qwen38ArtifactAdmissionStatus::Refused,
                requested_model_id: String::from(requested_model_id),
                normalized_model_id: None,
                artifact_facts_sha256,
                refusal_code: Some(Qwen38ArtifactRefusalCode::UnsupportedModelVariant),
                refusal_detail: Some(error.to_string()),
            };
        }
    };

    if let Err(error) = facts.validate_canonical_27b() {
        return Qwen38ArtifactAdmissionResult {
            schema_version: String::from(QWEN38_ARTIFACT_ADMISSION_SCHEMA_VERSION),
            status: Qwen38ArtifactAdmissionStatus::Refused,
            requested_model_id: String::from(requested_model_id),
            normalized_model_id: Some(normalized_model_id),
            artifact_facts_sha256,
            refusal_code: Some(Qwen38ArtifactRefusalCode::ArtifactFactsDrift),
            refusal_detail: Some(error.to_string()),
        };
    }

    Qwen38ArtifactAdmissionResult {
        schema_version: String::from(QWEN38_ARTIFACT_ADMISSION_SCHEMA_VERSION),
        status: Qwen38ArtifactAdmissionStatus::Admitted,
        requested_model_id: String::from(requested_model_id),
        normalized_model_id: Some(normalized_model_id),
        artifact_facts_sha256,
        refusal_code: None,
        refusal_detail: None,
    }
}

pub fn canonical_qwen38_27b_artifact_facts() -> Qwen38ArtifactFacts {
    Qwen38ArtifactFacts {
        schema_version: String::from(QWEN38_ARTIFACT_FACTS_SCHEMA_VERSION),
        upstream_repository_id: String::from(QWEN38_27B_MODEL_ID),
        upstream_revision: String::from(QWEN38_27B_UPSTREAM_REVISION),
        license: String::from("Apache-2.0"),
        pipeline_tag: String::from("image-text-to-text"),
        library: String::from("transformers"),
        weight_format: String::from("bf16_safetensors"),
        identity: Qwen38ProductIdentity {
            product_family: String::from(QWEN38_PRODUCT_FAMILY),
            official_model_id: String::from(QWEN38_27B_MODEL_ID),
            short_model_id: String::from(QWEN38_27B_SHORT_MODEL_ID),
            served_model_id: String::from(QWEN38_27B_SERVED_MODEL_ID),
        },
        architecture: Qwen38ArchitectureFacts {
            wrapper_architecture: String::from("Qwen3_5ForConditionalGeneration"),
            root_model_type: String::from("qwen3_5"),
            decoder_architecture: String::from("qwen3_5_text"),
            hidden_size: 5_120,
            intermediate_size: 17_408,
            layer_count: 64,
            vocabulary_size: 248_320,
            full_attention_interval: 4,
            full_attention_head_count: 24,
            full_attention_kv_head_count: 4,
            full_attention_head_size: 256,
            linear_attention_qk_head_count: 16,
            linear_attention_value_head_count: 48,
            linear_attention_head_size: 128,
            linear_convolution_width: 4,
            mtp_layer_count: 1,
        },
        processors: Qwen38ProcessorFacts {
            multimodal_processor: String::from("Qwen3VLProcessor"),
            image_processor: String::from("Qwen2VLImageProcessorFast"),
            video_processor: String::from("Qwen3VLVideoProcessor"),
            vision_layer_count: 27,
            vision_hidden_size: 1_152,
            vision_output_size: 5_120,
            vision_attention_head_count: 16,
            vision_intermediate_size: 4_304,
            spatial_patch_size: 16,
            temporal_patch_size: 2,
            spatial_merge_size: 2,
        },
        context: Qwen38ContextPosture {
            native_context_tokens: 262_144,
            native_runtime_status: Qwen38CapabilityStatus::Planned,
            extended_context_tokens: 1_000_000,
            extended_context_strategy: String::from("yarn"),
            extended_context_factor: 4.0,
            extended_context_runtime_status: Qwen38CapabilityStatus::Planned,
        },
        digests: Qwen38ArtifactDigests {
            readme_sha256: String::from(
                "57e4bdb258ee1a7d2635c5174ebd4e56abe392505cdb5f8bbb356b0dc4293641",
            ),
            config_sha256: String::from(
                "191e0af232104ed8b65258cf3fb2b842e288008baca7633c11b82a1ac7203aab",
            ),
            generation_config_sha256: String::from(
                "e70c136c1b78ddc1fb0905bac8e733a4dc448d4f852a5dd75143fffc70be550e",
            ),
            tokenizer_sha256: String::from(
                "0997f410c57a1f4e53b09e4be8f4a172d90edd9564368fb0847030937229b9f3",
            ),
            tokenizer_config_sha256: String::from(
                "b11349aafa7cdc6a320767cf7ceb29ed82f7eda5d65e8e0819e76f0ce947bf27",
            ),
            chat_template_sha256: String::from(
                "c3cf9e34abf4f9e36c2d72165aa9c132d3e2a725b6c2586aaa3a8af9d7a81041",
            ),
            image_preprocessor_sha256: String::from(
                "27225450ac9c6529872ee1924fcb0962ff5634834f817040f444118116f4e516",
            ),
            video_preprocessor_sha256: String::from(
                "7768af27c1fafa9cc9011c1dc20067e03f8915e03b63504550e11d5066986d13",
            ),
            safetensors_index_sha256: String::from(
                "77042094076611b69791a610065f28b7013b8c621795fa86ddccc8bac7d1b9df",
            ),
        },
        indexed_tensor_count: 1_199,
        indexed_tensor_data_bytes: 55_562_855_904,
        shard_file_bytes: 55_563_006_776,
        shards: qwen38_27b_shard_inventory(),
    }
}

fn qwen38_27b_shard_inventory() -> Vec<Qwen38ShardInventoryEntry> {
    [
        ("model-00001-of-00018.safetensors", 3_966_730_552),
        ("model-00002-of-00018.safetensors", 3_043_080_328),
        ("model-00003-of-00018.safetensors", 2_542_796_952),
        ("model-00004-of-00018.safetensors", 3_988_973_152),
        ("model-00005-of-00018.safetensors", 2_099_339_864),
        ("model-00006-of-00018.safetensors", 3_979_553_696),
        ("model-00007-of-00018.safetensors", 2_108_759_344),
        ("model-00008-of-00018.safetensors", 3_979_553_696),
        ("model-00009-of-00018.safetensors", 2_108_759_344),
        ("model-00010-of-00018.safetensors", 3_979_553_696),
        ("model-00011-of-00018.safetensors", 2_108_759_344),
        ("model-00012-of-00018.safetensors", 3_979_553_696),
        ("model-00013-of-00018.safetensors", 2_108_759_344),
        ("model-00014-of-00018.safetensors", 3_979_553_696),
        ("model-00015-of-00018.safetensors", 2_108_759_344),
        ("model-00016-of-00018.safetensors", 3_979_564_040),
        ("model-00017-of-00018.safetensors", 2_108_759_344),
        ("model-00018-of-00018.safetensors", 3_392_197_344),
    ]
    .into_iter()
    .map(|(filename, file_bytes)| Qwen38ShardInventoryEntry {
        filename: String::from(filename),
        file_bytes,
    })
    .collect()
}

fn first_json_difference(
    path: &str,
    expected: &Value,
    actual: &Value,
) -> Option<(String, String, String)> {
    match (expected, actual) {
        (Value::Object(expected), Value::Object(actual)) => {
            for (key, expected_value) in expected {
                let field = join_json_path(path, key);
                let Some(actual_value) = actual.get(key) else {
                    return Some((field, expected_value.to_string(), String::from("<missing>")));
                };
                if let Some(difference) =
                    first_json_difference(field.as_str(), expected_value, actual_value)
                {
                    return Some(difference);
                }
            }
            actual
                .keys()
                .find(|key| !expected.contains_key(*key))
                .map(|key| {
                    (
                        join_json_path(path, key),
                        String::from("<absent>"),
                        actual[key].to_string(),
                    )
                })
        }
        (Value::Array(expected), Value::Array(actual)) => {
            for (index, expected_value) in expected.iter().enumerate() {
                let field = format!("{path}[{index}]");
                let Some(actual_value) = actual.get(index) else {
                    return Some((field, expected_value.to_string(), String::from("<missing>")));
                };
                if let Some(difference) =
                    first_json_difference(field.as_str(), expected_value, actual_value)
                {
                    return Some(difference);
                }
            }
            if actual.len() > expected.len() {
                let index = expected.len();
                return Some((
                    format!("{path}[{index}]"),
                    String::from("<absent>"),
                    actual[index].to_string(),
                ));
            }
            None
        }
        _ if expected != actual => Some((
            if path.is_empty() {
                String::from("$")
            } else {
                String::from(path)
            },
            expected.to_string(),
            actual.to_string(),
        )),
        _ => None,
    }
}

fn join_json_path(path: &str, key: &str) -> String {
    if path.is_empty() {
        String::from(key)
    } else {
        format!("{path}.{key}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const FIXTURE: &str =
        include_str!("../../../fixtures/qwen38/qwen38_27b_artifact_facts_v1.json");

    #[test]
    fn qwen38_artifact_fixture_matches_canonical_facts() {
        let facts = Qwen38ArtifactFacts::from_json_bytes(FIXTURE.as_bytes()).expect("parse facts");
        facts.validate_canonical_27b().expect("canonical facts");
        assert_eq!(facts.shards.len(), 18);
        assert_eq!(
            facts
                .shards
                .iter()
                .map(|shard| shard.file_bytes)
                .sum::<u64>(),
            facts.shard_file_bytes
        );
        assert_eq!(
            facts.architecture.wrapper_architecture,
            "Qwen3_5ForConditionalGeneration"
        );
        assert_eq!(facts.processors.multimodal_processor, "Qwen3VLProcessor");
    }

    #[test]
    fn qwen38_artifact_model_ids_normalize_and_unknown_variants_refuse() {
        let facts = canonical_qwen38_27b_artifact_facts();
        for requested in [QWEN38_27B_MODEL_ID, QWEN38_27B_SHORT_MODEL_ID] {
            assert_eq!(
                normalize_qwen38_27b_model_id(requested).expect("normalize"),
                QWEN38_27B_MODEL_ID
            );
            let result = admit_qwen38_27b_artifact(requested, &facts);
            assert!(result.is_admitted());
            assert_eq!(
                result.normalized_model_id.as_deref(),
                Some(QWEN38_27B_MODEL_ID)
            );
            assert_eq!(result.refusal_code, None);
        }

        for requested in [
            QWEN38_27B_SERVED_MODEL_ID,
            "Qwen/Qwen3.8-4B",
            "Qwen3.8-27B-AWQ",
            "Qwen/Qwen3.6-27B",
        ] {
            let result = admit_qwen38_27b_artifact(requested, &facts);
            assert_eq!(result.status, Qwen38ArtifactAdmissionStatus::Refused);
            assert_eq!(
                result.refusal_code,
                Some(Qwen38ArtifactRefusalCode::UnsupportedModelVariant)
            );
            assert_eq!(result.normalized_model_id, None);
        }
    }

    #[test]
    fn qwen38_artifact_fixture_drift_names_the_exact_field() {
        let mut value = serde_json::from_str::<Value>(FIXTURE).expect("parse value");
        value["architecture"]["hidden_size"] = Value::from(4_096);
        let facts = serde_json::from_value::<Qwen38ArtifactFacts>(value).expect("parse facts");
        let error = facts
            .validate_canonical_27b()
            .expect_err("drift must refuse");
        match error {
            Qwen38ArtifactFactsError::FieldDrift {
                field,
                expected,
                actual,
            } => {
                assert_eq!(field, "architecture.hidden_size");
                assert_eq!(expected, "5120");
                assert_eq!(actual, "4096");
            }
            other => panic!("unexpected error: {other}"),
        }
    }

    #[test]
    fn qwen38_artifact_admission_is_stable_and_digest_bound() {
        let facts = Qwen38ArtifactFacts::from_json_bytes(FIXTURE.as_bytes()).expect("parse facts");
        let compact = serde_json::to_vec(&facts).expect("serialize compact facts");
        let reparsed = Qwen38ArtifactFacts::from_json_bytes(&compact).expect("reparse facts");
        assert_eq!(facts.canonical_sha256(), reparsed.canonical_sha256());

        let first = admit_qwen38_27b_artifact(QWEN38_27B_MODEL_ID, &facts);
        let second = admit_qwen38_27b_artifact(QWEN38_27B_SHORT_MODEL_ID, &facts);
        assert_eq!(first.artifact_facts_sha256, second.artifact_facts_sha256);
        assert_eq!(
            serde_json::to_vec(&first).expect("serialize admission"),
            serde_json::to_vec(
                &serde_json::from_slice::<Qwen38ArtifactAdmissionResult>(
                    &serde_json::to_vec(&first).expect("serialize admission"),
                )
                .expect("parse admission"),
            )
            .expect("reserialize admission")
        );
    }

    #[test]
    fn qwen38_artifact_drift_produces_machine_readable_refusal() {
        let mut facts = canonical_qwen38_27b_artifact_facts();
        facts.digests.chat_template_sha256 = String::from("changed");
        let result = admit_qwen38_27b_artifact(QWEN38_27B_MODEL_ID, &facts);
        assert_eq!(result.status, Qwen38ArtifactAdmissionStatus::Refused);
        assert_eq!(
            result.refusal_code,
            Some(Qwen38ArtifactRefusalCode::ArtifactFactsDrift)
        );
        assert!(
            result
                .refusal_detail
                .as_deref()
                .expect("refusal detail")
                .contains("digests.chat_template_sha256")
        );
    }
}
