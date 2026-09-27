//! NIP-SOV, step one of its implementation order: the pure contracts. A
//! sovereign profile is drafted, validated, and kept under
//! `~/.openagents/sov/`; `spawn` checks every precondition the NIP puts
//! before an activation and refuses, naming each missing piece, until a
//! host supplies them. Nothing here grants an agent authority: a profile on
//! disk is a draft until an admitting host authenticates it.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::{Args, Output};

const USAGE: &str = "usage: openagents sov COMMAND [OPTIONS]
  profile new NAME --agent PUBKEY --authority PUBKEY --policy REF --custody-adapter ID --custody-policy REF --state-schema REF --disclosure REF [--guardian REF] [--treasury REF] [--evidence REF]...
                            Draft revision 0 of a sovereign profile.
  profile validate FILE     Check a profile body against the SOV rules.
  profile show NAME         Print a stored profile.
  profile list              List stored profile drafts.
  spawn NAME [--host URL]   Activate the agent NAME: refuses until every SOV
                            precondition is met, and names what is missing.
  status                    What this machine can and cannot do for SOV.
REF is an ArtifactRef as sha256:HEX,SIZE,MEDIA_TYPE[,SCHEMA]. Profiles live
in ~/.openagents/sov/ (SOV_HOME overrides).";

pub const PROFILE_VERSION: &str = "openagents.sovereign-profile.v1";
const MAX_EVIDENCE: usize = 64;

/// An `ArtifactRef` from the shared contracts: exact bytes by digest.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ArtifactRef {
    pub digest: String,
    pub size: u64,
    pub media_type: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub schema: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub event: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sources: Option<Vec<Value>>,
}

impl ArtifactRef {
    /// Parses `sha256:HEX,SIZE,MEDIA_TYPE[,SCHEMA]`.
    pub fn parse(text: &str) -> Result<Self, String> {
        let parts: Vec<&str> = text.split(',').collect();
        let (digest, size, media_type, schema) = match parts.as_slice() {
            [digest, size, media] => (*digest, *size, *media, None),
            [digest, size, media, schema] => (*digest, *size, *media, Some((*schema).to_owned())),
            _ => {
                return Err(format!(
                    "`{text}` is not sha256:HEX,SIZE,MEDIA_TYPE[,SCHEMA]"
                ));
            }
        };
        let reference = Self {
            digest: digest.to_owned(),
            size: size
                .parse()
                .map_err(|_| format!("`{size}` is not a byte length"))?,
            media_type: media_type.to_owned(),
            schema,
            event: None,
            sources: None,
        };
        reference.check(text)?;
        Ok(reference)
    }

    pub fn check(&self, what: &str) -> Result<(), String> {
        let hex = self.digest.strip_prefix("sha256:").unwrap_or("");
        if hex.len() != 64
            || !hex
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        {
            return Err(format!(
                "{what}: digest must be sha256: and 64 lowercase hex"
            ));
        }
        if self.media_type.is_empty()
            || self.media_type.len() > 128
            || self.media_type != self.media_type.to_lowercase()
            || !self.media_type.contains('/')
        {
            return Err(format!("{what}: media_type must be a lowercase MIME type"));
        }
        if self
            .schema
            .as_ref()
            .is_some_and(|s| s.is_empty() || s.len() > 128)
        {
            return Err(format!("{what}: schema must be 1 to 128 bytes"));
        }
        Ok(())
    }
}

/// `{adapter, policy}`: a pinned custody adapter and the policy it enforces.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Custody {
    pub adapter: Value,
    pub policy: ArtifactRef,
}

/// `openagents.sovereign-profile.v1`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Profile {
    pub v: String,
    #[serde(default)]
    pub requires: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub meta: Option<Value>,
    pub agent: String,
    pub authority: String,
    pub revision: u64,
    pub previous: Option<ArtifactRef>,
    pub policy: ArtifactRef,
    pub custody: Custody,
    pub guardian_policy: Option<ArtifactRef>,
    pub treasury: Option<ArtifactRef>,
    pub state_schema: ArtifactRef,
    pub disclosure: ArtifactRef,
    #[serde(default)]
    pub evidence: Vec<ArtifactRef>,
}

fn is_key(text: &str) -> bool {
    text.len() == 64
        && text
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

impl Profile {
    /// Every structural rule the NIP states for a profile body.
    pub fn validate(&self) -> Result<(), String> {
        if self.v != PROFILE_VERSION {
            return Err(format!("v must be {PROFILE_VERSION}"));
        }
        if !self.requires.is_empty() {
            return Err(format!(
                "requires names features this program does not implement: {:?}",
                self.requires
            ));
        }
        if !is_key(&self.agent) {
            return Err("agent must be a lowercase 64-hex x-only public key".into());
        }
        if !is_key(&self.authority) {
            return Err("authority must be a lowercase 64-hex x-only public key".into());
        }
        match (&self.revision, &self.previous) {
            (0, None) => {}
            (0, Some(_)) => return Err("revision 0 has previous: null".into()),
            (_, None) => return Err("a revision after 0 references its predecessor".into()),
            (_, Some(previous)) => previous.check("previous")?,
        }
        self.policy.check("policy")?;
        let adapter = &self.custody.adapter;
        let id_ok = adapter["id"]
            .as_str()
            .is_some_and(|id| !id.is_empty() && id.len() <= 128);
        let artifact: Result<ArtifactRef, _> = serde_json::from_value(adapter["artifact"].clone());
        if !id_ok
            || artifact
                .map_err(|e| e.to_string())?
                .check("custody.adapter.artifact")
                .is_err()
        {
            return Err("custody.adapter must be a DefinitionRef {id, artifact, event?}".into());
        }
        if adapter.as_object().is_some_and(|map| {
            map.keys()
                .any(|k| !["id", "artifact", "event"].contains(&k.as_str()))
        }) {
            return Err("custody.adapter carries an unknown field".into());
        }
        self.custody.policy.check("custody.policy")?;
        if let Some(guardian) = &self.guardian_policy {
            guardian.check("guardian_policy")?;
        }
        if let Some(treasury) = &self.treasury {
            treasury.check("treasury")?;
        }
        self.state_schema.check("state_schema")?;
        if self.state_schema.media_type != "application/schema+json" {
            return Err("state_schema must be a SchemaRef (application/schema+json)".into());
        }
        self.disclosure.check("disclosure")?;
        if self.evidence.len() > MAX_EVIDENCE {
            return Err(format!("evidence has at most {MAX_EVIDENCE} entries"));
        }
        for (i, item) in self.evidence.iter().enumerate() {
            item.check(&format!("evidence[{i}]"))?;
        }
        Ok(())
    }
}

/// Where profile drafts live.
pub fn home() -> PathBuf {
    if let Some(dir) = std::env::var_os("SOV_HOME") {
        return PathBuf::from(dir);
    }
    verse::identity::home()
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."))
        .join("sov")
}

fn profile_path(name: &str) -> Result<PathBuf, String> {
    let ok = !name.is_empty()
        && name.len() <= 64
        && name
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-' || b == b'_');
    if !ok {
        return Err("NAME is 1 to 64 bytes of [a-z0-9_-]".into());
    }
    Ok(home().join(format!("{name}.profile.json")))
}

fn load(name: &str) -> Result<Profile, String> {
    let path = profile_path(name)?;
    let text = std::fs::read_to_string(&path)
        .map_err(|e| format!("no profile draft `{name}` at {}: {e}", path.display()))?;
    if text.len() > 64 * 1024 {
        return Err("profile is larger than 64 KiB".into());
    }
    serde_json::from_str(&text).map_err(|e| format!("profile does not parse: {e}"))
}

/// The pieces an activation needs and whether this machine has them.
fn capabilities() -> Vec<Value> {
    let missing = |name: &str, contract: &str, why: &str| json!({ "capability": name, "contract": contract, "present": false, "why": why });
    vec![
        json!({ "capability": "profile-contract", "contract": "NIP-SOV", "present": true,
                "why": "openagents.sovereign-profile.v1 is validated by this program" }),
        missing(
            "admitting-authority",
            "NIP-SOV",
            "no host has authenticated an authority for a profile; self-publication cannot",
        ),
        missing(
            "custody-adapter",
            "NIP-CAP / NIP-46",
            "no supported custody adapter holds the agent's signing identity",
        ),
        missing(
            "policy-store",
            "NIP-POL",
            "no admitted POL governance, custody, or disclosure policy is retained locally",
        ),
        missing(
            "controller",
            "NIP-AUTO / NIP-COORD",
            "no controller issues generations, claims, or fencing for an activation",
        ),
        missing(
            "environment",
            "NIP-ENV",
            "no host materializes an environment lease for an activation",
        ),
        missing(
            "checkpoints",
            "NIP-RUN",
            "no durable checkpoint and recovery store exists",
        ),
    ]
}

pub fn run(output: &Output, words: &[String]) -> u8 {
    let Some((command, rest)) = words.split_first() else {
        return output.usage("sov", "a command is required", USAGE);
    };
    let result = match command.as_str() {
        "--help" | "-h" | "help" => {
            println!("{USAGE}");
            Ok(0)
        }
        "profile" => profile(output, rest),
        "spawn" | "activate" => spawn(output, rest),
        "status" => {
            let caps = capabilities();
            output.emit(
                &json!({ "home": home(), "capabilities": caps }),
                render_caps,
            );
            Ok(0)
        }
        other => return output.usage("sov", &format!("unknown command `{other}`"), USAGE),
    };
    match result {
        Ok(code) => code,
        Err(message) => output.fail("sov", &message),
    }
}

fn render_caps(value: &Value) -> String {
    value["capabilities"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|cap| {
            format!(
                "{} {:<20} {:<22} {}",
                if cap["present"].as_bool().unwrap_or(false) {
                    "ok     "
                } else {
                    "missing"
                },
                cap["capability"].as_str().unwrap_or(""),
                cap["contract"].as_str().unwrap_or(""),
                cap["why"].as_str().unwrap_or("")
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn profile(output: &Output, words: &[String]) -> Result<u8, String> {
    let Some((sub, rest)) = words.split_first() else {
        return Err("profile takes new, validate, show, or list".into());
    };
    let args = Args::parse(rest, &[])?;
    match sub.as_str() {
        "new" => {
            let Some(name) = args.positional().first() else {
                return Err("NAME is required".into());
            };
            let path = profile_path(name)?;
            if path.exists() {
                return Err(format!(
                    "{} exists; profiles are immutable, draft a new name",
                    path.display()
                ));
            }
            let need = |flag: &str| -> Result<String, String> {
                args.option(flag)
                    .map(str::to_owned)
                    .ok_or_else(|| format!("--{flag} is required"))
            };
            let adapter_id = need("custody-adapter")?;
            let adapter_artifact = match args.option("custody-adapter-artifact") {
                Some(text) => ArtifactRef::parse(text)?,
                None => return Err("--custody-adapter-artifact REF is required (the adapter definition's ArtifactRef)".into()),
            };
            let profile = Profile {
                v: PROFILE_VERSION.into(),
                requires: Vec::new(),
                meta: None,
                agent: need("agent")?,
                authority: need("authority")?,
                revision: 0,
                previous: None,
                policy: ArtifactRef::parse(&need("policy")?)?,
                custody: Custody {
                    adapter: json!({ "id": adapter_id, "artifact": adapter_artifact }),
                    policy: ArtifactRef::parse(&need("custody-policy")?)?,
                },
                guardian_policy: args
                    .option("guardian")
                    .map(ArtifactRef::parse)
                    .transpose()?,
                treasury: args
                    .option("treasury")
                    .map(ArtifactRef::parse)
                    .transpose()?,
                state_schema: ArtifactRef::parse(&need("state-schema")?)?,
                disclosure: ArtifactRef::parse(&need("disclosure")?)?,
                evidence: args
                    .options("evidence")
                    .into_iter()
                    .map(ArtifactRef::parse)
                    .collect::<Result<_, _>>()?,
            };
            profile.validate()?;
            std::fs::create_dir_all(home()).map_err(|e| e.to_string())?;
            let text = serde_json::to_string_pretty(&profile).map_err(|e| e.to_string())?;
            std::fs::write(&path, text).map_err(|e| format!("write {}: {e}", path.display()))?;
            output.emit(
                &json!({ "name": name, "path": path, "profile": profile, "admitted": false }),
                |value| {
                    format!(
                        "drafted {} (not admitted: no authority has authenticated it)",
                        value["path"].as_str().unwrap_or("")
                    )
                },
            );
            Ok(0)
        }
        "validate" => {
            let Some(file) = args.positional().first() else {
                return Err("FILE is required".into());
            };
            let text = std::fs::read_to_string(file).map_err(|e| format!("{file}: {e}"))?;
            let profile: Profile =
                serde_json::from_str(&text).map_err(|e| format!("{file}: {e}"))?;
            match profile.validate() {
                Ok(()) => {
                    output.emit(&json!({ "valid": true, "agent": profile.agent, "revision": profile.revision }), |v| {
                        format!("valid: agent {} revision {}", v["agent"].as_str().unwrap_or(""), v["revision"])
                    });
                    Ok(0)
                }
                Err(message) => {
                    output.emit(&json!({ "valid": false, "error": message }), |v| {
                        format!("invalid: {}", v["error"].as_str().unwrap_or(""))
                    });
                    Ok(crate::EXIT_FAILURE)
                }
            }
        }
        "show" => {
            let Some(name) = args.positional().first() else {
                return Err("NAME is required".into());
            };
            let profile = load(name)?;
            output.emit(
                &serde_json::to_value(&profile).map_err(|e| e.to_string())?,
                |v| serde_json::to_string_pretty(v).unwrap_or_default(),
            );
            Ok(0)
        }
        "list" => {
            let mut names = Vec::new();
            if let Ok(entries) = std::fs::read_dir(home()) {
                for entry in entries.flatten() {
                    let file = entry.file_name().to_string_lossy().into_owned();
                    if let Some(name) = file.strip_suffix(".profile.json") {
                        names.push(name.to_owned());
                    }
                }
            }
            names.sort();
            output.emit(&json!({ "profiles": names }), |v| {
                v["profiles"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter_map(Value::as_str)
                    .collect::<Vec<_>>()
                    .join("\n")
            });
            Ok(0)
        }
        other => Err(format!("profile has no `{other}`")),
    }
}

/// Activation, fail closed: every precondition the NIP names is checked in
/// order and the first refusal stops the command. Today the profile
/// contract is the only precondition this machine can satisfy.
fn spawn(output: &Output, words: &[String]) -> Result<u8, String> {
    let args = Args::parse(words, &[])?;
    let Some(name) = args.positional().first() else {
        return Err("NAME is required".into());
    };
    let profile = load(name)?;
    let mut checks = vec![match profile.validate() {
        Ok(()) => json!({ "check": "profile-contract", "ok": true }),
        Err(error) => json!({ "check": "profile-contract", "ok": false, "why": error }),
    }];
    for cap in capabilities().into_iter().skip(1) {
        checks.push(json!({
            "check": cap["capability"],
            "ok": cap["present"],
            "why": cap["why"],
        }));
    }
    let refused = checks
        .iter()
        .find(|check| !check["ok"].as_bool().unwrap_or(false))
        .cloned();
    output.emit(
        &json!({
            "agent": profile.agent,
            "revision": profile.revision,
            "activated": refused.is_none(),
            "refused_by": refused.as_ref().map(|c| c["check"].clone()),
            "checks": checks,
        }),
        |value| {
            let lines: Vec<String> = value["checks"]
                .as_array()
                .into_iter()
                .flatten()
                .map(|check| {
                    format!(
                        "{} {}{}",
                        if check["ok"].as_bool().unwrap_or(false) {
                            "ok     "
                        } else {
                            "refused"
                        },
                        check["check"].as_str().unwrap_or(""),
                        check["why"]
                            .as_str()
                            .map(|w| format!(": {w}"))
                            .unwrap_or_default()
                    )
                })
                .collect();
            format!(
                "{}\nspawn refused: an activation needs every check above; nothing was started",
                lines.join("\n")
            )
        },
    );
    Ok(if refused.is_some() {
        crate::EXIT_FAILURE
    } else {
        0
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn reference(media: &str) -> ArtifactRef {
        ArtifactRef::parse(&format!("sha256:{},12,{media}", "a".repeat(64))).unwrap()
    }

    fn profile() -> Profile {
        Profile {
            v: PROFILE_VERSION.into(),
            requires: Vec::new(),
            meta: None,
            agent: "b".repeat(64),
            authority: "c".repeat(64),
            revision: 0,
            previous: None,
            policy: reference("application/json"),
            custody: Custody {
                adapter: json!({ "id": "openagents.custody.local", "artifact": reference("application/json") }),
                policy: reference("application/json"),
            },
            guardian_policy: None,
            treasury: None,
            state_schema: reference("application/schema+json"),
            disclosure: reference("application/json"),
            evidence: Vec::new(),
        }
    }

    #[test]
    fn accepts_a_well_formed_revision_zero() {
        profile().validate().unwrap();
    }

    #[test]
    fn refuses_bad_lineage_keys_and_features() {
        let mut p = profile();
        p.previous = Some(reference("application/json"));
        assert!(p.validate().is_err());
        let mut p = profile();
        p.revision = 1;
        assert!(p.validate().is_err());
        let mut p = profile();
        p.agent = "B".repeat(64);
        assert!(p.validate().is_err());
        let mut p = profile();
        p.requires = vec!["threshold-custody".into()];
        assert!(p.validate().is_err());
        let mut p = profile();
        p.state_schema = reference("application/json");
        assert!(p.validate().is_err());
        let mut p = profile();
        p.custody.adapter["extra"] = json!(1);
        assert!(p.validate().is_err());
    }

    #[test]
    fn parses_artifact_refs() {
        assert!(ArtifactRef::parse("sha256:zz,1,application/json").is_err());
        assert!(
            ArtifactRef::parse(&format!("sha256:{},1,Application/JSON", "0".repeat(64))).is_err()
        );
        let r = ArtifactRef::parse(&format!(
            "sha256:{},1,application/json,x.v1",
            "0".repeat(64)
        ))
        .unwrap();
        assert_eq!(r.schema.as_deref(), Some("x.v1"));
    }
}
