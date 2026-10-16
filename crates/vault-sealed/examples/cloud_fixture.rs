//! One-shot synthetic vault run on Confidential Space, with public evidence only.
//! The client and workload share a process; KMS is an ephemeral fixture.
#[cfg(unix)]
mod cloud {
    use nostr::{
        att,
        contracts::digest_bytes,
        domain::{Event, RelaySigner},
    };
    use oa_vault::{Vmk, b64, sealed::KeyPair};
    use secp256k1::SecretKey;
    use serde_json::{Value, json};
    use std::{
        collections::BTreeMap,
        io::{Read, Write},
        os::unix::net::UnixStream,
        time::{Duration, SystemTime, UNIX_EPOCH},
    };
    use vault_sealed::{
        Signed,
        client::{Client, verify_endpoint},
        id_bytes,
        kms::LocalKms,
        workload::{SizeWork, Workload},
    };

    type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;
    const RECIPE: &[u8] = include_bytes!("../../../deploy/vault-sealed/research.Dockerfile");
    fn now() -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs()
    }
    fn secret() -> SecretKey {
        SecretKey::new(&mut secp256k1::rand::rng())
    }
    fn signer(key: &SecretKey) -> RelaySigner {
        RelaySigner::from_secret_hex(&key.display_secret().to_string()).unwrap()
    }
    fn id(bytes: &[u8]) -> String {
        digest_bytes(bytes).trim_start_matches("sha256:").into()
    }
    fn event(value: &Value) -> Result<Event> {
        Ok(serde_json::from_value(value.clone())?)
    }

    fn prepare(image: &str, commit: &str) -> Result<Value> {
        let (reference, digest) = image.split_once('@').ok_or("Use a digest-pinned image.")?;
        let publisher = signer(&secret());
        let timestamp = now();
        let release = att::Release {
            v: att::RELEASE_V.into(), requires: vec![], workload: "vault".into(),
            publisher: publisher.pubkey().into(),
            image: att::Image { reference: reference.into(), digest: digest.into() },
            platforms: vec![att::Platform { kind: oa_att::PLATFORM.into(), hwmodel: "GCP_INTEL_TDX".into(), support: "STABLE".into() }],
            measurements: vec![], gpu: None, models: vec![], components: vec![],
            source: att::Source { repo: "https://github.com/OpenAgentsInc/openagents".into(),
                commit: commit.into(), recipe: "deploy/vault-sealed/research.Dockerfile".into(),
                recipe_digest: digest_bytes(RECIPE) },
            rebuilds: vec![], transparency: vec![],
            changes: "Synthetic vault composition with an ephemeral local key service; the client runs in the same workload.".into(),
            published_at: timestamp,
        };
        let release = att::release_event(&publisher, &release, timestamp)?;
        let head = att::Head {
            v: att::HEAD_V.into(),
            requires: vec![],
            workload: "vault".into(),
            generation: 0,
            notice_seconds: 600,
            emergency: None,
            admitted: vec![att::Admitted {
                release: release.id.clone(),
                effective_at: timestamp + 600,
                retire_at: None,
            }],
        };
        Ok(json!({"publisher":publisher.pubkey(),"release":release,
            "head":att::head_event(&publisher,&head,timestamp)?}))
    }

    fn token(binding: &str) -> Result<String> {
        let body = json!({"audience":att::TOKEN_AUDIENCE,"token_type":"PKI","nonces":[binding]})
            .to_string();
        let mut socket = UnixStream::connect("/run/container_launcher/teeserver.sock")?;
        socket.set_read_timeout(Some(Duration::from_secs(40)))?;
        socket.set_write_timeout(Some(Duration::from_secs(10)))?;
        write!(
            socket,
            "POST /v1/token HTTP/1.1\r\nHost: localhost\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        )?;
        let mut response = Vec::new();
        socket.take(131073).read_to_end(&mut response)?;
        if response.len() > 131072 {
            return Err("The launcher response exceeds 128 KiB.".into());
        }
        let split = response
            .windows(4)
            .position(|p| p == b"\r\n\r\n")
            .ok_or("Invalid launcher response.")?;
        let head = std::str::from_utf8(&response[..split])?;
        if head
            .lines()
            .next()
            .and_then(|s| s.split_whitespace().nth(1))
            != Some("200")
        {
            return Err("The launcher refused the evidence request.".into());
        }
        let mut data = &response[split + 4..];
        let body = if head
            .lines()
            .any(|s| s.eq_ignore_ascii_case("transfer-encoding: chunked"))
        {
            let mut body = Vec::new();
            loop {
                let end = data
                    .windows(2)
                    .position(|p| p == b"\r\n")
                    .ok_or("Missing chunk size.")?;
                let count = usize::from_str_radix(
                    std::str::from_utf8(&data[..end])?
                        .split(';')
                        .next()
                        .unwrap_or(""),
                    16,
                )?;
                data = &data[end + 2..];
                if count == 0 {
                    break;
                }
                let end = count.checked_add(2).ok_or("Chunk overflow.")?;
                if data.get(count..end) != Some(b"\r\n") {
                    return Err("Invalid chunk terminator.".into());
                }
                body.extend_from_slice(data.get(..count).ok_or("Truncated chunk.")?);
                data = &data[end..];
            }
            body
        } else {
            data.to_vec()
        };
        let value = std::str::from_utf8(&body)?.trim();
        if value.split('.').count() != 3 {
            return Err("The launcher returned no evidence token.".into());
        }
        Ok(value.into())
    }

    fn run(config: Value) -> Result<Value> {
        let timestamp = now();
        let release = event(&config["release"])?;
        let head = event(&config["head"])?;
        let parsed_release = att::parse_release(&release)?;
        if Some(parsed_release.source.commit.as_str()) != option_env!("OA_VAULT_RESEARCH_COMMIT") {
            return Err("The release source differs from the compiled fixture.".into());
        }
        if parsed_release.workload != "vault"
            || parsed_release.publisher
                != config["publisher"].as_str().ok_or("Missing publisher.")?
        {
            return Err("The research release has another workload or publisher.".into());
        }
        att::parse_head(&head)?.admits(&release.id, &parsed_release, timestamp)?;
        let key = secret();
        let endpoint_signer = signer(&key);
        let session = KeyPair::generate()?;
        let binding = att::binding(
            endpoint_signer.pubkey(),
            Some(&session.public()),
            &release.id,
        )?;
        let token = token(&binding)?;
        let claims = oa_att::token::verify(&token, att::TOKEN_AUDIENCE, timestamp)
            .map_err(|error| error.0)?;
        let body = att::Endpoint {
            v: att::ENDPOINT_V.into(),
            requires: vec![],
            release: release.id.clone(),
            endpoint: endpoint_signer.pubkey().into(),
            hpke: Some(b64(&session.public())),
            binding,
            evidence: vec![att::Evidence {
                kind: "gcp-confidential-space-token".into(),
                format: "pki".into(),
                token,
            }],
            operator: parsed_release.publisher.clone(),
            issued_at: timestamp,
            valid_until: claims.exp.min(timestamp + 3600),
        };
        let endpoint = att::endpoint_event(
            &endpoint_signer,
            &body,
            &id(claims.instance_name.as_bytes()),
            &att::parse_head(&head)?.address(&parsed_release.publisher),
        )?;
        let records = oa_att::Records {
            release: release.clone(),
            head: head.clone(),
            endpoint: endpoint.clone(),
            beacon: None,
        };
        let verified = verify_endpoint(&records, &parsed_release.publisher, timestamp)?;
        let mut workload = Workload::new(
            key,
            session,
            id_bytes(&endpoint.id)?,
            &release.id,
            Box::new(LocalKms::new(oa_vault::random::<32>()?, &release.id)),
            Box::new(SizeWork),
            false,
        );
        let vault = id(b"synthetic-cloud-vault");
        let object = id(b"synthetic-object");
        let slot = id(b"synthetic-enrolled-slot");
        let run = id(b"synthetic-run");
        let slot_key = secret();
        let mut client = Client::new(Vmk::from_bytes(oa_vault::random::<32>()?), &slot, slot_key);
        let certificate = workload.system_key(&vault)?;
        client.accept_system_key(&verified, &certificate, timestamp)?;
        let sealed =
            client.seal_object(&vault, &object, b"synthetic vault file", true, timestamp)?;
        let effects = json!({"read":true,"write_derived":false,"export":false});
        let lease = client.lease(
            &run,
            &[&object],
            "count bytes",
            "private",
            effects.clone(),
            300,
            1,
            timestamp,
        )?;
        let enrolled = BTreeMap::from([(slot, client.slot_pubkey())]);
        let objects = vec![(object.clone(), sealed.clone())];
        let (enc, request, pending) = client.request(&verified, &vault, &lease, timestamp)?;
        let replies = workload.serve(&enc, &request, &objects, &enrolled, timestamp)?;
        let answers = client.open_replies(&pending, &replies)?;
        if answers.len() != 1
            || serde_json::from_slice::<Value>(&answers[0])?["bytes"] != json!([20])
        {
            return Err("The synthetic answer has the wrong size.".into());
        }
        if client.open_on_device(&sealed)?.as_slice() != b"synthetic vault file" {
            return Err("The device copy differs.".into());
        }
        let replay = workload
            .serve(&enc, &request, &objects, &enrolled, timestamp)
            .unwrap_err();
        if !replay.starts_with("conflict") {
            return Err("The one-use lease was not enforced.".into());
        }
        let revoked = client.lease(
            &run,
            &[&object],
            "count bytes",
            "private",
            effects.clone(),
            300,
            1,
            timestamp,
        )?;
        workload.revoke(
            &client.revoke(revoked.body["lease"].as_str().ok_or("Missing lease.")?)?,
            &enrolled,
        )?;
        let (enc2, request2, _) = client.request(&verified, &vault, &revoked, timestamp)?;
        if workload
            .serve(&enc2, &request2, &objects, &enrolled, timestamp)
            .is_ok()
        {
            return Err("A revoked lease opened a file.".into());
        }
        let mut substituted = body;
        substituted.hpke = Some(b64(&KeyPair::generate()?.public()));
        substituted.binding = att::binding(
            endpoint_signer.pubkey(),
            Some(&oa_vault::unb64_n::<32>(
                substituted.hpke.as_ref().unwrap(),
            )?),
            &release.id,
        )?;
        let mut altered = records.clone();
        altered.endpoint = att::endpoint_event(
            &endpoint_signer,
            &substituted,
            &id(claims.instance_name.as_bytes()),
            &att::parse_head(&head)?.address(&parsed_release.publisher),
        )?;
        if verify_endpoint(&altered, &parsed_release.publisher, timestamp).is_ok() {
            return Err("An unbound HPKE key was accepted.".into());
        }
        if verify_endpoint(&records, &parsed_release.publisher, verified.valid_until).is_ok() {
            return Err("Expired evidence was accepted.".into());
        }
        let public = json!({"v":"openagents.vault-cloud-composition.v1","requires":[],
            "captured_at":timestamp,"release":release.id,"endpoint":endpoint.id,"image_digest":claims.image_digest,
            "source_commit":parsed_release.source.commit,"level":verified.level,
            "object_sha256":digest_bytes(&sealed),"request_sha256":digest_bytes(&request),
            "reply_sha256":digest_bytes(&replies[0].1),"answer_bytes":[20],
            "one_use_refused":true,"revoked_refused":true,"unbound_hpke_refused":true,"expired_refused":true,
            "client_location":"same-workload","kms":"ephemeral-local-fixture",
            "production_qualification":false,"independent_rebuild":false});
        let completion = Signed::sign(public, &key)?;
        Ok(
            json!({"schema":"openagents.vault-cloud-fixture.v1","publisher":parsed_release.publisher,
            "release":release,"head":head,"endpoint":endpoint,"certificate":certificate,
            "object":b64(&sealed),"request":b64(&request),"reply":b64(&replies[0].1),
            "audit":workload.log(),"completion":completion}),
        )
    }

    pub fn main() -> Result<()> {
        let args: Vec<_> = std::env::args().skip(1).collect();
        let result = match args.as_slice() {
            [mode, image, commit] if mode == "prepare" => prepare(image, commit)?,
            [mode] if mode == "run" => run(serde_json::from_str(&std::env::var(
                "OA_VAULT_RESEARCH_RECORDS",
            )?)?)?,
            _ => {
                return Err(
                    "Use prepare IMAGE COMMIT, or run inside the research workload.".into(),
                );
            }
        };
        println!("{}", serde_json::to_string(&result)?);
        Ok(())
    }
}

#[cfg(unix)]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    cloud::main()
}
#[cfg(not(unix))]
fn main() {
    eprintln!("This research fixture requires a Linux Confidential Space guest.");
    std::process::exit(1);
}
