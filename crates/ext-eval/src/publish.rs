//! Publishing a result: the suite as a NIP-EXT release, then the `3189`.
//!
//! `openagents ext eval publish` (on the phone, **Add to the Gym**) does
//! two things, each only once:
//!
//! 1. **The suite.** A NIP-EXT release (`3184`) of a package holding one
//!    `eval-suite` component, signed by the suite's author. Its manifest
//!    lists `suite.json`, `cases.json`, and every case file under
//!    `evals/<case>/`; the files go to the relay's Blossom server first.
//! 2. **The result.** The report, with its suite ArtifactRef now naming
//!    that release, as a NIP-EVAL `3189` with the `oa:ext-eval:v1` marker
//!    and the report inline, signed by the evaluator.
//!
//! This module builds the bytes and the unsigned events and checks them
//! with `nostr::eval_ext`'s parsers; the caller signs, uploads, and sends.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use nostr::kb::Unsigned;
use serde_json::{Value, json};

use crate::artifact::{ArtifactRef, JSON, MARKDOWN, TOML, json_bytes};
use crate::case::LoadOptions;
use crate::discover::Suite;
use crate::report::SUITE_SCHEMA;

/// The NIP-EXT manifest schema.
pub const MANIFEST_SCHEMA: &str = "openagents.package.v1";
/// The suite component's slug in a suite package.
pub const SUITE_COMPONENT: &str = "suite";
/// The license a suite package states: the runner asserts none.
pub const LICENSE: &str = "NOASSERTION";

/// What went wrong publishing.
#[derive(Debug, thiserror::Error)]
pub enum PublishError {
    /// A results file is missing or unreadable.
    #[error("{0}: {1}")]
    Io(String, String),
    /// A results file doesn't agree with the report.
    #[error("{0}")]
    Mismatch(String),
    /// A shared contract refused the record.
    #[error("{0}")]
    Contract(String),
}

fn io(path: &Path) -> impl Fn(std::io::Error) -> PublishError + '_ {
    move |error| PublishError::Io(path.display().to_string(), error.to_string())
}

fn contract(error: nostr::contracts::ContractError) -> PublishError {
    PublishError::Contract(error.to_string())
}

/// A results directory, read back for publishing.
#[derive(Debug)]
pub struct Results {
    /// The directory.
    pub dir: PathBuf,
    /// `report.json`'s exact bytes.
    pub report: Vec<u8>,
    /// The report, parsed.
    pub value: Value,
    /// `artifacts/suite.json`'s exact bytes.
    pub suite: Vec<u8>,
    /// `artifacts/cases.json`'s exact bytes.
    pub cases: Vec<u8>,
    /// The suite read back from `suite/`.
    pub loaded: Suite,
}

impl Results {
    /// Reads the results directory holding `report` (a `report.json` path
    /// or its directory), and checks that its suite files are the ones the
    /// report digests.
    ///
    /// # Errors
    ///
    /// Returns [`PublishError`] for a missing file, a report that isn't an
    /// extension evaluation report, or suite files that don't match it.
    pub fn open(report: &Path) -> Result<Self, PublishError> {
        let dir = if report.is_dir() {
            report.to_path_buf()
        } else {
            report
                .parent()
                .map_or_else(|| PathBuf::from("."), Path::to_path_buf)
        };
        let report_path = dir.join("report.json");
        let bytes = std::fs::read(&report_path).map_err(io(&report_path))?;
        nostr::eval_ext::parse_report(&bytes).map_err(contract)?;
        let value: Value = serde_json::from_slice(&bytes)
            .map_err(|error| PublishError::Mismatch(error.to_string()))?;
        let suite_path = dir.join("artifacts/suite.json");
        let suite = std::fs::read(&suite_path).map_err(io(&suite_path))?;
        let cases_path = dir.join("artifacts/cases.json");
        let cases = std::fs::read(&cases_path).map_err(io(&cases_path))?;
        let suite_ref: nostr::contracts::ArtifactRef =
            nostr::contracts::parse_artifact(&value["suite"]).map_err(contract)?;
        nostr::contracts::check_artifact_bytes(&suite_ref, &suite).map_err(contract)?;
        let parsed = nostr::eval_ext::parse_suite(&suite).map_err(contract)?;
        nostr::contracts::check_artifact_bytes(&parsed.cases, &cases).map_err(contract)?;
        let loaded = Suite::load(&dir.join("suite"), LoadOptions::default())
            .map_err(|error| PublishError::Mismatch(error.to_string()))?;
        Ok(Self {
            dir,
            report: bytes,
            value,
            suite,
            cases,
            loaded,
        })
    }

    /// A suite that hasn't run, as [`suite_release`] reads it: its cases
    /// and the `suite.json` and `cases.json` bytes
    /// [`crate::evaluate::suite_documents`] writes. Releasing it first is
    /// how a runner cites a suite's release before its first run.
    #[must_use]
    pub fn of_suite(loaded: Suite, suite: Vec<u8>, cases: Vec<u8>) -> Self {
        Self {
            dir: PathBuf::new(),
            report: Vec::new(),
            value: Value::Null,
            suite,
            cases,
            loaded,
        }
    }

    /// The suite's qualified ID, `<author>:<package>/<component>`.
    #[must_use]
    pub fn suite_id(&self) -> String {
        serde_json::from_slice::<Value>(&self.suite)
            .ok()
            .and_then(|value| value.get("id").and_then(Value::as_str).map(str::to_string))
            .unwrap_or_default()
    }

    /// The suite author's public key.
    #[must_use]
    pub fn author(&self) -> String {
        self.suite_id()
            .split_once(':')
            .map(|(author, _)| author.to_string())
            .unwrap_or_default()
    }

    /// The evaluator's public key.
    #[must_use]
    pub fn evaluator(&self) -> String {
        self.value["evaluator"]
            .as_str()
            .unwrap_or_default()
            .to_string()
    }

    /// Whether the report already names its suite's release.
    #[must_use]
    pub fn suite_release(&self) -> Option<Value> {
        self.value["suite"].get("event").cloned()
    }
}

/// One file a suite package lists.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Listed {
    /// Its path in the package.
    pub path: String,
    /// Its exact bytes.
    pub bytes: Vec<u8>,
    /// Its media type.
    pub media_type: String,
}

/// A suite, ready to release.
#[derive(Clone, Debug)]
pub struct SuiteRelease {
    /// `<author>:<package>`.
    pub package: String,
    /// The version label: `suite-` and the first 12 hex digits of the
    /// suite's digest, so different suites never share a label.
    pub version: String,
    /// The manifest's exact bytes.
    pub manifest: Vec<u8>,
    /// Every listed file.
    pub files: Vec<Listed>,
}

impl SuiteRelease {
    /// The unsigned `3184` release record. The signer must be the suite's
    /// author.
    #[must_use]
    pub fn event(&self) -> Unsigned {
        let body = json!({
            "v": 1,
            "requires": [],
            "type": "release",
            "package": self.package,
            "version": self.version,
            "manifest": ArtifactRef::of(&self.manifest, JSON, Some(MANIFEST_SCHEMA)).value(),
        });
        Unsigned {
            kind: nostr::kinds::EXT_RELEASE,
            tags: vec![nostr::domain::Tag::new(vec![
                "t".into(),
                "oa:ext:release:v1".into(),
            ])],
            content: body.to_string(),
        }
    }

    /// Every blob the release needs on the Blossom server: the manifest
    /// and each listed file, once each.
    #[must_use]
    pub fn blobs(&self) -> Vec<(Vec<u8>, String)> {
        let mut seen = std::collections::BTreeSet::new();
        let mut out = Vec::new();
        for (bytes, media) in std::iter::once((&self.manifest, JSON)).chain(
            self.files
                .iter()
                .map(|file| (&file.bytes, file.media_type.as_str())),
        ) {
            if seen.insert(nostr::contracts::digest_bytes(bytes)) {
                out.push((bytes.clone(), media.to_string()));
            }
        }
        out
    }
}

fn media_for(path: &str) -> &'static str {
    if path.ends_with(".md") {
        MARKDOWN
    } else if path.ends_with(".toml") {
        TOML
    } else if path.ends_with(".json") {
        JSON
    } else {
        "application/octet-stream"
    }
}

/// The suite package for a results directory: `suite.json`, `cases.json`,
/// and every case file under `evals/<case>/`, checked with
/// `nostr::eval_ext::check_suite_package`.
///
/// # Errors
///
/// Returns [`PublishError`] when the package doesn't check.
pub fn suite_release(results: &Results) -> Result<SuiteRelease, PublishError> {
    let id = results.suite_id();
    let (author, rest) = id
        .split_once(':')
        .ok_or_else(|| PublishError::Mismatch(format!("the suite ID {id} is not qualified")))?;
    let (package, component) = rest
        .split_once('/')
        .ok_or_else(|| PublishError::Mismatch(format!("the suite ID {id} names no component")))?;
    let mut files = vec![
        Listed {
            path: "suite.json".into(),
            bytes: results.suite.clone(),
            media_type: JSON.into(),
        },
        Listed {
            path: "cases.json".into(),
            bytes: results.cases.clone(),
            media_type: JSON.into(),
        },
    ];
    for case in &results.loaded.cases {
        let base = format!("evals/{}", case.name);
        files.push(Listed {
            path: format!("{base}/prompt.md"),
            bytes: case.files.prompt.clone(),
            media_type: MARKDOWN.into(),
        });
        if let Some(bytes) = &case.files.case_toml {
            files.push(Listed {
                path: format!("{base}/case.toml"),
                bytes: bytes.clone(),
                media_type: TOML.into(),
            });
        }
        for (name, bytes) in &case.files.graders {
            files.push(Listed {
                path: format!("{base}/graders/{name}"),
                bytes: bytes.clone(),
                media_type: MARKDOWN.into(),
            });
        }
        for (path, bytes) in &case.files.fixtures {
            files.push(Listed {
                path: format!("{base}/fixtures/{path}"),
                bytes: bytes.clone(),
                media_type: media_for(path).into(),
            });
        }
    }
    let listed: Vec<Value> = files
        .iter()
        .map(|file| {
            json!({
                "path": file.path,
                "digest": nostr::contracts::digest_bytes(&file.bytes),
                "size": file.bytes.len(),
                "media_type": file.media_type,
            })
        })
        .collect();
    let suite_digest = nostr::contracts::digest_bytes(&results.suite);
    let version = format!(
        "suite-{}",
        &suite_digest.trim_start_matches("sha256:")[..12]
    );
    let manifest = json!({
        "v": MANIFEST_SCHEMA,
        "requires": [],
        "package": format!("{author}:{package}"),
        "version": version,
        "license": LICENSE,
        "provenance": {"source": "local", "receipts": [], "unknowns": []},
        "components": [{
            "slug": component,
            "kind": nostr::eval_ext::COMPONENT_KIND,
            "definition": ArtifactRef::of(&results.suite, JSON, Some(SUITE_SCHEMA)).value(),
        }],
        "files": listed,
        "dependencies": [],
    });
    nostr::eval_ext::check_suite_package(&manifest, &results.suite, &results.cases)
        .map_err(contract)?;
    Ok(SuiteRelease {
        package: format!("{author}:{package}"),
        version,
        manifest: json_bytes(&manifest),
        files,
    })
}

/// The report with its suite ArtifactRef naming `release` (a `{id, pubkey,
/// kind}` EventRef), as exact bytes, checked under the profile.
///
/// # Errors
///
/// Returns [`PublishError::Contract`] when the result is not a valid
/// report.
pub fn with_suite_release(report: &Value, release: &Value) -> Result<Vec<u8>, PublishError> {
    nostr::eval_ext::event_pointer(release, nostr::kinds::EXT_RELEASE, "suite release")
        .map_err(contract)?;
    let mut report = report.clone();
    report["suite"]["event"] = release.clone();
    let bytes = json_bytes(&report);
    nostr::eval_ext::parse_report(&bytes).map_err(contract)?;
    Ok(bytes)
}

/// The unsigned `3189` for exact report bytes. `checks` names the
/// publication a check checks. The signer must be the report's evaluator.
///
/// # Errors
///
/// Returns [`PublishError::Contract`] when the report can't be published:
/// over 64 KiB, no suite release, or not valid under the profile.
pub fn result_event(report: &[u8], checks: Option<&str>) -> Result<Unsigned, PublishError> {
    let text = std::str::from_utf8(report)
        .map_err(|_| PublishError::Mismatch("the report is not UTF-8".into()))?;
    nostr::eval_ext::publication(text, checks).map_err(contract)
}

/// What a publish left, recorded in `published.json` so a second publish
/// reuses it.
#[derive(Clone, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Record {
    /// The suite release event ID.
    pub suite_release: Option<String>,
    /// The result publication event ID.
    pub result: Option<String>,
    /// The relay it went to.
    pub relay: Option<String>,
    /// Blobs uploaded, by digest.
    pub blobs: BTreeMap<String, String>,
}

impl Record {
    /// Reads `published.json` from `dir`, or an empty record.
    #[must_use]
    pub fn read(dir: &Path) -> Self {
        std::fs::read(dir.join("published.json"))
            .ok()
            .and_then(|bytes| serde_json::from_slice(&bytes).ok())
            .unwrap_or_default()
    }

    /// Writes `published.json` into `dir`.
    ///
    /// # Errors
    ///
    /// Returns the I/O error.
    pub fn write(&self, dir: &Path) -> std::io::Result<()> {
        std::fs::write(
            dir.join("published.json"),
            json_bytes(&serde_json::to_value(self).unwrap_or(Value::Null)),
        )
    }
}
