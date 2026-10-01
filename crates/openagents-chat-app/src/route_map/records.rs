//! The published records behind each plugin's evaluation status: verified
//! NIP-EVAL results, checks, and validations (`3189`), the NIP-EXT
//! releases they name (`3184`), and the adoptions into Coder's defaults.
//!
//! [`RECORDS`] (`route_map/records.json`) is the committed snapshot the
//! map reads with no network. `crates/coder/tests/route_map_sources.rs`
//! writes it from the relay (`live_route_map_records`, ignored): every
//! result is admitted only when `nostr::eval_ext::parse_publication`
//! accepts it, a check counts only as `nostr::eval_ext::linkage` reads it,
//! and an adoption only from a `coder-defaults` release whose manifest
//! matches the committed documents. A window may pass fresher records of
//! the same shape ([`Records::parse`]).

use serde::{Deserialize, Serialize};

/// The committed snapshot.
pub const RECORDS: &str = include_str!("records.json");

/// The snapshot's schema.
pub const SCHEMA: &str = "openagents.route-map.records.v1";

/// Every record the map reads.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Records {
    pub schema: String,
    pub generated_by: String,
    /// The relay they were read from.
    pub relay: String,
    /// When they were read, in Unix seconds.
    pub fetched_at: u64,
    /// NIP-EXT releases of plugins and test sets.
    pub releases: Vec<ReleaseRecord>,
    /// Verified results, checks, and validations, newest first.
    pub results: Vec<ResultRecord>,
    /// Adoptions into Coder's defaults, newest first.
    pub adoptions: Vec<AdoptionRecord>,
}

/// A NIP-EXT release.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReleaseRecord {
    pub id: String,
    pub pubkey: String,
    /// `publisher:slug`.
    pub package: String,
    pub version: String,
    pub created_at: u64,
}

/// What a result's gate decided.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Verdict {
    Better,
    NoClearChange,
    Worse,
}

impl Verdict {
    /// The words on screen.
    #[must_use]
    pub fn words(self) -> &'static str {
        match self {
            Verdict::Better => "Better",
            Verdict::NoClearChange => "No clear change",
            Verdict::Worse => "Worse",
        }
    }
}

/// A verified result publication: an original result, a check of one, or
/// a validation of one.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ResultRecord {
    pub id: String,
    pub created_at: u64,
    /// The signer.
    pub evaluator: String,
    /// The trainer it is credited to.
    pub trainer: String,
    /// The subject's DefinitionRef id, `publisher:slug/component`.
    pub subject: String,
    /// The subject's release, when it names one.
    pub subject_release: Option<String>,
    /// The test set's release.
    pub suite_release: String,
    pub verdict: Verdict,
    /// Tests passed with the plugin.
    pub with: u64,
    /// Tests passed without it.
    pub without: Option<u64>,
    /// Tests in the set.
    pub total: u64,
    /// The result this one checks.
    pub checks: Option<String>,
    /// The result this one validates on a second test set.
    pub validates: Option<String>,
    /// Checks of this result that confirm it, and that dispute it.
    pub confirmed: u64,
    pub disputed: u64,
}

impl ResultRecord {
    /// Whether this is an original result, neither a check nor a
    /// validation.
    #[must_use]
    pub fn original(&self) -> bool {
        self.checks.is_none() && self.validates.is_none()
    }

    /// "5 of 6 with, 2 of 6 without".
    #[must_use]
    pub fn headline(&self) -> String {
        match self.without {
            Some(without) => format!(
                "{} of {} with, {without} of {} without",
                self.with, self.total, self.total
            ),
            None => format!("{} of {} with", self.with, self.total),
        }
    }
}

/// An adoption: a `coder-defaults` release that depends on a plugin's
/// release, citing an admission.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AdoptionRecord {
    /// The `coder-defaults` release.
    pub defaults_release: String,
    /// The plugin release it depends on.
    pub release: String,
    /// The admission's digest.
    pub admission: String,
    pub at: u64,
}

impl Records {
    /// The committed snapshot.
    ///
    /// # Panics
    ///
    /// The committed file does not parse, which its test rules out.
    #[must_use]
    pub fn committed() -> Self {
        Self::parse(RECORDS).expect("the committed route map records parse")
    }

    /// Reads a snapshot.
    ///
    /// # Errors
    ///
    /// The JSON doesn't parse or names another schema.
    pub fn parse(text: &str) -> Result<Self, String> {
        let records: Records = serde_json::from_str(text).map_err(|error| error.to_string())?;
        if records.schema != SCHEMA {
            return Err(format!("the records name {}, not {SCHEMA}", records.schema));
        }
        Ok(records)
    }

    /// The document as the generator writes it.
    #[must_use]
    pub fn document(&self) -> String {
        serde_json::to_string_pretty(self).unwrap_or_default() + "\n"
    }

    /// The release with this id.
    #[must_use]
    pub fn release(&self, id: &str) -> Option<&ReleaseRecord> {
        self.releases.iter().find(|release| release.id == id)
    }

    /// The results whose subject is the package `publisher:slug`: named by
    /// its DefinitionRef id or by one of its releases.
    #[must_use]
    pub fn of_package(&self, package: &str) -> Vec<&ResultRecord> {
        let prefix = format!("{package}/");
        self.results
            .iter()
            .filter(|result| {
                result.subject.starts_with(&prefix)
                    || result
                        .subject_release
                        .as_deref()
                        .and_then(|id| self.release(id))
                        .is_some_and(|release| release.package == package)
            })
            .collect()
    }

    /// The adoptions of the package `publisher:slug`.
    #[must_use]
    pub fn adoptions_of(&self, package: &str) -> Vec<&AdoptionRecord> {
        self.adoptions
            .iter()
            .filter(|adoption| {
                self.release(&adoption.release)
                    .is_some_and(|release| release.package == package)
            })
            .collect()
    }
}
