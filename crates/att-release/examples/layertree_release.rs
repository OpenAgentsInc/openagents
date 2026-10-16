//! Prepare and seal the signet research image's public release evidence.
//! Keys are read from private files; this example has no network access.
use std::{
    fs,
    path::Path,
    time::{SystemTime, UNIX_EPOCH},
};

use att_release::{logged_digest, rekor};
use nostr::{
    att,
    domain::{Event, RelaySigner},
};
use serde::{Serialize, de::DeserializeOwned};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

fn read<T: DeserializeOwned>(path: &str) -> Result<T, Box<dyn std::error::Error>> {
    Ok(serde_json::from_slice(&fs::read(path)?)?)
}

fn write(path: &Path, value: &impl Serialize) -> Result<(), Box<dyn std::error::Error>> {
    fs::write(path, serde_json::to_vec_pretty(value)?)?;
    Ok(())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    let mode = args.first().ok_or("use pubkey, prepare, or finish")?;
    let key = fs::read_to_string(args.get(1).ok_or("provide a private key file")?)?;
    let signer = RelaySigner::from_secret_hex(key.trim()).map_err(|_| "invalid research key")?;
    let now = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();
    match mode.as_str() {
        "pubkey" if args.len() == 2 => println!("{}", signer.pubkey()),
        "prepare" if args.len() == 6 => {
            let build: Value = read(&args[2])?;
            if build["schema"] != "openagents.layertree.image-build.v1"
                || build["signing_service"] != false
            {
                return Err("use a non-signing research build record".into());
            }
            let builders: Vec<String> = read(&args[4])?;
            if builders.len() != 2
                || builders[0] == builders[1]
                || builders.iter().any(|b| b == signer.pubkey())
            {
                return Err("provide two distinct research builders".into());
            }
            let digest = build["image_digest"]
                .as_str()
                .ok_or("missing image digest")?;
            let release = att::Release {
                v: att::RELEASE_V.into(), requires: vec![], workload: "layertree-signet-probe".into(),
                publisher: signer.pubkey().into(),
                image: att::Image { reference: "us-central1-docker.pkg.dev/openagentsgemini/layertree-research/attestation-probe".into(), digest: digest.into() },
                platforms: vec![att::Platform { kind: "gcp-confidential-space".into(), hwmodel: "GCP_INTEL_TDX".into(), support: "STABLE".into() }],
                measurements: vec![], gpu: None, models: vec![], components: vec![],
                source: att::Source { repo: "https://github.com/OpenAgentsInc/openagents".into(),
                    commit: build["source_commit"].as_str().ok_or("missing source commit")?.into(),
                    recipe: "deploy/layertree/reproducible-probe.Dockerfile".into(),
                    recipe_digest: format!("sha256:{:x}", Sha256::digest(fs::read(&args[3])?)) },
                rebuilds: builders.into_iter().map(|builder| att::Rebuild { builder, digest: digest.into() }).collect(),
                transparency: vec![],
                changes: "We reproduced the signet attestation probe on two machines. This image signs no Bitcoin transaction.".into(),
                published_at: now,
            };
            let event = att::release_event(&signer, &release, now)?;
            let out = Path::new(&args[5]);
            fs::create_dir_all(out)?;
            write(&out.join("draft-event.json"), &event)?;
            fs::write(
                out.join("logged-body.json"),
                nostr::contracts::jcs(&serde_json::to_value(&release)?)?,
            )?;
            println!("{}", logged_digest(&release)?);
        }
        "finish" if args.len() == 5 => {
            let draft: Event = read(&args[2])?;
            let mut release = att::parse_release(&draft)?;
            if release.publisher != signer.pubkey()
                || release.workload != "layertree-signet-probe"
                || !release.transparency.is_empty()
            {
                return Err("use this research publisher's unlogged draft".into());
            }
            let entry: rekor::Entry = read(&args[3])?;
            let included = rekor::verify_entry(
                &entry,
                &rekor::LogKey::sigstore()?,
                &mut rekor::Witness::default(),
                None,
            )?;
            if included.body["kind"] != "hashedrekord"
                || included.body["spec"]["data"]["hash"]["algorithm"] != "sha256"
                || included.body["spec"]["data"]["hash"]["value"] != logged_digest(&release)?
            {
                return Err("the log entry names another research release".into());
            }
            release.transparency.push(att::Transparency {
                log: "rekor".into(),
                url: "https://rekor.sigstore.dev".into(),
                index: included.log_index,
            });
            let event = att::release_event(&signer, &release, now)?;
            let effective = now.checked_add(600).ok_or("invalid time")?;
            let head = att::head_event(
                &signer,
                &att::Head {
                    v: att::HEAD_V.into(),
                    requires: vec![],
                    workload: release.workload,
                    generation: now,
                    notice_seconds: 600,
                    admitted: vec![att::Admitted {
                        release: event.id.clone(),
                        effective_at: effective,
                        retire_at: None,
                    }],
                    emergency: None,
                },
                now,
            )?;
            let out = Path::new(&args[4]);
            fs::create_dir_all(out)?;
            write(&out.join("release.json"), &event)?;
            write(&out.join("head.json"), &head)?;
            write(
                &out.join("log-witness.json"),
                &json!({"index":included.log_index,"checkpoint_size":included.checkpoint.size,"effective_at":effective}),
            )?;
            println!(
                "{}",
                json!({"publisher":signer.pubkey(),"release":event.id,"head":head.id,"effective_at":effective})
            );
        }
        _ => {
            return Err(
                "use pubkey KEY; prepare KEY BUILD RECIPE BUILDERS OUT; finish KEY DRAFT ENTRY OUT"
                    .into(),
            );
        }
    }
    Ok(())
}
