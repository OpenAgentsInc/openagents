//! The vault service API (NIP-VAULT "Service API (tier user)"), behind a
//! trait so tests run against an in-process fake.
//!
//! Only ciphertext, sealed key slots and sealed file lists cross this
//! boundary, except for [`Service::answer`]: the Fast route, which sends the
//! plaintext of one question's files to the service for Google Gemini.

use std::time::Duration;

use oa_vault::Slot;
use openagents_login::Saved;
use serde_json::{Value, json};
use zeroize::Zeroizing;

/// The person's vault as the service holds it.
#[derive(Clone)]
pub(crate) struct State {
    pub id: String,
    pub slots: Vec<Slot>,
    /// The file list's current version number, and its sealed bytes.
    pub epoch: u32,
    pub blob: Vec<u8>,
    /// Stored objects and their sizes.
    pub objects: Vec<(String, u64)>,
}

/// A decrypted file sent for one answer.
pub(crate) struct Plain {
    pub name: String,
    pub media: String,
    pub data: Zeroizing<Vec<u8>>,
}

/// How a file list write ended.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Write {
    Done,
    /// Another device changed the list first.
    Conflict,
}

pub(crate) trait Service {
    /// The origin the service answers on, for pairing links.
    fn origin(&self) -> String;
    fn state(&self) -> Result<Option<State>, String>;
    fn create(&self, vault: &str, slots: &[Slot], index: &[u8]) -> Result<(), String>;
    fn add_slot(&self, slot: &Slot) -> Result<(), String>;
    fn delete_slot(&self, slot: &str) -> Result<(), String>;
    fn put_object(&self, object: &str, bytes: &[u8]) -> Result<(), String>;
    fn get_object(&self, object: &str) -> Result<Vec<u8>, String>;
    /// Compare-and-swap: replace the list at `after` with `blob`, then
    /// delete `delete`.
    fn write_index(&self, after: u32, blob: &[u8], delete: &[String]) -> Result<Write, String>;
    /// The Fast route: Google Gemini reads `files` for this one answer.
    /// Returns the answer and the model's name.
    fn answer(&self, question: &str, files: &[Plain]) -> Result<(String, String), String>;
}

/// Read `GET /vault/api/state`'s answer.
pub(crate) fn parse_state(body: &Value) -> Result<Option<State>, String> {
    let vault = &body["vault"];
    if vault.is_null() {
        return Ok(None);
    }
    let bad = || "The vault service sent something this program can't read.".to_owned();
    let id = vault["id"].as_str().ok_or_else(bad)?.to_owned();
    let slots = vault["slots"]
        .as_array()
        .ok_or_else(bad)?
        .iter()
        .map(|slot| serde_json::from_value::<Slot>(slot.clone()).map_err(|_| bad()))
        .collect::<Result<Vec<_>, _>>()?;
    let epoch = vault["index"]["epoch"]
        .as_u64()
        .and_then(|epoch| u32::try_from(epoch).ok())
        .ok_or_else(bad)?;
    let blob =
        oa_vault::unb64(vault["index"]["blob"].as_str().ok_or_else(bad)?).map_err(|_| bad())?;
    let objects = vault["objects"]
        .as_array()
        .map(|objects| {
            objects
                .iter()
                .filter_map(|o| Some((o["object"].as_str()?.to_owned(), o["size"].as_u64()?)))
                .collect()
        })
        .unwrap_or_default();
    Ok(Some(State {
        id,
        slots,
        epoch,
        blob,
        objects,
    }))
}

/// The website, as the signed-in account (`coder login`).
pub(crate) struct Http {
    saved: Saved,
    http: reqwest::blocking::Client,
}

impl Http {
    pub(crate) fn new(saved: Saved) -> Result<Self, String> {
        let http = reqwest::blocking::Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .connect_timeout(Duration::from_secs(10))
            .timeout(Duration::from_secs(180))
            .build()
            .map_err(|error| format!("Couldn't start the web client: {error}"))?;
        Ok(Self { saved, http })
    }

    fn url(&self, path: &str) -> String {
        format!("{}/vault/api{path}", self.saved.origin)
    }

    fn send(&self, request: reqwest::blocking::RequestBuilder) -> Result<Reply, String> {
        let response = request
            .bearer_auth(self.saved.token())
            .send()
            .map_err(|_| "The vault service couldn't be reached.".to_owned())?;
        let status = response.status().as_u16();
        let bytes = response
            .bytes()
            .map_err(|_| "The vault service stopped answering.".to_owned())?
            .to_vec();
        if status == 401 {
            return Err("This sign-in stopped working. Sign in again with: coder login".into());
        }
        Ok(Reply { status, bytes })
    }
}

struct Reply {
    status: u16,
    bytes: Vec<u8>,
}

impl Reply {
    fn ok(&self) -> bool {
        (200..300).contains(&self.status)
    }

    fn json(&self) -> Value {
        serde_json::from_slice(&self.bytes).unwrap_or(Value::Null)
    }

    /// The service's refusal in its own words.
    fn refusal(&self) -> String {
        let body = self.json();
        body["error"]
            .as_str()
            .or_else(|| body["error"]["message"].as_str())
            .map_or_else(
                || format!("The vault service answered {}.", self.status),
                str::to_owned,
            )
    }

    fn check(self) -> Result<Self, String> {
        if self.ok() {
            Ok(self)
        } else {
            Err(self.refusal())
        }
    }
}

impl Service for Http {
    fn origin(&self) -> String {
        self.saved.origin.clone()
    }

    fn state(&self) -> Result<Option<State>, String> {
        let reply = self.send(self.http.get(self.url("/state")))?.check()?;
        parse_state(&reply.json())
    }

    fn create(&self, vault: &str, slots: &[Slot], index: &[u8]) -> Result<(), String> {
        let body = json!({ "vault": vault, "slots": slots, "index": oa_vault::b64(index) });
        let reply = self.send(self.http.post(self.url("/create")).json(&body))?;
        if reply.status == 409 {
            return Err("You already have a vault.".into());
        }
        reply.check().map(drop)
    }

    fn add_slot(&self, slot: &Slot) -> Result<(), String> {
        let body = json!({ "slot": slot });
        self.send(self.http.post(self.url("/slots")).json(&body))?
            .check()
            .map(drop)
    }

    fn delete_slot(&self, slot: &str) -> Result<(), String> {
        let reply = self.send(self.http.post(self.url(&format!("/slots/{slot}/delete"))))?;
        if reply.status == 409 {
            return Err("Your vault needs your recovery code and one more way to open it.".into());
        }
        reply.check().map(drop)
    }

    fn put_object(&self, object: &str, bytes: &[u8]) -> Result<(), String> {
        self.send(
            self.http
                .put(self.url(&format!("/objects/{object}")))
                .header("content-type", "application/octet-stream")
                .body(bytes.to_vec()),
        )?
        .check()
        .map(drop)
    }

    fn get_object(&self, object: &str) -> Result<Vec<u8>, String> {
        let reply = self.send(self.http.get(self.url(&format!("/objects/{object}"))))?;
        if reply.status == 404 {
            return Err("That file is gone from your vault.".into());
        }
        Ok(reply.check()?.bytes)
    }

    fn write_index(&self, after: u32, blob: &[u8], delete: &[String]) -> Result<Write, String> {
        let body = json!({ "after": after, "blob": oa_vault::b64(blob), "delete": delete });
        let reply = self.send(self.http.post(self.url("/index")).json(&body))?;
        if reply.status == 409 {
            return Ok(Write::Conflict);
        }
        reply.check().map(|_| Write::Done)
    }

    fn answer(&self, question: &str, files: &[Plain]) -> Result<(String, String), String> {
        let files: Vec<Value> = files
            .iter()
            .map(|file| {
                json!({
                    "name": file.name,
                    "media": file.media,
                    "data": oa_vault::b64(&file.data),
                })
            })
            .collect();
        let body = Zeroizing::new(
            serde_json::to_vec(&json!({ "question": question, "files": files }))
                .map_err(|_| "The question couldn't be written.".to_owned())?,
        );
        let reply = self
            .send(
                self.http
                    .post(self.url("/answer"))
                    .header("content-type", "application/json")
                    .body(body.to_vec()),
            )?
            .check()?;
        let value = reply.json();
        let answer = value["answer"]
            .as_str()
            .ok_or("The vault service sent no answer.")?;
        let model = value["model"].as_str().unwrap_or("Google Gemini");
        Ok((answer.to_owned(), model.to_owned()))
    }
}
