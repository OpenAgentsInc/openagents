//! `openagents vault`: the person's "only you" vault (NIP-VAULT tier
//! `user`, #11240) from a terminal.
//!
//! Files are sealed here with `oa_vault` before they leave the computer;
//! the service (openagents.com) stores ciphertext, sealed key slots and a
//! sealed file list. This computer opens the vault with its device key,
//! kept in the OS keychain ([`keys`]). A new computer gets in with the
//! recovery code, the person's Nostr key, or a pairing link from a
//! computer that is already in, and then adds its own device key.
//!
//! Questions about files are answered either on this device by a local
//! Psionic server ([`local`]), or, only when the person passes
//! `--route fast`, by Google Gemini through the service. One question goes
//! to one route; nothing falls back to another.

mod api;
mod keys;
mod local;
#[cfg(test)]
mod tests;

use std::collections::BTreeMap;
use std::io::{BufRead, IsTerminal, Read, Write as _};
use std::path::{Path, PathBuf};

use coder::cli_route::tree::{Declared, Effect};
use oa_vault::index::{Add, Route};
use oa_vault::{Entry, Index, Kind, Method, Slot, Vmk, slot};
use serde_json::{Value, json};
use zeroize::Zeroizing;

use crate::{Args, Output};
use api::{Plain, Service, State, Write};
use keys::{DeviceKeys, NostrKey};
use local::LocalModel;

pub(crate) const USAGE: &str = "usage: openagents vault COMMAND
  setup       Make your vault: this computer can open it, and you get a
              24-word recovery code to write down.
  unlock [--recovery | --nostr [--nostr-key FILE] | --pair LINK]
              Open your vault on this computer. A computer that's new to it
              gets in with your recovery code, your Nostr key, or a link from
              one of your computers, and then remembers it.
  add-device  Make a link that lets another computer or browser in (works
              for 10 minutes).
  add-nostr [--nostr-key FILE]
              Let your Nostr key open your vault too.
  devices     What can open your vault.
  remove-device ID
              Stop a computer, key or code from opening your vault.
  add FILE [--project ID]
              Put a file in your vault.
  list [--project ID]
              Your files and saved answers.
  get FILE [--out PATH]
              Save a file from your vault here (only you can read the copy).
  rm FILE     Delete a file from your vault for good.
  ask FILE... --question TEXT [--route device|fast] [--psionic URL] [--project ID]
              Ask about your files. device: a model on this computer reads
              them (the default when one is running). fast: Google Gemini
              reads them, and Google sees them. The answer is saved in your vault.
  serve-local [--model PATH] [--port N] [--allow-origin ORIGIN] [--print]
              Run the model on this computer so your vault page in the browser
              can ask it too.
FILE is a file's name or the start of its id. Signs in as Coder does
(coder login), or with --account-dir DIR. See docs/security/sensitive-data-vault.md.";

/// What each command does, for the chat router's command tree.
pub(crate) const EFFECTS: &[Declared] = &[
    Declared::computer("setup", Effect::Secret),
    Declared::computer("unlock", Effect::Secret),
    Declared::computer("add-device", Effect::Grants),
    Declared::computer("add-nostr", Effect::Grants),
    Declared::computer("devices", Effect::ReadOnly),
    Declared::computer("remove-device", Effect::Grants),
    Declared::computer("add", Effect::Publishes),
    Declared::computer("list", Effect::ReadOnly),
    Declared::computer("get", Effect::LocalWrite),
    Declared::computer("rm", Effect::Publishes),
    Declared::computer("ask", Effect::Publishes),
    Declared::computer("serve-local", Effect::LongRunning),
];

/// A pairing link works this long.
const PAIRING_SECS: u64 = 600;
/// The service refuses objects over 11 MiB; leave room for the header and tags.
const MAX_FILE: u64 = 11 * 1024 * 1024 - 64 * 1024;

pub fn run(output: &Output, words: &[String]) -> u8 {
    if words.is_empty() || matches!(words[0].as_str(), "--help" | "-h" | "help") {
        println!("{USAGE}");
        return if words.is_empty() {
            crate::EXIT_USAGE
        } else {
            0
        };
    }
    // `-o PATH` is `--out PATH`.
    let words: Vec<String> = words
        .iter()
        .map(|word| {
            if word == "-o" {
                "--out".to_owned()
            } else {
                word.clone()
            }
        })
        .collect();
    let args = match Args::parse(&words, &["recovery", "nostr", "print"]) {
        Ok(args) => args,
        Err(message) => return output.usage("vault", &message, USAGE),
    };
    let positional = args.positional();
    let command = positional.first().map(String::as_str).unwrap_or_default();
    let known: &[&str] = match command {
        "setup" | "add-device" | "devices" | "remove-device" | "rm" => &["account-dir"],
        "unlock" => &["pair", "nostr-key", "account-dir"],
        "add-nostr" => &["nostr-key", "account-dir"],
        "add" | "list" => &["project", "account-dir"],
        "get" => &["out", "account-dir"],
        "ask" => &["question", "route", "psionic", "project", "account-dir"],
        "serve-local" => &["model", "port", "allow-origin", "account-dir"],
        _ => return output.usage("vault", &format!("unknown command `{command}`"), USAGE),
    };
    if let Some(name) = args
        .option_names()
        .into_iter()
        .find(|name| !known.contains(name))
    {
        return output.usage("vault", &format!("unknown option `--{name}`"), USAGE);
    }
    let result = if command == "serve-local" {
        serve_local(output, &args)
    } else {
        live(output, command, &args)
    };
    match result {
        Ok(value) => {
            output.emit(&value, |value| render(command, value));
            0
        }
        Err(why) => output.fail(&format!("vault {command}"), &why),
    }
}

fn home() -> PathBuf {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."))
}

fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |since| since.as_secs())
}

/// This computer's name, as a slot label.
fn label() -> String {
    let name: String = openagents_login::computer_name()
        .chars()
        .filter(|c| !c.is_control())
        .take(slot::MAX_LABEL)
        .collect();
    if name.trim().is_empty() {
        "This computer".to_owned()
    } else {
        name.trim().to_owned()
    }
}

/// Every command but `serve-local`, against the signed-in service.
fn live(output: &Output, command: &str, args: &Args) -> Result<Value, String> {
    let saved = crate::mac::saved(args.option("account-dir"))?;
    let service = api::Http::new(saved)?;
    let keychain = keys::Keychain::open()?;
    let vault = Vault {
        service: &service,
        keys: &keychain,
        seen: home().join(".openagents/vault/seen.json"),
        label: label(),
        now: now(),
    };
    let positional = args.positional();
    let target = |what: &str| {
        positional
            .get(1)
            .map(String::as_str)
            .ok_or_else(|| format!("{command} takes {what}"))
    };
    let nostr = |args: &Args| keys::nostr_key(args.option("nostr-key").map(Path::new), &home());
    match command {
        "setup" => {
            let made = vault.setup()?;
            Ok(json!({ "vault": made.vault, "recovery_code": made.words.as_str() }))
        }
        "unlock" => {
            let how = if args.switch("recovery") {
                let words = read_recovery(output)?;
                vault.unlock(Unlock::Recovery(words.as_str()))?
            } else if args.switch("nostr") {
                vault.unlock(Unlock::Nostr(&nostr(args)?))?
            } else if let Some(link) = args.option("pair") {
                vault.unlock(Unlock::Pair(link))?
            } else {
                vault.unlock(Unlock::Device)?
            };
            Ok(json!({ "files": how.files, "remembered": how.remembered }))
        }
        "add-device" => {
            let link = vault.add_pairing()?;
            Ok(json!({ "link": link.as_str(), "minutes": PAIRING_SECS / 60 }))
        }
        "add-nostr" => {
            let key = nostr(args)?;
            vault.add_nostr(&key)?;
            Ok(json!({ "pubkey": key.pubkey(), "from": key.from }))
        }
        "devices" => Ok(json!({ "devices": vault.devices()? })),
        "remove-device" => {
            let removed = vault.remove_device(target("the id of what to remove")?)?;
            Ok(json!({ "removed": removed }))
        }
        "add" => {
            let file = PathBuf::from(target("a file")?);
            let entry = vault.add_file(&file, args.option("project"))?;
            Ok(entry_json(&entry))
        }
        "list" => {
            let entries = vault.list(args.option("project"))?;
            Ok(json!({ "files": entries.iter().map(entry_json).collect::<Vec<_>>() }))
        }
        "get" => {
            let (entry, plain) = vault.get(target("a file")?)?;
            let path = save(&entry, &plain, args.option("out").map(Path::new))?;
            Ok(
                json!({ "object": entry.object, "name": entry.name, "path": path.display().to_string(), "size": entry.size }),
            )
        }
        "rm" => {
            let entry = vault.remove(target("a file")?)?;
            Ok(json!({ "removed": entry.object, "name": entry.name }))
        }
        "ask" => {
            let files: Vec<&str> = positional.iter().skip(1).map(String::as_str).collect();
            if files.is_empty() {
                return Err("ask takes one or more files".into());
            }
            let question = args
                .option("question")
                .filter(|q| !q.trim().is_empty())
                .ok_or("ask takes --question TEXT")?;
            let route = match args.option("route") {
                None => None,
                Some("device") => Some(Route::Device),
                Some("fast") => Some(Route::Fast),
                Some(other) => return Err(format!("--route is device or fast, not `{other}`")),
            };
            let model = local::Psionic::from(args.option("psionic"));
            let asked = vault.ask(&files, question, route, &model, args.option("project"))?;
            Ok(json!({
                "route": match asked.route { Route::Device => "device", Route::Fast => "fast", Route::Private => "private" },
                "model": asked.model,
                "answer": asked.answer.as_str(),
                "object": asked.entry.object,
            }))
        }
        _ => unreachable!("checked in run"),
    }
}

/// The recovery code, typed on the terminal without echo, or read from
/// standard input.
fn read_recovery(output: &Output) -> Result<Zeroizing<String>, String> {
    let mut words = Zeroizing::new(String::new());
    if std::io::stdin().is_terminal() {
        if !output.json() {
            eprint!("Type your 24-word recovery code: ");
            let _ = std::io::stderr().flush();
        }
        let _quiet = crate::provider_key::NoEcho::enter();
        std::io::stdin()
            .lock()
            .read_line(&mut words)
            .map_err(|_| "The recovery code couldn't be read.".to_owned())?;
    } else {
        std::io::stdin()
            .read_to_string(&mut words)
            .map_err(|_| "The recovery code couldn't be read.".to_owned())?;
    }
    Ok(words)
}

/// Write a file's plaintext to `out`, or to its name here, readable only
/// by this user.
fn save(entry: &Entry, plain: &[u8], out: Option<&Path>) -> Result<PathBuf, String> {
    let (path, fresh) = match out {
        Some(path) => (path.to_path_buf(), false),
        None => {
            let name = Path::new(&entry.name)
                .file_name()
                .map_or_else(|| "file".into(), |name| name.to_os_string());
            (PathBuf::from(name), true)
        }
    };
    let mut options = std::fs::OpenOptions::new();
    options.write(true);
    if fresh {
        options.create_new(true);
    } else {
        options.create(true).truncate(true);
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options.open(&path).map_err(|error| {
        if error.kind() == std::io::ErrorKind::AlreadyExists {
            format!(
                "{} already exists here. Pass --out PATH to save it somewhere else.",
                path.display()
            )
        } else {
            format!("{}: {error}", path.display())
        }
    })?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600));
    }
    file.write_all(plain)
        .map_err(|error| format!("{}: {error}", path.display()))?;
    Ok(path)
}

fn entry_json(entry: &Entry) -> Value {
    json!({
        "object": entry.object,
        "kind": match entry.kind { Kind::File => "file", Kind::Answer => "answer" },
        "name": entry.name,
        "media": entry.media,
        "size": entry.size,
        "project": entry.project,
        "about": entry.about,
        "route": entry.route.map(|route| match route { Route::Device => "device", Route::Fast => "fast", Route::Private => "private" }),
        "created_at": entry.created_at,
    })
}

fn short(id: &str) -> &str {
    &id[..id.len().min(8)]
}

fn render(command: &str, value: &Value) -> String {
    let text = |key: &str| value[key].as_str().unwrap_or_default().to_owned();
    match command {
        "setup" => format!(
            "Your vault is ready, and this computer can open it.\n\nYour recovery code:\n\n  {}\n\nWrite these 24 words down and keep them somewhere safe. They are the only way\nback in if you lose your computers, and they won't be shown again.",
            text("recovery_code")
        ),
        "unlock" => {
            let files = value["files"].as_u64().unwrap_or(0);
            let mut line = format!(
                "Your vault is open on this computer ({files} file{}).",
                if files == 1 { "" } else { "s" }
            );
            if value["remembered"].as_bool() == Some(true) {
                line.push_str(" This computer will open it by itself from now on.");
            }
            line
        }
        "add-device" => format!(
            "Open this link on the other computer or browser within {} minutes:\n\n  {}\n\nOn a computer with this program: openagents vault unlock --pair LINK\nAnyone with the link can open your vault until it's used, so share it only with yourself.",
            value["minutes"].as_u64().unwrap_or(10),
            text("link")
        ),
        "add-nostr" => format!(
            "Your Nostr key {} can open your vault now (from {}).",
            short(&text("pubkey")),
            text("from")
        ),
        "devices" => {
            let rows: Vec<Vec<String>> = std::iter::once(vec![
                "ID".into(),
                "NAME".into(),
                "OPENS WITH".into(),
                "ADDED".into(),
            ])
            .chain(value["devices"].as_array().into_iter().flatten().map(|d| {
                let mut name = d["label"].as_str().unwrap_or_default().to_owned();
                if d["this_computer"].as_bool() == Some(true) {
                    name.push_str(" (this computer)");
                }
                vec![
                    short(d["slot"].as_str().unwrap_or_default()).to_owned(),
                    name,
                    d["opens_with"].as_str().unwrap_or_default().to_owned(),
                    crate::out::date(d["created_at"].as_u64().unwrap_or(0)),
                ]
            }))
            .collect();
            crate::out::table(&rows)
        }
        "remove-device" => format!(
            "Removed. {} can't open your vault any more.",
            text("removed")
        ),
        "add" => format!(
            "Added {} to your vault ({}).",
            text("name"),
            short(&text("object"))
        ),
        "list" => {
            let files = value["files"].as_array().cloned().unwrap_or_default();
            if files.is_empty() {
                return "Your vault is empty.".into();
            }
            let rows: Vec<Vec<String>> = std::iter::once(vec![
                "ID".into(),
                "NAME".into(),
                "SIZE".into(),
                "ADDED".into(),
            ])
            .chain(files.iter().map(|f| {
                vec![
                    short(f["object"].as_str().unwrap_or_default()).to_owned(),
                    f["name"].as_str().unwrap_or_default().to_owned(),
                    size(f["size"].as_u64().unwrap_or(0)),
                    crate::out::date(f["created_at"].as_u64().unwrap_or(0)),
                ]
            }))
            .collect();
            crate::out::table(&rows)
        }
        "get" => format!("Saved {} to {}.", text("name"), text("path")),
        "rm" => format!("Deleted {} from your vault.", text("name")),
        "ask" => {
            let by = if text("route") == "fast" {
                "Answered by Google Gemini. Google saw these files.".to_owned()
            } else {
                format!("Answered on this device ({}).", text("model"))
            };
            format!(
                "{by}\n\n{}\n\nSaved in your vault ({}).",
                text("answer"),
                short(&text("object"))
            )
        }
        "serve-local" => text("message"),
        _ => value.to_string(),
    }
}

fn size(bytes: u64) -> String {
    match bytes {
        0..1024 => format!("{bytes} B"),
        1024..1_048_576 => format!("{} KB", bytes / 1024),
        _ => format!("{:.1} MB", bytes as f64 / 1_048_576.0),
    }
}

/// A file's media type from its extension.
fn media_of(path: &Path) -> &'static str {
    let ext = path
        .extension()
        .and_then(|ext| ext.to_str())
        .map(str::to_ascii_lowercase)
        .unwrap_or_default();
    match ext.as_str() {
        "txt" | "log" => "text/plain",
        "md" | "markdown" => "text/markdown",
        "csv" => "text/csv",
        "json" => "application/json",
        "html" | "htm" => "text/html",
        "pdf" => "application/pdf",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "heic" => "image/heic",
        "docx" => "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
        "xlsx" => "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
        _ => "application/octet-stream",
    }
}

/// `serve-local`: start (or with `--print`, show) the local model server
/// that answers this computer's and the browser vault's questions.
fn serve_local(output: &Output, args: &Args) -> Result<Value, String> {
    let home = home();
    let port: u16 = args.number("port", 8080)?;
    let origin = args.option("allow-origin").map_or_else(
        || openagents_login::origin_from(|name| std::env::var(name).ok()),
        str::to_owned,
    );
    let model = args
        .option("model")
        .map(PathBuf::from)
        .or_else(|| local::find_model(&home));
    let name = local::SERVER;
    let server = local::find_server(&home, name);
    let (Some(server), Some(model)) = (server, model) else {
        let build = format!(
            "cargo build --release --manifest-path crates/psionic/Cargo.toml -p psionic-serve --bin {name}"
        );
        let message = format!(
            "No local model server here yet. From an OpenAgents checkout, build it once with:\n\n  {build}\n\nthen put target/release/{name} on your PATH (or in ~/.openagents/bin), and run this again{}.",
            if args.option("model").is_none() && local::find_model(&home).is_none() {
                " with --model PATH to a .gguf model"
            } else {
                ""
            }
        );
        return Ok(json!({ "running": false, "build": build, "message": message }));
    };
    let command = local::serve_command(&server, &model, port, &origin);
    let line = command.join(" ");
    if args.switch("print") {
        return Ok(json!({ "running": false, "command": command, "message": line }));
    }
    if !output.json() {
        eprintln!("Starting the model on this computer for {origin}:\n  {line}");
    }
    let status = std::process::Command::new(&command[0])
        .args(&command[1..])
        .status()
        .map_err(|error| format!("{}: {error}", command[0]))?;
    if status.success() {
        Ok(
            json!({ "running": false, "command": command, "message": "The model on this computer stopped." }),
        )
    } else {
        Err(format!("The model server stopped ({status})."))
    }
}

/// How to get into the vault on this computer.
pub(crate) enum Unlock<'a> {
    Device,
    Recovery(&'a str),
    Nostr(&'a NostrKey),
    Pair(&'a str),
}

pub(crate) struct Unlocked {
    pub files: usize,
    /// This computer added its own device key.
    pub remembered: bool,
}

pub(crate) struct Made {
    pub vault: String,
    pub words: Zeroizing<String>,
}

pub(crate) struct Asked {
    pub route: Route,
    pub model: String,
    pub answer: Zeroizing<String>,
    pub entry: Entry,
}

/// The vault open in memory.
struct Open {
    state: State,
    vmk: Vmk,
    index: Index,
}

/// The vault operations, over a service, a key store, and a file that
/// remembers the newest file list this computer has seen.
pub(crate) struct Vault<'a> {
    pub service: &'a dyn Service,
    pub keys: &'a dyn DeviceKeys,
    pub seen: PathBuf,
    pub label: String,
    pub now: u64,
}

impl Vault<'_> {
    fn state(&self) -> Result<State, String> {
        self.service.state()?.ok_or_else(|| {
            "You don't have a vault yet. Make one with: openagents vault setup".to_owned()
        })
    }

    pub(crate) fn setup(&self) -> Result<Made, String> {
        if self.service.state()?.is_some() {
            return Err(
                "You already have a vault. Open it here with: openagents vault unlock".into(),
            );
        }
        let vmk = Vmk::generate().map_err(err)?;
        let vault = oa_vault::new_id().map_err(err)?;
        let key_ref = keys::key_ref(&vault);
        let device_secret = Zeroizing::new(oa_vault::random::<32>().map_err(err)?);
        let device = Slot::seal(
            slot::Draft::new(
                &vault,
                Method::Device,
                slot::device_params(keys::platform(), &key_ref),
                &self.label,
                self.now,
            )
            .map_err(err)?,
            device_secret.as_ref(),
            &vmk,
        )
        .map_err(err)?;
        let code = oa_vault::recovery::Code::generate().map_err(err)?;
        let params = oa_vault::recovery::params().map_err(err)?;
        let recovery_secret = code.secret_for(&params).map_err(err)?;
        let recovery = Slot::seal(
            slot::Draft::new(&vault, Method::Recovery, params, "Recovery code", self.now)
                .map_err(err)?,
            recovery_secret.as_ref(),
            &vmk,
        )
        .map_err(err)?;
        let index = Index::new(&vault).map_err(err)?;
        let blob = index.seal(&vmk).map_err(err)?;
        self.keys.store(&key_ref, &device_secret)?;
        if let Err(why) = self.service.create(&vault, &[device, recovery], &blob) {
            let _ = self.keys.delete(&key_ref);
            return Err(why);
        }
        self.remember(&vault, index.epoch)?;
        Ok(Made {
            vault,
            words: Zeroizing::new(code.words().to_owned()),
        })
    }

    /// Open the vault with this computer's device key.
    fn open(&self) -> Result<Open, String> {
        let state = self.state()?;
        let vmk = self.device_vmk(&state)?.ok_or(
            "This computer can't open your vault yet. Get in once with: openagents vault unlock --recovery (or --pair LINK from another computer, or --nostr)",
        )?;
        self.with_index(state, vmk)
    }

    /// The master key from this computer's device key, if one of the
    /// vault's device slots is this computer's.
    fn device_vmk(&self, state: &State) -> Result<Option<Vmk>, String> {
        let key_ref = keys::key_ref(&state.id);
        let Some(secret) = self.keys.load(&key_ref)? else {
            return Ok(None);
        };
        Ok(state
            .slots
            .iter()
            .filter(|s| s.method == Method::Device && s.param("key_ref") == Some(&key_ref))
            .find_map(|s| s.open(secret.as_ref()).ok()))
    }

    /// The id of this computer's device slot.
    fn device_slot(&self, state: &State) -> Result<Option<String>, String> {
        let key_ref = keys::key_ref(&state.id);
        let Some(secret) = self.keys.load(&key_ref)? else {
            return Ok(None);
        };
        Ok(state
            .slots
            .iter()
            .filter(|s| s.method == Method::Device && s.param("key_ref") == Some(&key_ref))
            .find(|s| s.open(secret.as_ref()).is_ok())
            .map(|s| s.slot.clone()))
    }

    fn with_index(&self, state: State, vmk: Vmk) -> Result<Open, String> {
        let index = self.open_index(&state, &vmk)?;
        Ok(Open { state, vmk, index })
    }

    fn open_index(&self, state: &State, vmk: &Vmk) -> Result<Index, String> {
        let claimed = Index::epoch_of(&state.blob).map_err(err)?;
        if claimed != state.epoch {
            return Err(
                "Your vault's file list doesn't match what the service says. Try again.".into(),
            );
        }
        if claimed < self.seen(&state.id) {
            return Err("The service sent an older list of your files than this computer has already seen, so this program won't use it. Try again in a minute.".into());
        }
        let index = Index::open(vmk, &state.id, &state.blob).map_err(err)?;
        self.remember(&state.id, index.epoch)?;
        Ok(index)
    }

    /// The newest file list version this computer has seen for `vault`.
    fn seen(&self, vault: &str) -> u32 {
        std::fs::read(&self.seen)
            .ok()
            .and_then(|bytes| serde_json::from_slice::<BTreeMap<String, u32>>(&bytes).ok())
            .and_then(|seen| seen.get(vault).copied())
            .unwrap_or(0)
    }

    fn remember(&self, vault: &str, epoch: u32) -> Result<(), String> {
        let mut seen: BTreeMap<String, u32> = std::fs::read(&self.seen)
            .ok()
            .and_then(|bytes| serde_json::from_slice(&bytes).ok())
            .unwrap_or_default();
        if seen.get(vault).is_some_and(|at| *at >= epoch) {
            return Ok(());
        }
        seen.insert(vault.to_owned(), epoch);
        let bytes = serde_json::to_vec(&seen).map_err(|error| error.to_string())?;
        write_private(&self.seen, &bytes)
    }

    pub(crate) fn unlock(&self, how: Unlock<'_>) -> Result<Unlocked, String> {
        let state = self.state()?;
        if let Some(vmk) = self.device_vmk(&state)? {
            let open = self.with_index(state, vmk)?;
            return Ok(Unlocked {
                files: open.index.entries.len(),
                remembered: false,
            });
        }
        let (vmk, pairing) = match how {
            Unlock::Device => {
                return Err("This computer can't open your vault yet. Get in once with: openagents vault unlock --recovery (or --pair LINK from another computer, or --nostr)".into());
            }
            Unlock::Recovery(words) => {
                let code = oa_vault::recovery::Code::parse(words).map_err(err)?;
                let vmk = state
                    .slots
                    .iter()
                    .filter(|s| s.method == Method::Recovery)
                    .find_map(|s| {
                        let secret = code.secret_for(&s.params).ok()?;
                        s.open(secret.as_ref()).ok()
                    })
                    .ok_or("That recovery code doesn't open your vault.")?;
                (vmk, None)
            }
            Unlock::Nostr(key) => {
                let pubkey = key.pubkey();
                let vmk = state
                    .slots
                    .iter()
                    .filter(|s| s.method == Method::Nostr && s.param("pubkey") == Some(&pubkey))
                    .find_map(|s| {
                        let secret = keys::open_from_self(key, s.param("sealed")?).ok()?;
                        s.open(secret.as_ref()).ok()
                    })
                    .ok_or("Your Nostr key can't open this vault. Add it from a computer that's already in with: openagents vault add-nostr")?;
                (vmk, None)
            }
            Unlock::Pair(link) => {
                let (slot_id, secret) = parse_link(link)?;
                let slot = state
                    .slots
                    .iter()
                    .find(|s| s.slot == slot_id && s.method == Method::Pairing)
                    .ok_or("That link has been used or has expired. Make a new one with: openagents vault add-device")?;
                if slot.expired(self.now) {
                    return Err(
                        "That link has expired. Make a new one with: openagents vault add-device"
                            .into(),
                    );
                }
                let vmk = slot
                    .open(secret.as_ref())
                    .map_err(|_| "That link doesn't open your vault.".to_owned())?;
                (vmk, Some(slot_id))
            }
        };
        let open = self.with_index(state, vmk)?;
        self.remember_device(&open)?;
        if let Some(pairing) = pairing {
            // The link is used up. Its own expiry deletes it otherwise.
            let _ = self.service.delete_slot(&pairing);
        }
        Ok(Unlocked {
            files: open.index.entries.len(),
            remembered: true,
        })
    }

    /// Give this computer its own device slot.
    fn remember_device(&self, open: &Open) -> Result<(), String> {
        let key_ref = keys::key_ref(&open.state.id);
        let secret = match self.keys.load(&key_ref)? {
            Some(secret) => secret,
            None => {
                let secret = Zeroizing::new(oa_vault::random::<32>().map_err(err)?);
                self.keys.store(&key_ref, &secret)?;
                secret
            }
        };
        let slot = Slot::seal(
            slot::Draft::new(
                &open.state.id,
                Method::Device,
                slot::device_params(keys::platform(), &key_ref),
                &self.label,
                self.now,
            )
            .map_err(err)?,
            secret.as_ref(),
            &open.vmk,
        )
        .map_err(err)?;
        self.service.add_slot(&slot)
    }

    /// A pairing link for another computer or browser.
    pub(crate) fn add_pairing(&self) -> Result<Zeroizing<String>, String> {
        let open = self.open()?;
        let secret = Zeroizing::new(oa_vault::random::<32>().map_err(err)?);
        let slot = Slot::seal(
            slot::Draft::new(
                &open.state.id,
                Method::Pairing,
                slot::pairing_params(self.now + PAIRING_SECS),
                "Link",
                self.now,
            )
            .map_err(err)?,
            secret.as_ref(),
            &open.vmk,
        )
        .map_err(err)?;
        self.service.add_slot(&slot)?;
        use base64::Engine;
        let encoded =
            Zeroizing::new(base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(&secret[..]));
        Ok(Zeroizing::new(format!(
            "{}/settings/vault#pair={}.{}",
            self.service.origin().trim_end_matches('/'),
            slot.slot,
            encoded.as_str()
        )))
    }

    pub(crate) fn add_nostr(&self, key: &NostrKey) -> Result<(), String> {
        let open = self.open()?;
        let pubkey = key.pubkey();
        if open
            .state
            .slots
            .iter()
            .any(|s| s.method == Method::Nostr && s.param("pubkey") == Some(&pubkey))
        {
            return Err("This Nostr key can already open your vault.".into());
        }
        let secret = Zeroizing::new(oa_vault::random::<32>().map_err(err)?);
        let sealed = keys::seal_to_self(key, &secret)?;
        let slot = Slot::seal(
            slot::Draft::new(
                &open.state.id,
                Method::Nostr,
                slot::nostr_params(&pubkey, &sealed),
                "Nostr key",
                self.now,
            )
            .map_err(err)?,
            secret.as_ref(),
            &open.vmk,
        )
        .map_err(err)?;
        self.service.add_slot(&slot)
    }

    pub(crate) fn devices(&self) -> Result<Vec<Value>, String> {
        let state = self.state()?;
        let mine = self.device_slot(&state)?;
        Ok(state
            .slots
            .iter()
            .filter(|s| !s.expired(self.now))
            .map(|s| {
                json!({
                    "slot": s.slot,
                    "label": s.label,
                    "method": s.method.as_str(),
                    "opens_with": opens_with(s),
                    "created_at": s.created_at,
                    "this_computer": mine.as_deref() == Some(s.slot.as_str()),
                })
            })
            .collect())
    }

    pub(crate) fn remove_device(&self, id: &str) -> Result<String, String> {
        let state = self.state()?;
        let matches: Vec<&Slot> = state
            .slots
            .iter()
            .filter(|s| s.slot.starts_with(id) || s.label == id)
            .collect();
        let slot = match matches.as_slice() {
            [one] if !id.is_empty() => *one,
            [] => {
                return Err(format!(
                    "Nothing that opens your vault matches {id}. See: openagents vault devices"
                ));
            }
            _ => {
                return Err(format!(
                    "More than one thing matches {id}; give more of its id."
                ));
            }
        };
        let rest: Vec<Slot> = state
            .slots
            .iter()
            .filter(|s| s.slot != slot.slot)
            .cloned()
            .collect();
        if !slot::enough(&rest) {
            return Err("Your vault needs your recovery code and one more way to open it, so this one has to stay. Add another first.".into());
        }
        let mine = self.device_slot(&state)?;
        self.service.delete_slot(&slot.slot)?;
        if mine.as_deref() == Some(slot.slot.as_str()) {
            let _ = self.keys.delete(&keys::key_ref(&state.id));
        }
        Ok(slot.label.clone())
    }

    /// Write the file list `change` makes, retrying once on a conflict
    /// with a fresh copy of the list.
    fn commit(
        &self,
        open: &mut Open,
        delete: &[String],
        change: &dyn Fn(&Index) -> Result<Index, String>,
    ) -> Result<(), String> {
        for attempt in 0..2 {
            let next = change(&open.index)?;
            let blob = next.seal(&open.vmk).map_err(err)?;
            match self.service.write_index(open.index.epoch, &blob, delete)? {
                Write::Done => {
                    self.remember(&open.state.id, next.epoch)?;
                    open.index = next;
                    return Ok(());
                }
                Write::Conflict if attempt == 0 => {
                    let state = self.state()?;
                    if state.id != open.state.id {
                        return Err("Your vault was replaced while this ran.".into());
                    }
                    open.index = self.open_index(&state, &open.vmk)?;
                    open.state = state;
                }
                Write::Conflict => {}
            }
        }
        Err("Your vault changed on another device at the same moment. Try again.".into())
    }

    /// Seal `plain` as a new object, upload it, and list it.
    fn store(&self, open: &mut Open, add: Add<'_>, plain: &[u8]) -> Result<Entry, String> {
        let (bytes, next) = open.index.add(&open.vmk, add, plain).map_err(err)?;
        let entry = next
            .entries
            .last()
            .cloned()
            .ok_or("The file couldn't be added.")?;
        self.service.put_object(&entry.object, &bytes)?;
        let added = entry.clone();
        self.commit(open, &[], &|index| {
            let mut next = index.next().map_err(err)?;
            next.entries.push(added.clone());
            Ok(next)
        })?;
        Ok(entry)
    }

    pub(crate) fn add_file(&self, path: &Path, project: Option<&str>) -> Result<Entry, String> {
        let size = std::fs::metadata(path)
            .map_err(|error| format!("{}: {error}", path.display()))?
            .len();
        if size > MAX_FILE {
            return Err(format!(
                "{} is too big for your vault (11 MB at most).",
                path.display()
            ));
        }
        let plain = Zeroizing::new(
            std::fs::read(path).map_err(|error| format!("{}: {error}", path.display()))?,
        );
        let name = path
            .file_name()
            .map_or_else(|| "File".into(), |name| name.to_string_lossy().into_owned());
        let mut open = self.open()?;
        self.store(
            &mut open,
            Add {
                kind: Kind::File,
                name: &name,
                media: Some(media_of(path)),
                project,
                about: Vec::new(),
                route: None,
                created_at: self.now,
            },
            &plain,
        )
    }

    pub(crate) fn list(&self, project: Option<&str>) -> Result<Vec<Entry>, String> {
        let open = self.open()?;
        Ok(open
            .index
            .entries
            .into_iter()
            .filter(|e| project.is_none_or(|p| e.project.as_deref() == Some(p)))
            .collect())
    }

    fn find<'i>(index: &'i Index, what: &str) -> Result<&'i Entry, String> {
        if let Some(entry) = index.find(what) {
            return Ok(entry);
        }
        let named: Vec<&Entry> = index.entries.iter().filter(|e| e.name == what).collect();
        if let [one] = named.as_slice() {
            return Ok(one);
        }
        let prefixed: Vec<&Entry> = index
            .entries
            .iter()
            .filter(|e| what.len() >= 4 && e.object.starts_with(what))
            .collect();
        match (named.len(), prefixed.as_slice()) {
            (0, [one]) => Ok(one),
            (0, []) => Err(format!(
                "No file in your vault matches {what}. See: openagents vault list"
            )),
            _ => Err(format!(
                "More than one file matches {what}; give the start of its id."
            )),
        }
    }

    pub(crate) fn get(&self, what: &str) -> Result<(Entry, Zeroizing<Vec<u8>>), String> {
        let open = self.open()?;
        let entry = Self::find(&open.index, what)?.clone();
        let bytes = self.service.get_object(&entry.object)?;
        let plain = open
            .index
            .open_object(&open.vmk, &entry.object, &bytes)
            .map_err(err)?;
        Ok((entry, plain))
    }

    pub(crate) fn remove(&self, what: &str) -> Result<Entry, String> {
        let mut open = self.open()?;
        let entry = Self::find(&open.index, what)?.clone();
        let object = entry.object.clone();
        self.commit(&mut open, std::slice::from_ref(&object), &|index| {
            index.remove(&object).map_err(err)
        })?;
        Ok(entry)
    }

    pub(crate) fn ask(
        &self,
        what: &[&str],
        question: &str,
        route: Option<Route>,
        model: &dyn LocalModel,
        project: Option<&str>,
    ) -> Result<Asked, String> {
        let mut open = self.open()?;
        let entries: Vec<Entry> = what
            .iter()
            .map(|w| Self::find(&open.index, w).cloned())
            .collect::<Result<_, _>>()?;
        // Pick the route before any file is opened.
        let local = match route {
            Some(Route::Fast) => None,
            Some(Route::Device) => Some(model.model().ok_or(
                "No model is running on this computer. Start one with: openagents vault serve-local",
            )?),
            Some(Route::Private) => return Err("That route isn't offered yet.".into()),
            None => Some(model.model().ok_or(
                "No model is running on this computer. Start one with: openagents vault serve-local. Or pass --route fast to have Google Gemini read these files (Google sees them).",
            )?),
        };
        let mut files = Vec::new();
        for entry in &entries {
            let bytes = self.service.get_object(&entry.object)?;
            let data = open
                .index
                .open_object(&open.vmk, &entry.object, &bytes)
                .map_err(err)?;
            files.push(Plain {
                name: entry.name.clone(),
                media: entry
                    .media
                    .clone()
                    .unwrap_or_else(|| "application/octet-stream".into()),
                data,
            });
        }
        let (route, model_name, answer) = match local {
            Some(name) => {
                let request = local::chat_request(&name, question, &files)?;
                let answer = Zeroizing::new(model.complete(&request)?);
                (Route::Device, name, answer)
            }
            None => {
                let (answer, name) = self.service.answer(question, &files)?;
                (Route::Fast, name, Zeroizing::new(answer))
            }
        };
        drop(files);
        let project = project.map(str::to_owned).or_else(|| {
            let first = entries.first()?.project.clone()?;
            entries
                .iter()
                .all(|e| e.project.as_deref() == Some(first.as_str()))
                .then_some(first)
        });
        let title: String = question.chars().take(120).collect();
        let name = format!("Answer: {title}");
        let entry = self.store(
            &mut open,
            Add {
                kind: Kind::Answer,
                name: &name,
                media: Some("text/plain"),
                project: project.as_deref(),
                about: entries.iter().map(|e| e.object.clone()).collect(),
                route: Some(route),
                created_at: self.now,
            },
            answer.as_bytes(),
        )?;
        Ok(Asked {
            route,
            model: model_name,
            answer,
            entry,
        })
    }
}

/// What a slot opens with, in plain words.
fn opens_with(slot: &Slot) -> &'static str {
    match slot.method {
        Method::PasskeyPrf => "passkey",
        Method::Device => "this device's key",
        Method::Nostr => "Nostr key",
        Method::Recovery => "recovery code",
        Method::Pairing => "link (not used yet)",
    }
}

/// A pairing link's slot id and secret.
fn parse_link(link: &str) -> Result<(String, Zeroizing<Vec<u8>>), String> {
    use base64::Engine;
    let bad = || "That isn't a link from your vault.".to_owned();
    let fragment = link
        .trim()
        .split_once("#pair=")
        .map_or(link.trim(), |(_, f)| f);
    let (slot, secret) = fragment.split_once('.').ok_or_else(bad)?;
    if !oa_vault::valid_id(slot) {
        return Err(bad());
    }
    let secret = Zeroizing::new(
        base64::engine::general_purpose::URL_SAFE_NO_PAD
            .decode(secret)
            .map_err(|_| bad())?,
    );
    if secret.len() < 32 {
        return Err(bad());
    }
    Ok((slot.to_owned(), secret))
}

fn err(error: oa_vault::Error) -> String {
    error.to_string()
}

/// Write `bytes` to `path`, readable only by this user.
fn write_private(path: &Path, bytes: &[u8]) -> Result<(), String> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|error| format!("{}: {error}", dir.display()))?;
    }
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create(true).truncate(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    options
        .open(path)
        .and_then(|mut file| file.write_all(bytes))
        .map_err(|error| format!("{}: {error}", path.display()))
}
