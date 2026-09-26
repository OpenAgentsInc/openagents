//! Explicit file-based setup and one finite remote operation per invocation.
use coder_control::{Blobs, Host, Result, Setup, client, transport};
use nostr::domain::Event;
use secp256k1::SecretKey;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::fs::{File, OpenOptions};
use std::io::{Read, Write};
use std::os::unix::fs::{MetadataExt, OpenOptionsExt, PermissionsExt};
use std::path::Path;

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Packet {
    schema: String,
    authority: String,
    request_id: String,
    request: Event,
    input: Event,
    text: Option<client::TextDelivery>,
}
fn read(path: &Path) -> Result<Vec<u8>> {
    let file = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW)
        .open(path)
        .map_err(|e| e.to_string())?;
    let metadata = file.metadata().map_err(|e| e.to_string())?;
    if !metadata.is_file()
        || metadata.nlink() != 1
        || metadata.permissions().mode() & 0o077 != 0
        || metadata.len() > 4 * 1024 * 1024
    {
        return Err(
            "control inputs must be private, singly linked regular files under 4 MiB".into(),
        );
    }
    let mut bytes = vec![];
    file.take(4 * 1024 * 1024 + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() > 4 * 1024 * 1024 {
        return Err("control input grew beyond its byte bound".into());
    }
    Ok(bytes)
}
fn load<T: serde::de::DeserializeOwned>(path: &str) -> Result<T> {
    serde_json::from_value(
        nostr::contracts::parse_strict_bounded(&read(Path::new(path))?, 4 * 1024 * 1024)
            .map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())
}
fn write(path: &str, value: &impl Serialize) -> Result<()> {
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .custom_flags(libc::O_NOFOLLOW)
        .open(path)
        .map_err(|e| e.to_string())?;
    file.write_all(&serde_json::to_vec_pretty(value).map_err(|e| e.to_string())?)
        .and_then(|_| file.sync_all())
        .map_err(|e| e.to_string())?;
    if let Some(parent) = Path::new(path)
        .parent()
        .filter(|path| !path.as_os_str().is_empty())
    {
        File::open(parent)
            .and_then(|file| file.sync_all())
            .map_err(|e| e.to_string())?;
    }
    Ok(())
}
fn secret() -> Result<SecretKey> {
    let path = std::env::var_os("CODER_CONTROL_KEY_FILE")
        .ok_or("set CODER_CONTROL_KEY_FILE to a private 0600 hex-key file")?;
    let bytes = read(Path::new(&path))?;
    let text = std::str::from_utf8(&bytes)
        .map_err(|_| "invalid control key encoding")?
        .trim();
    SecretKey::from_str(text).map_err(|_| "invalid control secret key".into())
}
use std::str::FromStr;
fn unix_time() -> Result<u64> {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|v| v.as_secs())
        .map_err(|e| e.to_string())
}
fn usage() -> String {
    "coder-control init SETUP TASK_DIR TASK_ID OWNER_PUBKEY | client-config SETUP OUTPUT | invite SETUP HOST_DIR CLIENT_PUBKEY observe,steer,cancel OUTPUT | text CLIENT_CONFIG TEXT_FILE OUTPUT | prepare CLIENT_CONFIG ROLE BODY_JSON PACKET [TEXT_DELIVERY] | call RELAY PACKET RESULT | serve-once SETUP HOST_DIR RELAY".into()
}
#[tokio::main]
async fn main() {
    if let Err(error) = run().await {
        eprintln!("coder-control: {error}");
        std::process::exit(1);
    }
}
async fn run() -> Result<()> {
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    if args.is_empty() || matches!(args[0].as_str(), "help" | "--help" | "-h") {
        println!("{}", usage());
        return Ok(());
    }
    let now = unix_time()?;
    match args[0].as_str() {
        "publish" if args.len() == 3 => {
            let event: Event = load(&args[2])?;
            nostr_transport::artifacts::publish(&args[1], &secret()?, &event).await?;
        }
        "init" if args.len() == 5 => {
            let secret = secret()?;
            let setup = Setup::for_task(
                Path::new(&args[2]),
                &args[3],
                &args[4],
                &client::pubkey(&secret),
                now,
                now + 7 * 86400,
            )?;
            write(&args[1], &setup)?;
        }
        "client-config" if args.len() == 3 => {
            let setup: Setup = load(&args[1])?;
            write(&args[2], &setup.client_configuration())?;
        }
        "invite" if args.len() == 6 => {
            let setup: Setup = load(&args[1])?;
            let key = secret()?;
            let mut host = Host::open(Path::new(&args[2]), setup, key)?;
            let event = host.invite(
                &args[3],
                &args[4].split(',').map(str::to_owned).collect::<Vec<_>>(),
                now,
                now + 300,
                now + 3600,
            )?;
            write(&args[5], &event)?;
        }
        "text" if args.len() == 4 => {
            let config: client::Configuration = load(&args[1])?;
            let bytes = read(Path::new(&args[2]))?;
            let text = std::str::from_utf8(&bytes).map_err(|_| "instruction must be UTF-8")?;
            write(
                &args[3],
                &client::text(
                    text,
                    &secret()?,
                    &config.authority,
                    now,
                    config.retain_until,
                )?,
            )?;
        }
        "prepare" if (5..=6).contains(&args.len()) => {
            let config: client::Configuration = load(&args[1])?;
            let key = secret()?;
            let body: Value = load(&args[3])?;
            nostr::control::validate(&body).map_err(|e| e.to_string())?;
            let input = client::envelope(&body, &key, &config.authority, now, config.retain_until)?;
            let r = coder_control::reference(
                &nostr::contracts::jcs(&body).map_err(|e| e.to_string())?,
                "application/json",
                body["v"].as_str().ok_or("input schema")?,
            );
            let request_id = client::random_id();
            let request = client::request(
                &key,
                &config.authority,
                config
                    .operations
                    .get(&args[2])
                    .ok_or("unknown control role")?,
                &input,
                &r,
                now,
                now + 120,
                config.retain_until,
                &request_id,
            )?;
            let text = if args.len() == 6 {
                Some(load::<client::TextDelivery>(&args[5])?)
            } else {
                None
            };
            if let Some(text) = &text {
                text.verify(&body["payload"]["message"], &client::pubkey(&key), &key)?;
            }
            write(
                &args[4],
                &Packet {
                    schema: "openagents.control-packet.v1".into(),
                    authority: config.authority,
                    request_id,
                    request,
                    input,
                    text,
                },
            )?;
        }
        "call" if args.len() == 4 => {
            let key = secret()?;
            let packet: Packet = load(&args[2])?;
            if packet.schema != "openagents.control-packet.v1"
                || packet.request.pubkey != client::pubkey(&key)
                || packet.input.pubkey != client::pubkey(&key)
            {
                return Err("packet signer or version differs".into());
            }
            if let Some(text) = &packet.text {
                for event in [&text.declaration, &text.carrier] {
                    nostr_transport::artifacts::publish(&args[1], &key, event).await?;
                }
            }
            nostr_transport::artifacts::publish(&args[1], &key, &packet.input).await?;
            let event =
                transport::exchange(&args[1], &key, &packet.request, &packet.authority).await?;
            let result = client::result(
                &event,
                &packet.request,
                &packet.authority,
                &packet.request_id,
                &key,
            )?;
            let mut artifacts = vec![];
            for r in result["artifacts"]
                .as_array()
                .ok_or("result artifact list is missing")?
            {
                let event_id = r["event"]["id"]
                    .as_str()
                    .ok_or("result artifact lacks its original event")?;
                let event = nostr_transport::artifacts::fetch(&args[1], &key, event_id).await?;
                client::open_artifact(&event, &packet.authority, r, &key)?;
                artifacts.push(event);
            }
            write(
                &args[3],
                &json!({"schema":"openagents.control-response.v1","result":event,"payload":result,"artifacts":artifacts}),
            )?;
        }
        "serve-once" if args.len() == 4 => {
            let setup: Setup = load(&args[1])?;
            let key = secret()?;
            let mut host = Host::open(Path::new(&args[2]), setup, key)?;
            let mut receiver = transport::Receiver::connect(&args[3], &key).await?;
            eprintln!("control subscription ready");
            let request = receiver.receive().await?;
            let opened = nostr::execution::open_request(
                &request,
                &client::pubkey(&key),
                &key,
                unix_time()?,
                nostr::execution::Window::DEFAULT,
            );
            let opened = opened.map_err(|e| format!("{e:?}"))?;
            let nostr::execution::Body::Execute(execute) = opened.body else {
                return Err("control execution request required".into());
            };
            let reference = execute
                .input_artifact
                .ok_or("control input artifact missing")?;
            let input = nostr_transport::artifacts::fetch(
                &args[3],
                &key,
                &reference.event.ok_or("input declaring event missing")?.id,
            )
            .await?;
            let now = unix_time()?;
            let reply = if let Some(reference) = host.required_text(&request, &input, now)? {
                let text = client::fetch_text(&args[3], &key, &reference).await?;
                host.handle_text(&request, &input, &text, unix_time()?)?
            } else {
                host.handle(&request, &input, &Blobs::default(), now)?
            };
            for event in &reply.artifacts {
                nostr_transport::artifacts::publish(&args[3], &key, event).await?;
            }
            receiver.reply(&request, &reply).await?;
        }
        _ => return Err(usage()),
    }
    Ok(())
}
