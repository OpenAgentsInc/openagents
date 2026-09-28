//! The buyer's standing policy and the ledger of what it paid.
//!
//! `policy.json` holds ceilings the buyer commands apply when no flag names
//! one: a default, one per provider node (`payTo`), and one per capability
//! (`PUBKEY:SLUG`), plus an allowlist of provider nodes and a daily cap. Flags
//! can raise a per-call ceiling; nothing on the command line raises the daily
//! cap or bypasses the allowlist. `ledger.ndjson` records every payment the
//! buyer made across the three bindings, one line each, and is what the
//! daily cap counts.

use std::collections::BTreeMap;
use std::fs;
use std::io::{BufRead, Write};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

pub const POLICY_FILE: &str = "policy.json";
pub const LEDGER_FILE: &str = "ledger.ndjson";
pub const DAY_SECS: u64 = 24 * 3_600;

/// A spending ceiling for one call. `None` means the policy does not say.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Ceiling {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_msat: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_fee_msat: Option<u64>,
}

impl Ceiling {
    fn over(self, base: Ceiling) -> Ceiling {
        Ceiling {
            max_msat: self.max_msat.or(base.max_msat),
            max_fee_msat: self.max_fee_msat.or(base.max_fee_msat),
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Policy {
    #[serde(default)]
    pub default: Ceiling,
    /// Most the buyer spends in any rolling 24 hours, amounts and fees.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub daily_cap_msat: Option<u64>,
    /// Ceilings by provider node id (`payTo`).
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub providers: BTreeMap<String, Ceiling>,
    /// Ceilings by capability, `PUBKEY:SLUG`.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub capabilities: BTreeMap<String, Ceiling>,
    /// Provider node ids the buyer pays. Empty admits every provider.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub allow: Vec<String>,
}

/// What the caller asked for on the command line.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Flags {
    pub max_msat: Option<u64>,
    pub max_fee_msat: Option<u64>,
}

/// The ceilings one call runs under after policy and flags are combined.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Limits {
    pub max_msat: u64,
    pub max_fee_msat: u64,
    /// Where `max_msat` came from.
    pub source: Source,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Source {
    Flag,
    Capability,
    Provider,
    #[default]
    Default,
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum PolicyError {
    #[error("no --max-msat and the policy has no ceiling for this call")]
    NoCeiling,
    #[error("the resource costs {amount_msat} msat, above the ceiling {max_msat} ({set_by:?})")]
    AboveCeiling {
        amount_msat: u64,
        max_msat: u64,
        set_by: Source,
    },
    #[error("provider {0} is not on the policy's allowlist")]
    NotAllowed(String),
    #[error(
        "paying {amount_msat} msat would put the last 24 hours at {would_be} msat, above the daily cap {daily_cap_msat}"
    )]
    DailyCap {
        amount_msat: u64,
        would_be: u64,
        daily_cap_msat: u64,
    },
    #[error("policy: {0}")]
    Io(String),
}

impl Policy {
    pub fn load(path: &Path) -> Result<Option<Self>, PolicyError> {
        match fs::read(path) {
            Ok(bytes) => serde_json::from_slice(&bytes)
                .map(Some)
                .map_err(|error| PolicyError::Io(format!("{}: {error}", path.display()))),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(error) => Err(PolicyError::Io(format!("{}: {error}", path.display()))),
        }
    }

    pub fn save(&self, path: &Path) -> Result<(), PolicyError> {
        let io = |error: std::io::Error| PolicyError::Io(format!("{}: {error}", path.display()));
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(io)?;
        }
        let tmp = path.with_extension("json.tmp");
        let bytes = serde_json::to_vec_pretty(self).map_err(|e| PolicyError::Io(e.to_string()))?;
        fs::write(&tmp, bytes).map_err(io)?;
        fs::rename(&tmp, path).map_err(io)
    }

    /// The ceiling for a call to `capability` at `provider`: capability over
    /// provider over default, field by field.
    pub fn ceiling(&self, provider: Option<&str>, capability: Option<&str>) -> (Ceiling, Source) {
        let mut ceiling = self.default;
        let mut source = Source::Default;
        if let Some(found) = provider.and_then(|p| self.providers.get(p)) {
            if found.max_msat.is_some() {
                source = Source::Provider;
            }
            ceiling = found.over(ceiling);
        }
        if let Some(found) = capability.and_then(|c| self.capabilities.get(c)) {
            if found.max_msat.is_some() {
                source = Source::Capability;
            }
            ceiling = found.over(ceiling);
        }
        (ceiling, source)
    }

    /// The limits a call runs under before its price is known. `--max-msat`
    /// wins over the policy; a missing ceiling on both sides is a refusal.
    /// With no policy file, `flags` alone decide.
    pub fn limits(
        policy: Option<&Self>,
        flags: Flags,
        provider: Option<&str>,
        capability: Option<&str>,
    ) -> Result<Limits, PolicyError> {
        let (ceiling, source) = policy
            .map(|p| p.ceiling(provider, capability))
            .unwrap_or_default();
        let (max_msat, source) = match flags.max_msat {
            Some(max) => (max, Source::Flag),
            None => (ceiling.max_msat.ok_or(PolicyError::NoCeiling)?, source),
        };
        let max_fee_msat = flags
            .max_fee_msat
            .or(ceiling.max_fee_msat)
            .unwrap_or(max_msat / 100 + 1_000);
        Ok(Limits {
            max_msat,
            max_fee_msat,
            source,
        })
    }

    /// Whether to pay `amount_msat` to `provider` now, given `spent_24h`
    /// from the ledger. The allowlist and the daily cap hold whatever the
    /// flags said.
    pub fn admit(
        policy: Option<&Self>,
        limits: Limits,
        provider: &str,
        amount_msat: u64,
        spent_24h: u64,
    ) -> Result<(), PolicyError> {
        if amount_msat > limits.max_msat {
            return Err(PolicyError::AboveCeiling {
                amount_msat,
                max_msat: limits.max_msat,
                set_by: limits.source,
            });
        }
        let Some(policy) = policy else {
            return Ok(());
        };
        if !policy.allow.is_empty() && !policy.allow.iter().any(|p| p == provider) {
            return Err(PolicyError::NotAllowed(provider.to_owned()));
        }
        if let Some(cap) = policy.daily_cap_msat {
            let would_be = spent_24h.saturating_add(amount_msat);
            if would_be > cap {
                return Err(PolicyError::DailyCap {
                    amount_msat,
                    would_be,
                    daily_cap_msat: cap,
                });
            }
        }
        Ok(())
    }
}

/// One payment the buyer made.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Entry {
    pub paid_at: u64,
    /// `http:1`, `mcp:1`, or `nostr:openagents:1`.
    pub binding: String,
    pub network: String,
    /// The provider node id the invoice was signed by (`payTo`).
    pub provider: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub capability: Option<String>,
    /// URL, tool name, or `PROVIDER/PURCHASE`.
    pub resource: String,
    pub amount_msat: u64,
    pub fee_msat: u64,
    pub payment_hash: String,
    /// What the buyer got: `completed`, `failed`, or the phase the run ended in.
    pub phase: String,
}

/// Append-only NDJSON, one `Entry` per line.
pub struct Ledger {
    path: PathBuf,
}

impl Ledger {
    pub fn open(path: &Path) -> Self {
        Self {
            path: path.to_path_buf(),
        }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn append(&self, entry: &Entry) -> Result<(), PolicyError> {
        let io =
            |error: std::io::Error| PolicyError::Io(format!("{}: {error}", self.path.display()));
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent).map_err(io)?;
        }
        let mut line = serde_json::to_vec(entry).map_err(|e| PolicyError::Io(e.to_string()))?;
        line.push(b'\n');
        let mut file = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)
            .map_err(io)?;
        file.write_all(&line).map_err(io)?;
        file.sync_data().map_err(io)
    }

    /// Every entry, oldest first. A missing file is an empty ledger; a
    /// malformed line is skipped.
    pub fn entries(&self) -> Result<Vec<Entry>, PolicyError> {
        let file = match fs::File::open(&self.path) {
            Ok(file) => file,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(error) => {
                return Err(PolicyError::Io(format!("{}: {error}", self.path.display())));
            }
        };
        let mut out = Vec::new();
        for line in std::io::BufReader::new(file).lines() {
            let line = line.map_err(|e| PolicyError::Io(e.to_string()))?;
            if let Ok(entry) = serde_json::from_str::<Entry>(&line) {
                out.push(entry);
            }
        }
        Ok(out)
    }

    /// Record how the call paid for by `payment_hash` ended. Rewrites the
    /// file; a hash that is not in it is left alone.
    pub fn set_phase(&self, payment_hash: &str, phase: &str) -> Result<(), PolicyError> {
        let mut entries = self.entries()?;
        let mut changed = false;
        for entry in entries.iter_mut().rev() {
            if entry.payment_hash == payment_hash {
                entry.phase = phase.to_owned();
                changed = true;
                break;
            }
        }
        if !changed {
            return Ok(());
        }
        let io =
            |error: std::io::Error| PolicyError::Io(format!("{}: {error}", self.path.display()));
        let mut bytes = Vec::new();
        for entry in &entries {
            bytes.extend(serde_json::to_vec(entry).map_err(|e| PolicyError::Io(e.to_string()))?);
            bytes.push(b'\n');
        }
        let tmp = self.path.with_extension("ndjson.tmp");
        fs::write(&tmp, bytes).map_err(io)?;
        fs::rename(&tmp, &self.path).map_err(io)
    }

    /// Amounts plus fees paid since `since`.
    pub fn spent_since(&self, since: u64) -> Result<u64, PolicyError> {
        Ok(self
            .entries()?
            .iter()
            .filter(|e| e.paid_at >= since)
            .map(|e| e.amount_msat.saturating_add(e.fee_msat))
            .fold(0u64, u64::saturating_add))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn policy() -> Policy {
        let mut p = Policy {
            default: Ceiling {
                max_msat: Some(5_000),
                max_fee_msat: None,
            },
            daily_cap_msat: Some(20_000),
            ..Policy::default()
        };
        p.providers.insert(
            "02aa".into(),
            Ceiling {
                max_msat: Some(10_000),
                max_fee_msat: Some(50),
            },
        );
        p.capabilities.insert(
            "deadbeef:echo".into(),
            Ceiling {
                max_msat: Some(1_000),
                max_fee_msat: None,
            },
        );
        p
    }

    #[test]
    fn ceilings_resolve_capability_over_provider_over_default() {
        let p = policy();
        let limits = Policy::limits(
            Some(&p),
            Flags::default(),
            Some("02aa"),
            Some("deadbeef:echo"),
        )
        .unwrap();
        assert_eq!((limits.max_msat, limits.max_fee_msat), (1_000, 50));
        assert_eq!(limits.source, Source::Capability);
        let limits = Policy::limits(Some(&p), Flags::default(), Some("02aa"), None).unwrap();
        assert_eq!((limits.max_msat, limits.source), (10_000, Source::Provider));
        let limits = Policy::limits(Some(&p), Flags::default(), Some("02bb"), None).unwrap();
        assert_eq!((limits.max_msat, limits.max_fee_msat), (5_000, 1_050));
        assert_eq!(limits.source, Source::Default);
    }

    #[test]
    fn a_flag_wins_and_no_ceiling_anywhere_refuses() {
        let p = policy();
        let flags = Flags {
            max_msat: Some(99),
            max_fee_msat: None,
        };
        let limits = Policy::limits(Some(&p), flags, Some("02aa"), None).unwrap();
        assert_eq!((limits.max_msat, limits.source), (99, Source::Flag));
        assert_eq!(
            Policy::limits(None, Flags::default(), None, None),
            Err(PolicyError::NoCeiling)
        );
        assert_eq!(
            Policy::limits(
                Some(&Policy::default()),
                Flags::default(),
                Some("02aa"),
                None
            ),
            Err(PolicyError::NoCeiling)
        );
    }

    #[test]
    fn admit_refuses_above_ceiling_off_allowlist_and_over_the_daily_cap() {
        let mut p = policy();
        let limits = Policy::limits(Some(&p), Flags::default(), Some("02aa"), None).unwrap();
        assert!(matches!(
            Policy::admit(Some(&p), limits, "02aa", 10_001, 0),
            Err(PolicyError::AboveCeiling { .. })
        ));
        assert_eq!(Policy::admit(Some(&p), limits, "02aa", 10_000, 0), Ok(()));
        assert!(matches!(
            Policy::admit(Some(&p), limits, "02aa", 10_000, 10_001),
            Err(PolicyError::DailyCap {
                would_be: 20_001,
                ..
            })
        ));
        p.allow.push("02cc".into());
        assert_eq!(
            Policy::admit(Some(&p), limits, "02aa", 1, 0),
            Err(PolicyError::NotAllowed("02aa".into()))
        );
        // Flags cannot lift the allowlist or the cap.
        let flags = Flags {
            max_msat: Some(1_000_000),
            max_fee_msat: None,
        };
        let limits = Policy::limits(Some(&p), flags, Some("02cc"), None).unwrap();
        assert!(matches!(
            Policy::admit(Some(&p), limits, "02cc", 30_000, 0),
            Err(PolicyError::DailyCap { .. })
        ));
    }

    #[test]
    fn the_ledger_appends_reads_back_and_sums_a_window() {
        let dir = std::env::temp_dir().join(format!("x402-ledger-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        let ledger = Ledger::open(&dir.join(LEDGER_FILE));
        assert_eq!(ledger.entries().unwrap(), Vec::new());
        let entry = |paid_at: u64, amount_msat: u64| Entry {
            paid_at,
            binding: "http:1".into(),
            network: "lnbtc:testnet".into(),
            provider: "02aa".into(),
            capability: None,
            resource: "https://example.test/x".into(),
            amount_msat,
            fee_msat: 1,
            payment_hash: "00".repeat(32),
            phase: "completed".into(),
        };
        ledger.append(&entry(100, 10)).unwrap();
        ledger.append(&entry(200, 20)).unwrap();
        assert_eq!(ledger.entries().unwrap().len(), 2);
        assert_eq!(ledger.spent_since(150).unwrap(), 21);
        assert_eq!(ledger.spent_since(0).unwrap(), 32);
        ledger.set_phase(&"00".repeat(32), "failed").unwrap();
        let phases: Vec<_> = ledger
            .entries()
            .unwrap()
            .into_iter()
            .map(|e| e.phase)
            .collect();
        assert_eq!(phases, vec!["completed", "failed"]);
        assert_eq!(ledger.spent_since(0).unwrap(), 32);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_policy_file_round_trips() {
        let dir = std::env::temp_dir().join(format!("x402-policy-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        let path = dir.join(POLICY_FILE);
        assert_eq!(Policy::load(&path).unwrap(), None);
        policy().save(&path).unwrap();
        assert_eq!(Policy::load(&path).unwrap(), Some(policy()));
        let _ = fs::remove_dir_all(&dir);
    }
}
