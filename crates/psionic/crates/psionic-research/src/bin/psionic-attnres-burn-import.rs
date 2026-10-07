use std::env;
use std::fs;
use std::path::PathBuf;

use psionic_models::AttnResConfig;
use psionic_research::{
    AttnResBurnImportError, AttnResBurnImportRequest, AttnResBurnPathRemap,
    AttnResBurnPathRemapMode, import_attnres_burn_artifact,
    persist_attnres_burn_import_bundle,
};

fn main() {
    if let Err(error) = run() {
        eprintln!("{error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), AttnResBurnImportError> {
    let mut args = env::args().skip(1);
    let mut source_path = None;
    let mut format = None;
    let mut config_path = None;
    let mut output_dir = None;
    let mut model_id = None;
    let mut model_revision = None;
    let mut allow_partial = false;
    let mut remaps = Vec::new();

    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--source" => source_path = args.next().map(PathBuf::from),
            "--format" => {
                format = Some(
                    args.next()
                        .ok_or_else(|| {
                            AttnResBurnImportError::InvalidRequest(String::from(
                                "missing value after --format",
                            ))
                        })?
                        .parse()?,
                );
            }
            "--config" => config_path = args.next().map(PathBuf::from),
            "--output-dir" => output_dir = args.next().map(PathBuf::from),
            "--model-id" => model_id = args.next(),
            "--model-revision" => model_revision = args.next(),
            "--allow-partial" => allow_partial = true,
            "--remap" => {
                let spec = args.next().ok_or_else(|| {
                    AttnResBurnImportError::InvalidRequest(String::from(
                        "missing value after --remap",
                    ))
                })?;
                remaps.push(parse_remap(&spec)?);
            }
            _ => {
                return Err(AttnResBurnImportError::InvalidRequest(format!(
                    "unknown argument `{arg}`"
                )));
            }
        }
    }

    let source_path = source_path.ok_or_else(|| {
        AttnResBurnImportError::InvalidRequest(String::from("missing --source <path>"))
    })?;
    let format = format.ok_or_else(|| {
        AttnResBurnImportError::InvalidRequest(String::from(
            "missing --format <default|compact|binary>",
        ))
    })?;
    let config_path = config_path.ok_or_else(|| {
        AttnResBurnImportError::InvalidRequest(String::from("missing --config <path>"))
    })?;
    let output_dir = output_dir.ok_or_else(|| {
        AttnResBurnImportError::InvalidRequest(String::from("missing --output-dir <dir>"))
    })?;

    let config_bytes = fs::read(&config_path).map_err(|error| AttnResBurnImportError::SourceRead {
        path: config_path.display().to_string(),
        detail: error.to_string(),
    })?;
    let config: AttnResConfig =
        serde_json::from_slice(&config_bytes).map_err(|error| AttnResBurnImportError::InvalidRequest(
            format!("failed to parse AttnResConfig at {}: {error}", config_path.display()),
        ))?;

    let mut request =
        AttnResBurnImportRequest::new(source_path, format, config).with_allow_partial(allow_partial);
    if let Some(model_id) = model_id {
        request.model_id = model_id;
    }
    if let Some(model_revision) = model_revision {
        request.model_revision = model_revision;
    }
    for remap in remaps {
        request = request.with_path_remap(remap);
    }

    let bundle = import_attnres_burn_artifact(&request)?;
    persist_attnres_burn_import_bundle(&bundle, &output_dir)?;
    Ok(())
}

fn parse_remap(spec: &str) -> Result<AttnResBurnPathRemap, AttnResBurnImportError> {
    let (mode, rest) = spec.split_once(':').ok_or_else(|| {
        AttnResBurnImportError::InvalidRequest(format!(
            "invalid --remap `{spec}`; expected <exact|prefix|suffix|replace>:<from>=<to>"
        ))
    })?;
    let (from, to) = rest.split_once('=').ok_or_else(|| {
        AttnResBurnImportError::InvalidRequest(format!(
            "invalid --remap `{spec}`; expected <exact|prefix|suffix|replace>:<from>=<to>"
        ))
    })?;
    let mode = match mode {
        "exact" => AttnResBurnPathRemapMode::Exact,
        "prefix" => AttnResBurnPathRemapMode::Prefix,
        "suffix" => AttnResBurnPathRemapMode::Suffix,
        "replace" => AttnResBurnPathRemapMode::Replace,
        other => {
            return Err(AttnResBurnImportError::InvalidRequest(format!(
                "unknown remap mode `{other}`"
            )));
        }
    };
    Ok(AttnResBurnPathRemap::new(mode, from, to))
}
