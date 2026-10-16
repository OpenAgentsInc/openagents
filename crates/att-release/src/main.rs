//! `att-release`: check a NIP-ATT release with its transparency entry,
//! rebuild statements and personal approvals; sign a rebuild statement;
//! keep the approval set; verify a sealed-job receipt.
use att_release::approvals::{ApprovalSet, Decision};
use att_release::{Evidence, Policy, rebuild, receipt, rekor};
use nostr::domain::{Event, RelaySigner};
use std::collections::BTreeMap;

const USAGE: &str = "usage:
  att-release verify --release R.json --head H.json --publisher HEX --workload SLUG --now SECS
      [--rekor ENTRY.json --log-key KEY.pem [--log-name NAME] --witness STATE.json [--consistency PROOF.json]]
      [--rebuild EVENT.json]... [--min-rebuilds N] [--approvals SET.json]
  att-release rebuild --secret-file FILE --release R.json --image-digest sha256:HEX --now SECS
  att-release approve|revoke --approvals SET.json --release ID [--workload SLUG] --now SECS
  att-release receipt --receipt E.json --request E.json --result E.json --endpoint ADDR --endpoint-key HEX
      --release ID --level tee|tee-cloud --measurement M";

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match run(&args) {
        Ok(out) => println!("{out}"),
        Err(e) => {
            eprintln!("{e}");
            std::process::exit(1);
        }
    }
}

struct Flags {
    one: BTreeMap<String, String>,
    many: Vec<(String, String)>,
}

impl Flags {
    fn parse(args: &[String]) -> Result<Self, String> {
        let mut one = BTreeMap::new();
        let mut many = Vec::new();
        let mut it = args.iter();
        while let Some(flag) = it.next() {
            let name = flag
                .strip_prefix("--")
                .ok_or_else(|| format!("unexpected argument {flag}\n{USAGE}"))?;
            let value = it.next().ok_or_else(|| format!("--{name} needs a value"))?;
            many.push((name.to_string(), value.clone()));
            one.insert(name.to_string(), value.clone());
        }
        Ok(Self { one, many })
    }
    fn get(&self, name: &str) -> Result<&str, String> {
        self.one
            .get(name)
            .map(String::as_str)
            .ok_or_else(|| format!("--{name} is required\n{USAGE}"))
    }
    fn opt(&self, name: &str) -> Option<&str> {
        self.one.get(name).map(String::as_str)
    }
    fn all(&self, name: &str) -> Vec<&str> {
        self.many
            .iter()
            .filter(|(n, _)| n == name)
            .map(|(_, v)| v.as_str())
            .collect()
    }
    fn now(&self) -> Result<u64, String> {
        self.get("now")?
            .parse()
            .map_err(|_| "--now is not a number".into())
    }
}

fn read(path: &str) -> Result<String, String> {
    std::fs::read_to_string(path).map_err(|e| format!("cannot read {path}: {e}"))
}

fn json<T: serde::de::DeserializeOwned>(path: &str) -> Result<T, String> {
    serde_json::from_str(&read(path)?).map_err(|e| format!("{path} does not parse: {e}"))
}

fn write(path: &str, value: &impl serde::Serialize) -> Result<(), String> {
    let text = serde_json::to_string_pretty(value).map_err(|e| e.to_string())?;
    std::fs::write(path, text).map_err(|e| format!("cannot write {path}: {e}"))
}

fn approvals_file(path: &str) -> Result<ApprovalSet, String> {
    if std::path::Path::new(path).exists() {
        json(path)
    } else {
        Ok(ApprovalSet::default())
    }
}

/// Run one command and return what it prints.
///
/// # Errors
///
/// With the refusal or usage problem.
pub fn run(args: &[String]) -> Result<String, String> {
    let (command, rest) = args.split_first().ok_or(USAGE)?;
    let flags = Flags::parse(rest)?;
    match command.as_str() {
        "verify" => verify(&flags),
        "rebuild" => {
            let secret = read(flags.get("secret-file")?)?;
            let builder = RelaySigner::from_secret_hex(secret.trim()).map_err(|e| e.to_string())?;
            let release_event: Event = json(flags.get("release")?)?;
            let release = nostr::att::parse_release(&release_event)?;
            let statement = rebuild::Rebuild {
                v: rebuild::REBUILD_V.into(),
                requires: Vec::new(),
                workload: release.workload,
                source: release.source,
                image_digest: flags.get("image-digest")?.into(),
                components: release.components,
                built_at: flags.now()?,
            };
            serde_json::to_string(&rebuild::event(&builder, &statement)?).map_err(|e| e.to_string())
        }
        "approve" | "revoke" => {
            let path = flags.get("approvals")?;
            let mut set = approvals_file(path)?;
            set.check(None)?;
            let release = flags.get("release")?;
            if command == "approve" {
                set.approve(release, flags.get("workload")?, flags.now()?);
            } else {
                set.revoke(release, flags.now()?)?;
            }
            write(path, &set)?;
            Ok(format!(
                "{command}d {release}; approval set generation {}",
                set.generation
            ))
        }
        "receipt" => {
            let event: Event = json(flags.get("receipt")?)?;
            let attested = receipt::Attested {
                endpoint: flags.get("endpoint")?.into(),
                release: flags.get("release")?.into(),
                level: flags.get("level")?.into(),
                measurement: flags.get("measurement")?.into(),
            };
            let r = receipt::verify(
                &event,
                &attested,
                flags.get("endpoint-key")?,
                &json(flags.get("request")?)?,
                &json(flags.get("result")?)?,
            )?;
            Ok(format!(
                "sealed receipt verified: {:?} for request {}",
                r.outcome, r.request
            ))
        }
        _ => Err(USAGE.into()),
    }
}

fn verify(flags: &Flags) -> Result<String, String> {
    let release: Event = json(flags.get("release")?)?;
    let head: Event = json(flags.get("head")?)?;
    let entry: Option<rekor::Entry> = flags.opt("rekor").map(json).transpose()?;
    let key = flags
        .opt("log-key")
        .map(|p| {
            rekor::LogKey::from_pem(
                flags.opt("log-name").unwrap_or(rekor::SIGSTORE_REKOR_NAME),
                &read(p)?,
            )
        })
        .transpose()?;
    let consistency: Option<Vec<String>> = flags.opt("consistency").map(json).transpose()?;
    let consistency: Option<Vec<rekor::Hash>> = consistency
        .map(|hashes| {
            hashes
                .iter()
                .map(|h| {
                    let b: Vec<u8> = (0..h.len() / 2)
                        .map(|i| u8::from_str_radix(&h[2 * i..2 * i + 2], 16))
                        .collect::<Result<_, _>>()
                        .map_err(|_| "bad proof hash")?;
                    b.try_into()
                        .map_err(|_| "a proof hash is not 32 bytes".to_string())
                })
                .collect::<Result<_, String>>()
        })
        .transpose()?;
    let rebuilds: Vec<Event> = flags
        .all("rebuild")
        .into_iter()
        .map(json)
        .collect::<Result<_, _>>()?;
    let approvals: Option<ApprovalSet> = flags.opt("approvals").map(approvals_file).transpose()?;
    let witness_path = flags.opt("witness");
    if key.is_some() && witness_path.is_none() {
        return Err("--witness is required with --log-key".into());
    }
    let mut witness: rekor::Witness = match witness_path {
        Some(p) if std::path::Path::new(p).exists() => json(p)?,
        _ => rekor::Witness::default(),
    };
    let policy = Policy {
        publisher: flags.get("publisher")?,
        workload: flags.get("workload")?,
        log: key.as_ref(),
        min_rebuilds: flags
            .opt("min-rebuilds")
            .map(str::parse)
            .transpose()
            .map_err(|_| "--min-rebuilds is not a number")?
            .unwrap_or(0),
        seen_generation: None,
    };
    let evidence = Evidence {
        release: &release,
        head: &head,
        entry: entry.as_ref(),
        consistency: consistency.as_deref(),
        rebuilds: &rebuilds,
        approvals: approvals.as_ref(),
    };
    let decision = att_release::check_release(&evidence, &policy, &mut witness, flags.now()?)?;
    if let Some(p) = witness_path {
        write(p, &witness)?;
    }
    match decision {
        Decision::Admitted => Ok(format!("admitted {}", release.id)),
        Decision::AwaitingApproval { emergency } => Err(format!(
            "awaiting your approval{} for {}",
            if emergency {
                " (emergency release)"
            } else {
                ""
            },
            release.id
        )),
        Decision::Revoked => Err(format!("your approval of {} is revoked", release.id)),
    }
}

#[cfg(test)]
mod cli_tests {
    #[test]
    fn usage_and_approvals_round_trip() {
        assert!(super::run(&[]).is_err());
        let dir = std::env::temp_dir().join(format!("att-release-cli-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let set = dir.join("approvals.json").to_string_lossy().to_string();
        let args = |v: &[&str]| v.iter().map(|s| (*s).to_string()).collect::<Vec<_>>();
        let id = "ab".repeat(32);
        assert!(
            super::run(&args(&[
                "approve",
                "--approvals",
                &set,
                "--release",
                &id,
                "--workload",
                "w",
                "--now",
                "10"
            ]))
            .unwrap()
            .contains("generation 1")
        );
        assert!(
            super::run(&args(&[
                "revoke",
                "--approvals",
                &set,
                "--release",
                &id,
                "--now",
                "11"
            ]))
            .unwrap()
            .contains("generation 2")
        );
        assert!(
            super::run(&args(&[
                "revoke",
                "--approvals",
                &set,
                "--release",
                &"cd".repeat(32),
                "--now",
                "12"
            ]))
            .is_err()
        );
        std::fs::remove_dir_all(dir).unwrap();
    }
}
