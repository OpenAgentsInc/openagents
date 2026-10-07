use serde::{Deserialize, Serialize};

/// Repo-relative path for the frozen CSM Python parity fixture.
pub const CSM_PYTHON_PARITY_FIXTURE_PATH: &str =
    "fixtures/csm/python_reference/csm_python_parity_v1.json";

const CSM_PYTHON_PARITY_FIXTURE_JSON: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../fixtures/csm/python_reference/csm_python_parity_v1.json"
));

/// Frozen CSM Python-reference parity corpus.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CsmPythonParityFixture {
    /// Fixture schema id.
    pub schema: String,
    /// Stable creation timestamp for the frozen fixture.
    pub created_at: String,
    /// Local reference repo used to extract the fixture.
    pub source_repo: String,
    /// Reference command that proved the demo path.
    pub reference_command: String,
    /// Model and artifact identities.
    pub model: CsmParityModelFacts,
    /// Prompt voice profiles captured from the reference repo.
    pub prompts: Vec<CsmParityPrompt>,
    /// Llama tokenizer and 33-lane frame examples.
    pub tokenizer_examples: Vec<CsmTokenizerExample>,
    /// Compact Mimi prompt-codebook prefixes.
    pub mimi_codebook_prefixes: Vec<CsmMimiCodebookPrefix>,
    /// Deterministic greedy generated-codebook prefix.
    pub deterministic_generation_case: CsmDeterministicGenerationCase,
    /// Explicit secret-redaction assertions for the fixture.
    pub secret_redaction: Vec<String>,
}

/// Model and artifact identities captured by the fixture.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CsmParityModelFacts {
    /// CSM model repository.
    pub csm_repo: String,
    /// Llama tokenizer repository.
    pub llama_tokenizer_repo: String,
    /// Mimi repository.
    pub mimi_repo: String,
    /// Mimi weight filename.
    pub mimi_weight: String,
    /// CSM config digest.
    pub csm_config_digest: String,
    /// CSM model weight digest.
    pub csm_model_digest: String,
    /// Llama tokenizer digest.
    pub llama_tokenizer_digest: String,
    /// Mimi weight digest.
    pub mimi_weight_digest: String,
}

/// Prompt audio facts for one CSM reference voice.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CsmParityPrompt {
    /// Stable profile id.
    pub profile_id: String,
    /// Numeric CSM speaker id.
    pub speaker: u32,
    /// Prompt transcript.
    pub text: String,
    /// Source path class.
    pub audio_path_kind: String,
    /// Prompt WAV SHA-256 digest.
    pub audio_sha256: String,
    /// Prompt WAV metadata.
    pub audio: CsmAudioMetadata,
}

/// Audio metadata captured without storing raw audio bytes.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CsmAudioMetadata {
    /// WAV sample rate in Hz.
    pub sample_rate_hz: u32,
    /// Channel count.
    pub channels: u16,
    /// Sample width in bytes.
    pub sample_width_bytes: u16,
    /// Frame count.
    pub frame_count: u64,
    /// Duration in milliseconds.
    pub duration_ms: u64,
}

/// Tokenizer and frame example for one utterance.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CsmTokenizerExample {
    /// Numeric CSM speaker id.
    pub speaker: u32,
    /// Source text before speaker-prefix encoding.
    pub text: String,
    /// Token IDs emitted by the Python reference tokenizer.
    pub encoded_token_ids: Vec<u32>,
    /// Expected `[frames, lanes]` shape for the text frame block.
    pub frame_shape: [usize; 2],
    /// First 33-lane text frame.
    pub first_frame_tokens: Vec<u32>,
    /// First 33-lane text mask.
    pub first_frame_mask: Vec<bool>,
}

/// Compact prefix of Mimi prompt codebooks.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CsmMimiCodebookPrefix {
    /// Prompt profile id.
    pub profile_id: String,
    /// Mimi sample rate.
    pub sample_rate_hz: u32,
    /// Total codebooks in the encoded prompt.
    pub codebook_count: usize,
    /// Total encoded frame count.
    pub frame_count: usize,
    /// Prefix frame count retained per codebook.
    pub prefix_frame_count: usize,
    /// Retained codebook rows.
    pub prefix_by_codebook: Vec<Vec<u32>>,
    /// Count of retained codebook rows.
    pub prefix_codebook_count: usize,
    /// Digest of the full Python-reference codebook tensor.
    pub tokens_sha256: String,
}

/// Deterministic generated-codebook prefix exported from the Python reference.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CsmDeterministicGenerationCase {
    /// Case status.
    pub status: String,
    /// Sampling policy used for deterministic export.
    pub sampling: String,
    /// Number of context segments supplied.
    pub context_segments: usize,
    /// Requested transcript text.
    pub requested_text: String,
    /// Numeric CSM speaker id.
    pub speaker: u32,
    /// Max generation duration in milliseconds.
    pub max_audio_length_ms: u64,
    /// Prompt frame count before generation.
    pub prompt_frame_count: usize,
    /// Generated prefix frame count.
    pub generated_prefix_frame_count: usize,
    /// Generated codebook frames, each with 32 codebook token ids.
    pub frames: Vec<Vec<u32>>,
    /// Digest of the compact generated-frame list.
    pub frames_sha256: String,
}

/// Loads the committed CSM Python parity fixture.
pub fn csm_python_parity_fixture() -> Result<CsmPythonParityFixture, serde_json::Error> {
    serde_json::from_str(CSM_PYTHON_PARITY_FIXTURE_JSON)
}

/// Validates the committed CSM Python parity fixture.
pub fn validate_csm_python_parity_fixture(fixture: &CsmPythonParityFixture) -> Result<(), String> {
    if fixture.schema != "psionic.csm.python_parity.v1" {
        return Err(format!("unexpected schema `{}`", fixture.schema));
    }
    if fixture.model.csm_repo != "sesame/csm-1b" {
        return Err(String::from("CSM fixture is not bound to sesame/csm-1b"));
    }
    require_sha256("csm_config_digest", &fixture.model.csm_config_digest)?;
    require_sha256("csm_model_digest", &fixture.model.csm_model_digest)?;
    require_sha256(
        "llama_tokenizer_digest",
        &fixture.model.llama_tokenizer_digest,
    )?;
    require_sha256("mimi_weight_digest", &fixture.model.mimi_weight_digest)?;

    let profile_ids = fixture
        .prompts
        .iter()
        .map(|prompt| prompt.profile_id.as_str())
        .collect::<std::collections::BTreeSet<_>>();
    for expected in ["conversational_a", "conversational_b"] {
        if !profile_ids.contains(expected) {
            return Err(format!("missing CSM prompt profile `{expected}`"));
        }
    }
    for prompt in &fixture.prompts {
        require_sha256("audio_sha256", &prompt.audio_sha256)?;
        if prompt.audio.sample_rate_hz != 44_100 {
            return Err(format!(
                "prompt `{}` expected 44.1 kHz source audio",
                prompt.profile_id
            ));
        }
        if prompt.audio.channels != 1 {
            return Err(format!("prompt `{}` is not mono", prompt.profile_id));
        }
        if prompt.audio.duration_ms == 0 || prompt.text.trim().is_empty() {
            return Err(format!("prompt `{}` is empty", prompt.profile_id));
        }
    }

    for example in &fixture.tokenizer_examples {
        if example.frame_shape[1] != 33 {
            return Err(format!(
                "tokenizer example `{}` does not use 33 lanes",
                example.text
            ));
        }
        if example.frame_shape[0] != example.encoded_token_ids.len() {
            return Err(format!(
                "tokenizer example `{}` frame/token length mismatch",
                example.text
            ));
        }
        if example.first_frame_tokens.len() != 33 || example.first_frame_mask.len() != 33 {
            return Err(format!(
                "tokenizer example `{}` first frame is not 33 lanes",
                example.text
            ));
        }
        if example.first_frame_tokens[32] != example.encoded_token_ids[0] {
            return Err(format!(
                "tokenizer example `{}` first text lane is not the first token",
                example.text
            ));
        }
        if example
            .first_frame_mask
            .iter()
            .enumerate()
            .any(|(index, value)| *value != (index == 32))
        {
            return Err(format!(
                "tokenizer example `{}` first mask should only enable text lane",
                example.text
            ));
        }
    }

    for prefix in &fixture.mimi_codebook_prefixes {
        require_sha256("tokens_sha256", &prefix.tokens_sha256)?;
        if prefix.sample_rate_hz != 24_000 {
            return Err(format!(
                "Mimi prefix `{}` expected 24 kHz tokens",
                prefix.profile_id
            ));
        }
        if prefix.codebook_count != 32 {
            return Err(format!(
                "Mimi prefix `{}` expected 32 codebooks",
                prefix.profile_id
            ));
        }
        if prefix.prefix_by_codebook.len() != prefix.prefix_codebook_count {
            return Err(format!(
                "Mimi prefix `{}` retained codebook count mismatch",
                prefix.profile_id
            ));
        }
        for row in &prefix.prefix_by_codebook {
            if row.len() != prefix.prefix_frame_count {
                return Err(format!(
                    "Mimi prefix `{}` retained frame count mismatch",
                    prefix.profile_id
                ));
            }
            if row.iter().any(|token| *token > 2050) {
                return Err(format!(
                    "Mimi prefix `{}` has token outside CSM audio vocab",
                    prefix.profile_id
                ));
            }
        }
    }

    let generation = &fixture.deterministic_generation_case;
    if generation.status != "available" || generation.sampling != "greedy_argmax_topk1" {
        return Err(String::from(
            "deterministic generation case must be available and greedy",
        ));
    }
    require_sha256("frames_sha256", &generation.frames_sha256)?;
    if generation.frames.len() != generation.generated_prefix_frame_count {
        return Err(String::from("generated frame count mismatch"));
    }
    for frame in &generation.frames {
        if frame.len() != 32 {
            return Err(String::from("generated CSM frame is not 32 codebooks"));
        }
        if frame.iter().any(|token| *token > 2050) {
            return Err(String::from(
                "generated frame token outside CSM audio vocab",
            ));
        }
    }
    if !fixture
        .secret_redaction
        .iter()
        .any(|entry| entry == "no_huggingface_token_recorded")
    {
        return Err(String::from(
            "fixture must record Hugging Face token redaction",
        ));
    }
    Ok(())
}

fn require_sha256(field: &str, digest: &str) -> Result<(), String> {
    let Some(hex) = digest.strip_prefix("sha256:") else {
        return Err(format!("{field} is not a sha256 digest"));
    };
    if hex.len() != 64 || !hex.chars().all(|character| character.is_ascii_hexdigit()) {
        return Err(format!("{field} has invalid sha256 hex"));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn csm_python_parity_fixture_is_valid() {
        let fixture = csm_python_parity_fixture().expect("fixture should parse");

        validate_csm_python_parity_fixture(&fixture).expect("fixture should validate");
    }

    #[test]
    fn csm_python_parity_fixture_freezes_expected_reference_values() {
        let fixture = csm_python_parity_fixture().expect("fixture should parse");

        assert_eq!(
            fixture.model.csm_config_digest,
            "sha256:b203c014cb5a2f7b4f98d2e945f091182aceb17fa530ce968e8c3437e01a9b70"
        );
        assert_eq!(
            fixture.model.csm_model_digest,
            "sha256:2e7721144afe38b906d4f1048671da639fe142423f4a26283606ecebe894f4bf"
        );
        assert_eq!(
            fixture.model.llama_tokenizer_digest,
            "sha256:79e3e522635f3171300913bb421464a87de6222182a0570b9b2ccba2a964b2b4"
        );
        assert_eq!(fixture.prompts.len(), 2);
        assert_eq!(
            fixture.prompts[0].audio_sha256,
            "sha256:356648c1bc6c1da7883004557e9b21a2ef7d01682d8b9d02d6dcb950b348b04f"
        );
        assert_eq!(
            fixture.tokenizer_examples[0].encoded_token_ids,
            vec![128000, 58, 15, 60, 19182, 1268, 527, 499, 3815, 30, 128001]
        );
        assert_eq!(
            fixture.mimi_codebook_prefixes[0].tokens_sha256,
            "sha256:30c8683dbb8552c199bff785e48b85cbb3f47b066038e634bf2b62e329bf3614"
        );
        assert_eq!(
            fixture.deterministic_generation_case.frames_sha256,
            "sha256:6d006f86aa37962ab4fb37477ce2cdeafcc4095641ad8a6dc1816a36f2825a9d"
        );
    }
}
