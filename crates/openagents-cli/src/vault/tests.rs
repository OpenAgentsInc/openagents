//! `openagents vault` against an in-process fake service and in-memory
//! keychains. Nothing here reaches the network, a real keychain, or the
//! real home folder.

use std::cell::{Cell, RefCell};
use std::collections::BTreeMap;
use std::path::Path;

use oa_vault::index::Route;
use oa_vault::{Kind, Method, Slot, slot};
use serde_json::Value;
use zeroize::Zeroizing;

use super::api::{Plain, Service, State, Write};
use super::keys::{DeviceKeys, NostrKey};
use super::local::{self, LocalModel};
use super::{Unlock, Vault, parse_link};

type Race = Box<dyn FnOnce(&Fake)>;

/// The service, in memory, with its compare-and-swap and slot rules.
#[derive(Default)]
struct Fake {
    state: RefCell<Option<State>>,
    objects: RefCell<BTreeMap<String, Vec<u8>>>,
    answered: Cell<usize>,
    /// Runs once, inside the next file list write, before its check.
    race: RefCell<Option<Race>>,
}

impl Service for Fake {
    fn origin(&self) -> String {
        "https://openagents.test".into()
    }

    fn state(&self) -> Result<Option<State>, String> {
        let mut state = self.state.borrow().clone();
        if let Some(state) = &mut state {
            state.objects = self
                .objects
                .borrow()
                .iter()
                .map(|(id, bytes)| (id.clone(), bytes.len() as u64))
                .collect();
        }
        Ok(state)
    }

    fn create(&self, vault: &str, slots: &[Slot], index: &[u8]) -> Result<(), String> {
        if self.state.borrow().is_some() {
            return Err("You already have a vault.".into());
        }
        assert!(slot::enough(slots), "a new vault needs enough slots");
        *self.state.borrow_mut() = Some(State {
            id: vault.to_owned(),
            slots: slots.to_vec(),
            epoch: oa_vault::Index::epoch_of(index).expect("index"),
            blob: index.to_vec(),
            objects: Vec::new(),
        });
        Ok(())
    }

    fn add_slot(&self, slot: &Slot) -> Result<(), String> {
        slot.check().map_err(|e| e.to_string())?;
        let mut state = self.state.borrow_mut();
        let state = state.as_mut().ok_or("no vault")?;
        assert_eq!(slot.vault, state.id);
        state.slots.push(slot.clone());
        Ok(())
    }

    fn delete_slot(&self, id: &str) -> Result<(), String> {
        let mut state = self.state.borrow_mut();
        let state = state.as_mut().ok_or("no vault")?;
        let rest: Vec<Slot> = state
            .slots
            .iter()
            .filter(|s| s.slot != id)
            .cloned()
            .collect();
        if !slot::enough(&rest) {
            return Err("not enough".into());
        }
        state.slots = rest;
        Ok(())
    }

    fn put_object(&self, object: &str, bytes: &[u8]) -> Result<(), String> {
        oa_vault::object::parse(bytes).map_err(|e| e.to_string())?;
        self.objects
            .borrow_mut()
            .insert(object.to_owned(), bytes.to_vec());
        Ok(())
    }

    fn get_object(&self, object: &str) -> Result<Vec<u8>, String> {
        self.objects
            .borrow()
            .get(object)
            .cloned()
            .ok_or_else(|| "That file is gone from your vault.".into())
    }

    fn write_index(&self, after: u32, blob: &[u8], delete: &[String]) -> Result<Write, String> {
        let race = self.race.borrow_mut().take();
        if let Some(race) = race {
            race(self);
        }
        let mut state = self.state.borrow_mut();
        let state = state.as_mut().ok_or("no vault")?;
        let epoch = oa_vault::Index::epoch_of(blob).map_err(|e| e.to_string())?;
        if after != state.epoch || epoch != after + 1 {
            return Ok(Write::Conflict);
        }
        state.epoch = epoch;
        state.blob = blob.to_vec();
        let mut objects = self.objects.borrow_mut();
        for object in delete {
            objects.remove(object);
        }
        Ok(Write::Done)
    }

    fn answer(&self, question: &str, files: &[Plain]) -> Result<(String, String), String> {
        self.answered.set(self.answered.get() + 1);
        Ok((
            format!("{} files; you asked: {question}", files.len()),
            "gemini-test".into(),
        ))
    }
}

/// One computer's keychain.
#[derive(Default)]
struct Memory(RefCell<BTreeMap<String, [u8; 32]>>);

impl DeviceKeys for Memory {
    fn load(&self, key_ref: &str) -> Result<Option<Zeroizing<[u8; 32]>>, String> {
        Ok(self.0.borrow().get(key_ref).copied().map(Zeroizing::new))
    }

    fn store(&self, key_ref: &str, secret: &[u8; 32]) -> Result<(), String> {
        self.0.borrow_mut().insert(key_ref.to_owned(), *secret);
        Ok(())
    }

    fn delete(&self, key_ref: &str) -> Result<(), String> {
        self.0.borrow_mut().remove(key_ref);
        Ok(())
    }
}

/// A local model that records what it was sent.
#[derive(Default)]
struct Model {
    up: bool,
    sent: RefCell<Option<Value>>,
}

impl LocalModel for Model {
    fn model(&self) -> Option<String> {
        self.up.then(|| "gpt-oss-20b".to_owned())
    }

    fn complete(&self, request: &Value) -> Result<String, String> {
        *self.sent.borrow_mut() = Some(request.clone());
        Ok("The balance is 42.".into())
    }
}

fn computer<'a>(
    service: &'a Fake,
    keys: &'a Memory,
    dir: &Path,
    name: &str,
    now: u64,
) -> Vault<'a> {
    Vault {
        service,
        keys,
        seen: dir.join(format!("{name}-seen.json")),
        label: name.to_owned(),
        now,
    }
}

fn file(dir: &Path, name: &str, text: &str) -> std::path::PathBuf {
    let path = dir.join(name);
    std::fs::write(&path, text).expect("write");
    path
}

#[test]
fn setup_then_files_round_trip_and_rm_shreds() {
    let dir = tempfile::tempdir().expect("dir");
    let service = Fake::default();
    let keys = Memory::default();
    let a = computer(&service, &keys, dir.path(), "laptop", 1_000);
    let made = a.setup().expect("setup");
    assert_eq!(made.words.split(' ').count(), 24);
    assert!(a.setup().is_err(), "a second vault is refused");
    let state = service.state.borrow().clone().expect("vault");
    assert_eq!(state.slots.len(), 2);
    assert!(state.slots.iter().any(|s| s.method == Method::Recovery));
    assert_eq!(
        state.slots[0].param("key_ref"),
        Some(super::keys::key_ref(&made.vault).as_str())
    );

    let statement = file(dir.path(), "statement.txt", "Balance: 42");
    let added = a.add_file(&statement, Some("p1")).expect("add");
    assert_eq!(added.kind, Kind::File);
    assert_eq!(added.media.as_deref(), Some("text/plain"));
    let stored = service
        .objects
        .borrow()
        .get(&added.object)
        .cloned()
        .expect("stored");
    assert!(
        !stored.windows(7).any(|w| w == b"Balance"),
        "the service holds only ciphertext"
    );
    assert_eq!(a.list(None).expect("list").len(), 1);
    assert_eq!(a.list(Some("p1")).expect("list").len(), 1);
    assert!(a.list(Some("p2")).expect("list").is_empty());
    let (entry, plain) = a.get("statement.txt").expect("get by name");
    assert_eq!(entry.object, added.object);
    assert_eq!(plain.as_slice(), b"Balance: 42");
    let (_, plain) = a.get(&added.object[..8]).expect("get by id");
    assert_eq!(plain.as_slice(), b"Balance: 42");

    let out = dir.path().join("copy.txt");
    let path = super::save(&entry, &plain, Some(&out)).expect("save");
    assert_eq!(std::fs::read(&path).expect("read"), b"Balance: 42");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = std::fs::metadata(&path).expect("meta").permissions().mode();
        assert_eq!(mode & 0o777, 0o600);
    }

    a.remove("statement.txt").expect("rm");
    assert!(a.list(None).expect("list").is_empty());
    assert!(service.objects.borrow().is_empty(), "the object is deleted");
    assert!(a.get(&added.object).is_err());
}

#[test]
fn recovery_code_lets_a_new_computer_in_and_it_remembers() {
    let dir = tempfile::tempdir().expect("dir");
    let service = Fake::default();
    let (laptop_keys, desk_keys) = (Memory::default(), Memory::default());
    let laptop = computer(&service, &laptop_keys, dir.path(), "laptop", 1_000);
    let made = laptop.setup().expect("setup");
    laptop
        .add_file(&file(dir.path(), "a.txt", "alpha"), None)
        .expect("add");

    let desk = computer(&service, &desk_keys, dir.path(), "desk", 2_000);
    assert!(desk.list(None).is_err(), "a new computer can't open it yet");
    assert!(desk.unlock(Unlock::Device).is_err());
    let wrong = oa_vault::recovery::Code::generate().expect("code");
    assert!(desk.unlock(Unlock::Recovery(wrong.words())).is_err());
    let typed = made.words.to_uppercase();
    let unlocked = desk.unlock(Unlock::Recovery(&typed)).expect("recovery");
    assert!(unlocked.remembered);
    assert_eq!(unlocked.files, 1);
    // From now on the device key opens it.
    assert_eq!(desk.list(None).expect("list").len(), 1);
    let again = desk.unlock(Unlock::Device).expect("device");
    assert!(!again.remembered);
    let devices = desk.devices().expect("devices");
    assert_eq!(devices.len(), 3);
    assert_eq!(
        devices
            .iter()
            .filter(|d| d["this_computer"] == Value::Bool(true))
            .count(),
        1
    );
}

#[test]
fn a_pairing_link_works_once_and_nostr_keys_open_it() {
    let dir = tempfile::tempdir().expect("dir");
    let service = Fake::default();
    let (a_keys, b_keys, c_keys) = (Memory::default(), Memory::default(), Memory::default());
    let a = computer(&service, &a_keys, dir.path(), "a", 1_000);
    a.setup().expect("setup");

    let link = a.add_pairing().expect("link");
    assert!(link.starts_with("https://openagents.test/settings/vault#pair="));
    let (slot_id, secret) = parse_link(&link).expect("parse");
    assert_eq!(secret.len(), 32);
    let pairing = service
        .state
        .borrow()
        .as_ref()
        .and_then(|s| s.slots.iter().find(|s| s.slot == slot_id).cloned())
        .expect("pairing slot");
    assert_eq!(pairing.method, Method::Pairing);
    assert_eq!(pairing.params["expires_at"], 1_600);

    let late = computer(&service, &b_keys, dir.path(), "late", 1_000 + 601);
    assert!(late.unlock(Unlock::Pair(&link)).is_err(), "expired");
    let b = computer(&service, &b_keys, dir.path(), "b", 1_100);
    assert!(b.unlock(Unlock::Pair(&link)).expect("pair").remembered);
    assert!(b.list(None).is_ok());
    assert!(
        service
            .state
            .borrow()
            .as_ref()
            .is_some_and(|s| s.slots.iter().all(|s| s.method != Method::Pairing)),
        "the link is used up"
    );
    let c = computer(&service, &c_keys, dir.path(), "c", 1_200);
    assert!(c.unlock(Unlock::Pair(&link)).is_err());

    let key = NostrKey {
        secret: secp256k1::SecretKey::from_byte_array([7; 32]).expect("key"),
        from: "test".into(),
    };
    assert!(c.unlock(Unlock::Nostr(&key)).is_err());
    a.add_nostr(&key).expect("add nostr");
    assert!(a.add_nostr(&key).is_err(), "once per key");
    assert!(c.unlock(Unlock::Nostr(&key)).expect("nostr").remembered);
    let other = NostrKey {
        secret: secp256k1::SecretKey::from_byte_array([9; 32]).expect("key"),
        from: "test".into(),
    };
    let d_keys = Memory::default();
    let d = computer(&service, &d_keys, dir.path(), "d", 1_300);
    assert!(d.unlock(Unlock::Nostr(&other)).is_err());
}

#[test]
fn a_vault_keeps_its_recovery_code_and_one_more_way_in() {
    let dir = tempfile::tempdir().expect("dir");
    let service = Fake::default();
    let keys = Memory::default();
    let a = computer(&service, &keys, dir.path(), "a", 1_000);
    a.setup().expect("setup");
    let state = service.state.borrow().clone().expect("vault");
    let device = state.slots[0].slot.clone();
    let recovery = state.slots[1].slot.clone();
    assert!(a.remove_device(&device[..8]).is_err());
    assert!(a.remove_device(&recovery[..8]).is_err());
    let key = NostrKey {
        secret: secp256k1::SecretKey::from_byte_array([7; 32]).expect("key"),
        from: "test".into(),
    };
    a.add_nostr(&key).expect("nostr");
    assert_eq!(a.remove_device(&device[..8]).expect("remove"), "a");
    assert!(
        keys.0.borrow().is_empty(),
        "this computer's key is gone too"
    );
    assert!(a.list(None).is_err());
}

#[test]
fn a_racing_write_is_retried_on_the_newer_list() {
    let dir = tempfile::tempdir().expect("dir");
    let service = Fake::default();
    let (a_keys, b_keys) = (Memory::default(), Memory::default());
    let a = computer(&service, &a_keys, dir.path(), "a", 1_000);
    let made = a.setup().expect("setup");
    let b = computer(&service, &b_keys, dir.path(), "b", 1_000);
    b.unlock(Unlock::Recovery(&made.words)).expect("b in");

    let first = file(dir.path(), "one.txt", "one");
    let second = file(dir.path(), "two.txt", "two");
    // b adds a file while a is between reading the list and writing it.
    let before = service.state.borrow().clone();
    b.add_file(&second, None).expect("b adds");
    let after = service.state.borrow().clone();
    *service.state.borrow_mut() = before;
    *service.race.borrow_mut() = Some(Box::new(move |fake: &Fake| {
        *fake.state.borrow_mut() = after;
    }));
    a.add_file(&first, None).expect("a adds after a retry");
    let names: Vec<String> = a
        .list(None)
        .expect("list")
        .into_iter()
        .map(|e| e.name)
        .collect();
    assert_eq!(names, ["two.txt", "one.txt"]);
}

#[test]
fn an_older_file_list_is_refused() {
    let dir = tempfile::tempdir().expect("dir");
    let service = Fake::default();
    let keys = Memory::default();
    let a = computer(&service, &keys, dir.path(), "a", 1_000);
    a.setup().expect("setup");
    let old = service.state.borrow().clone().expect("vault");
    a.add_file(&file(dir.path(), "x.txt", "x"), None)
        .expect("add");
    *service.state.borrow_mut() = Some(old);
    let error = a.list(None).err().expect("refused");
    assert!(error.contains("older"), "{error}");
}

#[test]
fn asking_on_this_device_sends_the_files_there_and_saves_the_answer() {
    let dir = tempfile::tempdir().expect("dir");
    let service = Fake::default();
    let keys = Memory::default();
    let a = computer(&service, &keys, dir.path(), "a", 1_000);
    a.setup().expect("setup");
    let added = a
        .add_file(
            &file(dir.path(), "statement.txt", "Balance: 42"),
            Some("p1"),
        )
        .expect("add");

    let down = Model::default();
    let error = a
        .ask(
            &["statement.txt"],
            "What is the balance?",
            None,
            &down,
            None,
        )
        .err()
        .expect("no model, no fallback");
    assert!(error.contains("--route fast"), "{error}");
    assert_eq!(service.answered.get(), 0, "never falls back to Fast");
    assert!(
        a.ask(&["statement.txt"], "q", Some(Route::Device), &down, None)
            .is_err()
    );

    let up = Model {
        up: true,
        ..Model::default()
    };
    let asked = a
        .ask(&["statement.txt"], "What is the balance?", None, &up, None)
        .expect("ask");
    assert_eq!(asked.route, Route::Device);
    assert_eq!(asked.answer.as_str(), "The balance is 42.");
    let sent = up.sent.borrow().clone().expect("request");
    assert_eq!(sent["model"], "gpt-oss-20b");
    assert_eq!(sent["stream"], false);
    assert_eq!(sent["messages"][0]["role"], "system");
    let user = sent["messages"][1]["content"].as_str().expect("user");
    assert!(user.contains("--- statement.txt ---\nBalance: 42"));
    assert!(user.ends_with("Question: What is the balance?"));
    assert_eq!(service.answered.get(), 0);

    assert_eq!(asked.entry.kind, Kind::Answer);
    assert_eq!(asked.entry.route, Some(Route::Device));
    assert_eq!(asked.entry.about, [added.object.clone()]);
    assert_eq!(asked.entry.project.as_deref(), Some("p1"));
    let (_, saved) = a.get(&asked.entry.object).expect("answer stored");
    assert_eq!(saved.as_slice(), b"The balance is 42.");

    let fast = a
        .ask(
            &[&added.object[..8]],
            "Total?",
            Some(Route::Fast),
            &down,
            None,
        )
        .expect("fast");
    assert_eq!(fast.route, Route::Fast);
    assert_eq!(fast.model, "gemini-test");
    assert_eq!(service.answered.get(), 1);
    assert_eq!(fast.entry.route, Some(Route::Fast));
}

#[test]
fn binary_files_are_refused_on_this_device() {
    let files = [Plain {
        name: "scan.pdf".into(),
        media: "application/pdf".into(),
        data: Zeroizing::new(vec![0xff, 0xfe, 0x00]),
    }];
    let error = local::chat_request("m", "q", &files)
        .err()
        .expect("refused");
    assert!(error.contains("--route fast"));
}

#[test]
fn messages_are_plain_words() {
    let mut texts = vec![super::USAGE.to_owned()];
    for (command, value) in [
        ("setup", serde_json::json!({ "recovery_code": "a b c" })),
        (
            "unlock",
            serde_json::json!({ "files": 2, "remembered": true }),
        ),
        (
            "add-device",
            serde_json::json!({ "link": "L", "minutes": 10 }),
        ),
        (
            "ask",
            serde_json::json!({ "route": "fast", "answer": "x", "object": "abcdef12" }),
        ),
        (
            "ask",
            serde_json::json!({ "route": "device", "model": "m", "answer": "x", "object": "abcdef12" }),
        ),
        ("remove-device", serde_json::json!({ "removed": "laptop" })),
    ] {
        texts.push(super::render(command, &value));
    }
    for text in texts {
        let found = oa_copy::violations(&text, &[]);
        assert!(found.is_empty(), "{found:?} in {text}");
    }
}

#[test]
fn serve_local_names_the_exact_origin() {
    let command = local::serve_command(
        Path::new("/bin/psionic-openai-server"),
        Path::new("/m/gpt-oss-20b-MXFP4.gguf"),
        8080,
        "https://openagents.com",
    );
    let line = command.join(" ");
    assert!(line.starts_with("/bin/psionic-openai-server -m /m/gpt-oss-20b-MXFP4.gguf"));
    assert!(line.ends_with("--host 127.0.0.1 --port 8080 --allow-origin https://openagents.com"));
    assert!(!line.contains("metal"), "GPT-OSS runs on the CPU");
}
