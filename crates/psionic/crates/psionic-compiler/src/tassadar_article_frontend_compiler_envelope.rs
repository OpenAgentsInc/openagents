use std::{
    fs,
    path::{Path, PathBuf},
};

use psionic_runtime::{
    TASSADAR_CANONICAL_C_SOURCE_REF, TASSADAR_HEAP_SUM_RUST_SOURCE_REF,
    TASSADAR_HUNGARIAN_10X10_RUST_SOURCE_REF, TASSADAR_LONG_LOOP_RUST_SOURCE_REF,
    TASSADAR_MEMORY_LOOKUP_RUST_SOURCE_REF, TASSADAR_MICRO_WASM_RUST_SOURCE_REF,
    TASSADAR_MULTI_EXPORT_RUST_SOURCE_REF, TASSADAR_PARAM_ABI_RUST_SOURCE_REF,
    TASSADAR_SUDOKU_9X9_RUST_SOURCE_REF, TassadarProgramSourceKind,
    TassadarRustToWasmCompileConfig,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

pub const TASSADAR_ARTICLE_FRONTEND_COMPILER_ENVELOPE_MANIFEST_REF: &str =
    "fixtures/tassadar/sources/tassadar_article_frontend_compiler_envelope_v1.json";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarArticleFrontendFamily {
    RustSource,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarArticleFrontendLibrarySurface {
    CoreOnlyNoStd,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarArticleFrontendAbiSurfaceId {
    NullaryI32Return,
    ScalarI32ParamsSingleI32Return,
    PointerLengthI32HeapInputSingleI32Return,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarArticleFrontendEnvelopeRefusalKind {
    OutsideDeclaredFrontendFamily,
    OutsideDeclaredLanguageVersion,
    OutsideDeclaredLibrarySurface,
    OutsideDeclaredAbiSurface,
    HostOrSyscallSurfaceDisallowed,
    UbDependentSourceDisallowed,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarArticleFrontendCompilerToolchainPolicy {
    pub compiler_family: String,
    pub target: String,
    pub language_version: String,
    pub crate_type: String,
    pub optimization_level: String,
    pub panic_strategy: String,
    pub require_no_std: bool,
    pub require_no_main: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarArticleFrontendAbiSurface {
    pub abi_surface_id: TassadarArticleFrontendAbiSurfaceId,
    pub signature_shape: String,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarArticleFrontendAdmittedSourceRow {
    pub case_id: String,
    pub workload_family_id: String,
    pub source_kind: TassadarProgramSourceKind,
    pub source_ref: String,
    pub compile_surface_id: String,
    pub compile_config_digest: String,
    pub compile_pipeline_features: Vec<String>,
    pub abi_surface_id: TassadarArticleFrontendAbiSurfaceId,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarArticleFrontendDisallowedRow {
    pub row_id: String,
    pub refusal_kind: TassadarArticleFrontendEnvelopeRefusalKind,
    pub title: String,
    pub representative_source_ref: String,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarArticleFrontendCompilerEnvelopeManifest {
    pub schema_version: u16,
    pub manifest_id: String,
    pub manifest_ref: String,
    pub route_anchor: String,
    pub frontend_family: TassadarArticleFrontendFamily,
    pub toolchain_policy: TassadarArticleFrontendCompilerToolchainPolicy,
    pub allowed_library_surfaces: Vec<TassadarArticleFrontendLibrarySurface>,
    pub allowed_abi_surfaces: Vec<TassadarArticleFrontendAbiSurface>,
    pub admitted_source_rows: Vec<TassadarArticleFrontendAdmittedSourceRow>,
    pub disallowed_rows: Vec<TassadarArticleFrontendDisallowedRow>,
    pub current_truth_boundary: String,
    pub non_implications: Vec<String>,
    pub claim_boundary: String,
    pub manifest_digest: String,
}

impl TassadarArticleFrontendCompilerEnvelopeManifest {
    fn new() -> Self {
        let mut manifest = Self {
            schema_version: 1,
            manifest_id: String::from("tassadar.article_frontend_compiler_envelope.v1"),
            manifest_ref: String::from(TASSADAR_ARTICLE_FRONTEND_COMPILER_ENVELOPE_MANIFEST_REF),
            route_anchor: String::from("psionic_transformer.article_route"),
            frontend_family: TassadarArticleFrontendFamily::RustSource,
            toolchain_policy: TassadarArticleFrontendCompilerToolchainPolicy {
                compiler_family: String::from("rustc"),
                target: String::from("wasm32-unknown-unknown"),
                language_version: String::from("edition_2024"),
                crate_type: String::from("cdylib"),
                optimization_level: String::from("3"),
                panic_strategy: String::from("abort"),
                require_no_std: true,
                require_no_main: true,
            },
            allowed_library_surfaces: vec![TassadarArticleFrontendLibrarySurface::CoreOnlyNoStd],
            allowed_abi_surfaces: vec![
                TassadarArticleFrontendAbiSurface {
                    abi_surface_id: TassadarArticleFrontendAbiSurfaceId::NullaryI32Return,
                    signature_shape: String::from("extern_c fn() -> i32"),
                    detail: String::from(
                        "nullary i32-return entrypoints cover the bounded article demo and long-horizon kernels",
                    ),
                },
                TassadarArticleFrontendAbiSurface {
                    abi_surface_id:
                        TassadarArticleFrontendAbiSurfaceId::ScalarI32ParamsSingleI32Return,
                    signature_shape: String::from("extern_c fn(i32...) -> i32"),
                    detail: String::from(
                        "direct scalar i32 arguments with one i32 return stay inside the declared article frontend/compiler envelope",
                    ),
                },
                TassadarArticleFrontendAbiSurface {
                    abi_surface_id: TassadarArticleFrontendAbiSurfaceId::PointerLengthI32HeapInputSingleI32Return,
                    signature_shape: String::from(
                        "extern_c fn(*const/*mut i32, i32...) -> i32",
                    ),
                    detail: String::from(
                        "bounded pointer-length heap-input entrypoints stay admitted only for i32-backed buffers inside the already-declared article ABI lane",
                    ),
                },
            ],
            admitted_source_rows: admitted_source_rows(),
            disallowed_rows: disallowed_rows(),
            current_truth_boundary: String::from(
                "the declared article frontend/compiler envelope is one Rust-source-only `rustc` -> `wasm32-unknown-unknown` route over `#![no_std]` + `#![no_main]` sources, `core`-only source surface, `cdylib` output, `panic=abort`, and the already-bounded i32-oriented article ABI rows; C/C++, std/alloc surfaces, host imports, syscall-dependent rows, UB-dependent rows, and wider numeric or multi-result ABI rows stay outside this declared public envelope",
            ),
            non_implications: vec![
                String::from("not arbitrary C or C++ article ingress"),
                String::from("not generic Rust frontend closure"),
                String::from("not Cargo dependency-graph closure"),
                String::from("not host-import, syscall, or OS service closure"),
                String::from("not a widening of the current bounded article ABI or interpreter breadth"),
            ],
            claim_boundary: String::from(
                "this manifest declares only the frontend/compiler envelope that current public article-closure claims are allowed to rely on. It does not by itself expand the corpus, prove article-demo parity, or turn final article-equivalence green.",
            ),
            manifest_digest: String::new(),
        };
        manifest.manifest_digest = stable_digest(
            b"psionic_tassadar_article_frontend_compiler_envelope|",
            &manifest,
        );
        manifest
    }
}

#[derive(Debug, Error)]
pub enum TassadarArticleFrontendCompilerEnvelopeManifestError {
    #[error("failed to create `{path}`: {error}")]
    CreateDir { path: String, error: std::io::Error },
    #[error("failed to write `{path}`: {error}")]
    Write { path: String, error: std::io::Error },
    #[error("failed to read `{path}`: {error}")]
    Read { path: String, error: std::io::Error },
    #[error("failed to decode `{path}`: {error}")]
    Decode {
        path: String,
        error: serde_json::Error,
    },
    #[error(transparent)]
    Json(#[from] serde_json::Error),
}

pub fn build_tassadar_article_frontend_compiler_envelope_manifest()
-> TassadarArticleFrontendCompilerEnvelopeManifest {
    TassadarArticleFrontendCompilerEnvelopeManifest::new()
}

pub fn tassadar_article_frontend_compiler_envelope_manifest_path() -> PathBuf {
    repo_root().join(TASSADAR_ARTICLE_FRONTEND_COMPILER_ENVELOPE_MANIFEST_REF)
}

pub fn write_tassadar_article_frontend_compiler_envelope_manifest(
    output_path: impl AsRef<Path>,
) -> Result<
    TassadarArticleFrontendCompilerEnvelopeManifest,
    TassadarArticleFrontendCompilerEnvelopeManifestError,
> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            TassadarArticleFrontendCompilerEnvelopeManifestError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let manifest = build_tassadar_article_frontend_compiler_envelope_manifest();
    let json = serde_json::to_string_pretty(&manifest)?;
    fs::write(output_path, format!("{json}\n")).map_err(|error| {
        TassadarArticleFrontendCompilerEnvelopeManifestError::Write {
            path: output_path.display().to_string(),
            error,
        }
    })?;
    Ok(manifest)
}

fn admitted_source_rows() -> Vec<TassadarArticleFrontendAdmittedSourceRow> {
    vec![
        admitted_row(
            "multi_export_exact",
            "rust.multi_export_kernel",
            TASSADAR_MULTI_EXPORT_RUST_SOURCE_REF,
            "rustc.wasm32_unknown_unknown.article_envelope.v1",
            TassadarRustToWasmCompileConfig::canonical_multi_export_kernel(),
            TassadarArticleFrontendAbiSurfaceId::ScalarI32ParamsSingleI32Return,
        ),
        admitted_row(
            "memory_lookup_exact",
            "rust.memory_lookup_kernel",
            TASSADAR_MEMORY_LOOKUP_RUST_SOURCE_REF,
            "rustc.wasm32_unknown_unknown.article_envelope.v1",
            TassadarRustToWasmCompileConfig::canonical_memory_lookup_kernel(),
            TassadarArticleFrontendAbiSurfaceId::NullaryI32Return,
        ),
        admitted_row(
            "param_abi_fixture",
            "rust.param_abi_kernel",
            TASSADAR_PARAM_ABI_RUST_SOURCE_REF,
            "rustc.wasm32_unknown_unknown.article_envelope.v1",
            TassadarRustToWasmCompileConfig::canonical_param_abi_kernel(),
            TassadarArticleFrontendAbiSurfaceId::ScalarI32ParamsSingleI32Return,
        ),
        admitted_row(
            "micro_wasm_article",
            "rust.micro_wasm_article",
            TASSADAR_MICRO_WASM_RUST_SOURCE_REF,
            "rustc.wasm32_unknown_unknown.article_envelope.v1",
            TassadarRustToWasmCompileConfig::canonical_micro_wasm_kernel(),
            TassadarArticleFrontendAbiSurfaceId::PointerLengthI32HeapInputSingleI32Return,
        ),
        admitted_row(
            "heap_sum_article",
            "rust.heap_sum_article",
            TASSADAR_HEAP_SUM_RUST_SOURCE_REF,
            "rustc.wasm32_unknown_unknown.article_envelope.v1",
            TassadarRustToWasmCompileConfig::canonical_heap_sum_kernel(),
            TassadarArticleFrontendAbiSurfaceId::PointerLengthI32HeapInputSingleI32Return,
        ),
        admitted_row(
            "long_loop_article",
            "rust.long_loop_article",
            TASSADAR_LONG_LOOP_RUST_SOURCE_REF,
            "rustc.wasm32_unknown_unknown.article_envelope.v1",
            TassadarRustToWasmCompileConfig::canonical_long_loop_kernel(),
            TassadarArticleFrontendAbiSurfaceId::NullaryI32Return,
        ),
        admitted_row(
            "hungarian_10x10_article",
            "rust.hungarian_10x10_article",
            TASSADAR_HUNGARIAN_10X10_RUST_SOURCE_REF,
            "rustc.wasm32_unknown_unknown.article_envelope.v1",
            TassadarRustToWasmCompileConfig::canonical_hungarian_10x10_article(),
            TassadarArticleFrontendAbiSurfaceId::NullaryI32Return,
        ),
        admitted_row(
            "sudoku_9x9_article",
            "rust.sudoku_9x9_article",
            TASSADAR_SUDOKU_9X9_RUST_SOURCE_REF,
            "rustc.wasm32_unknown_unknown.article_envelope.v1",
            TassadarRustToWasmCompileConfig::canonical_sudoku_9x9_article(),
            TassadarArticleFrontendAbiSurfaceId::NullaryI32Return,
        ),
    ]
}

fn admitted_row(
    case_id: &str,
    workload_family_id: &str,
    source_ref: &str,
    compile_surface_id: &str,
    compile_config: TassadarRustToWasmCompileConfig,
    abi_surface_id: TassadarArticleFrontendAbiSurfaceId,
) -> TassadarArticleFrontendAdmittedSourceRow {
    TassadarArticleFrontendAdmittedSourceRow {
        case_id: String::from(case_id),
        workload_family_id: String::from(workload_family_id),
        source_kind: TassadarProgramSourceKind::RustSource,
        source_ref: String::from(source_ref),
        compile_surface_id: String::from(compile_surface_id),
        compile_config_digest: compile_config.stable_digest(),
        compile_pipeline_features: compile_config.pipeline_features(),
        abi_surface_id,
    }
}

fn disallowed_rows() -> Vec<TassadarArticleFrontendDisallowedRow> {
    vec![
        TassadarArticleFrontendDisallowedRow {
            row_id: String::from("c_source_outside_declared_frontend"),
            refusal_kind: TassadarArticleFrontendEnvelopeRefusalKind::OutsideDeclaredFrontendFamily,
            title: String::from("C/C++ source ingress stays outside the declared article envelope"),
            representative_source_ref: String::from(TASSADAR_CANONICAL_C_SOURCE_REF),
            detail: String::from(
                "the public article frontend/compiler envelope is Rust-source-only for now; the historical C-source compile receipt remains visible but out of envelope",
            ),
        },
        TassadarArticleFrontendDisallowedRow {
            row_id: String::from("std_surface_outside_declared_library"),
            refusal_kind: TassadarArticleFrontendEnvelopeRefusalKind::OutsideDeclaredLibrarySurface,
            title: String::from(
                "std or alloc-backed Rust sources stay outside the declared article envelope",
            ),
            representative_source_ref: String::from(
                "fixtures/tassadar/sources/tassadar_article_std_surface_refusal.rs",
            ),
            detail: String::from(
                "the declared envelope admits `core`-only `#![no_std]` sources rather than a broader std/alloc source surface",
            ),
        },
        TassadarArticleFrontendDisallowedRow {
            row_id: String::from("wider_numeric_abi_outside_declared_surface"),
            refusal_kind: TassadarArticleFrontendEnvelopeRefusalKind::OutsideDeclaredAbiSurface,
            title: String::from(
                "wider numeric or multi-result ABI rows stay outside the declared article envelope",
            ),
            representative_source_ref: String::from(
                "fixtures/tassadar/sources/tassadar_wider_numeric_kernel.rs",
            ),
            detail: String::from(
                "the declared frontend/compiler envelope stays aligned with the already-bounded i32 article ABI rather than widening to i64 or multi-result rows",
            ),
        },
        TassadarArticleFrontendDisallowedRow {
            row_id: String::from("host_imports_and_syscalls_outside_declared_surface"),
            refusal_kind:
                TassadarArticleFrontendEnvelopeRefusalKind::HostOrSyscallSurfaceDisallowed,
            title: String::from(
                "host imports and syscall-dependent rows stay outside the declared article envelope",
            ),
            representative_source_ref: String::from(
                "fixtures/tassadar/sources/tassadar_article_host_import_refusal.rs",
            ),
            detail: String::from(
                "the declared envelope admits direct pure exports only; host calls, imported callbacks, and syscall-backed behavior remain out of scope",
            ),
        },
        TassadarArticleFrontendDisallowedRow {
            row_id: String::from("ub_dependent_rows_outside_declared_surface"),
            refusal_kind: TassadarArticleFrontendEnvelopeRefusalKind::UbDependentSourceDisallowed,
            title: String::from(
                "UB-dependent source rows stay outside the declared article envelope",
            ),
            representative_source_ref: String::from(
                "fixtures/tassadar/sources/tassadar_article_ub_refusal.rs",
            ),
            detail: String::from(
                "unsafe code is not rejected categorically, but rows whose semantics depend on undefined behavior stay outside the declared envelope",
            ),
        },
        TassadarArticleFrontendDisallowedRow {
            row_id: String::from("older_language_revision_outside_declared_surface"),
            refusal_kind:
                TassadarArticleFrontendEnvelopeRefusalKind::OutsideDeclaredLanguageVersion,
            title: String::from(
                "language revisions outside edition 2024 stay outside the declared article envelope",
            ),
            representative_source_ref: String::from(TASSADAR_PARAM_ABI_RUST_SOURCE_REF),
            detail: String::from(
                "public article-closure claims bind to the explicit current edition and toolchain policy instead of implying generic cross-edition frontend closure",
            ),
        },
    ]
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("psionic-compiler should live under <repo>/crates/psionic-compiler")
        .to_path_buf()
}

fn stable_digest<T: Serialize>(prefix: &[u8], value: &T) -> String {
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    hasher.update(serde_json::to_vec(value).unwrap_or_default());
    hex::encode(hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::{
        TASSADAR_ARTICLE_FRONTEND_COMPILER_ENVELOPE_MANIFEST_REF,
        TassadarArticleFrontendAbiSurfaceId, TassadarArticleFrontendCompilerEnvelopeManifest,
        TassadarArticleFrontendEnvelopeRefusalKind,
        build_tassadar_article_frontend_compiler_envelope_manifest,
        tassadar_article_frontend_compiler_envelope_manifest_path,
        write_tassadar_article_frontend_compiler_envelope_manifest,
    };

    #[test]
    fn article_frontend_compiler_envelope_manifest_is_machine_legible() {
        let manifest = build_tassadar_article_frontend_compiler_envelope_manifest();

        assert_eq!(manifest.schema_version, 1);
        assert_eq!(manifest.admitted_source_rows.len(), 8);
        assert!(manifest.allowed_abi_surfaces.iter().any(|row| {
            row.abi_surface_id
                == TassadarArticleFrontendAbiSurfaceId::PointerLengthI32HeapInputSingleI32Return
        }));
        assert!(manifest.disallowed_rows.iter().any(|row| {
            row.refusal_kind
                == TassadarArticleFrontendEnvelopeRefusalKind::OutsideDeclaredFrontendFamily
        }));
        assert!(manifest.disallowed_rows.iter().any(|row| {
            row.refusal_kind
                == TassadarArticleFrontendEnvelopeRefusalKind::HostOrSyscallSurfaceDisallowed
        }));
        assert!(manifest.disallowed_rows.iter().any(|row| {
            row.refusal_kind
                == TassadarArticleFrontendEnvelopeRefusalKind::UbDependentSourceDisallowed
        }));
    }

    #[test]
    fn article_frontend_compiler_envelope_manifest_matches_committed_truth()
    -> Result<(), Box<dyn std::error::Error>> {
        let generated = build_tassadar_article_frontend_compiler_envelope_manifest();
        let committed: TassadarArticleFrontendCompilerEnvelopeManifest = serde_json::from_slice(
            &std::fs::read(tassadar_article_frontend_compiler_envelope_manifest_path())?,
        )?;
        assert_eq!(generated, committed);
        Ok(())
    }

    #[test]
    fn write_article_frontend_compiler_envelope_manifest_persists_current_truth()
    -> Result<(), Box<dyn std::error::Error>> {
        let directory = tempfile::tempdir()?;
        let output_path = directory
            .path()
            .join("tassadar_article_frontend_compiler_envelope_v1.json");
        let written = write_tassadar_article_frontend_compiler_envelope_manifest(&output_path)?;
        let persisted: TassadarArticleFrontendCompilerEnvelopeManifest =
            serde_json::from_slice(&std::fs::read(&output_path)?)?;
        assert_eq!(written, persisted);
        assert_eq!(
            tassadar_article_frontend_compiler_envelope_manifest_path()
                .file_name()
                .and_then(|name| name.to_str()),
            Some("tassadar_article_frontend_compiler_envelope_v1.json")
        );
        Ok(())
    }

    #[test]
    fn manifest_ref_is_stable() {
        assert_eq!(
            TASSADAR_ARTICLE_FRONTEND_COMPILER_ENVELOPE_MANIFEST_REF,
            "fixtures/tassadar/sources/tassadar_article_frontend_compiler_envelope_v1.json"
        );
    }
}
