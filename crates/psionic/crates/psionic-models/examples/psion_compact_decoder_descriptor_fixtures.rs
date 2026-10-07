use std::{error::Error, fs, path::PathBuf};

use psionic_models::{
    PsionCompactDecoderDescriptor, PsionCompactDecoderSizeAnchor,
    PsionCompactDecoderTokenizerBinding, PsionCompactDecoderTokenizerFamily,
};

fn main() -> Result<(), Box<dyn Error>> {
    let root = workspace_root()?;
    let fixtures_dir = root.join("fixtures/psion/models");
    fs::create_dir_all(&fixtures_dir)?;

    let tokenizer_binding = PsionCompactDecoderTokenizerBinding {
        tokenizer_id: String::from("psion_sentencepiece_seed"),
        tokenizer_version: String::from("v1"),
        tokenizer_family: PsionCompactDecoderTokenizerFamily::SentencePiece,
        tokenizer_digest: String::from("sha256:psion_sentencepiece_seed_tokenizer_digest_v1"),
        vocab_size: 32_768,
        special_tokens_digest: Some(String::from(
            "sha256:psion_sentencepiece_seed_added_tokens_digest_v1",
        )),
        template_digest: Some(String::from(
            "sha256:psion_sentencepiece_seed_config_digest_v1",
        )),
    };

    let pilot = PsionCompactDecoderDescriptor::new(
        PsionCompactDecoderSizeAnchor::Pilot32m,
        "v1",
        4096,
        tokenizer_binding.clone(),
    )?;
    let internal = PsionCompactDecoderDescriptor::new(
        PsionCompactDecoderSizeAnchor::Internal128m,
        "v1",
        8192,
        tokenizer_binding,
    )?;

    fs::write(
        fixtures_dir.join("psion_compact_decoder_pilot_descriptor_v1.json"),
        serde_json::to_string_pretty(&pilot)?,
    )?;
    fs::write(
        fixtures_dir.join("psion_compact_decoder_internal_descriptor_v1.json"),
        serde_json::to_string_pretty(&internal)?,
    )?;
    Ok(())
}

fn workspace_root() -> Result<PathBuf, Box<dyn Error>> {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest_dir
        .ancestors()
        .nth(2)
        .map(PathBuf::from)
        .ok_or_else(|| "failed to resolve workspace root".into())
}
