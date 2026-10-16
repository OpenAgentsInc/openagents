//! Research-only public inline-file transport; see this crate's README.
use base64::{Engine, engine::general_purpose::STANDARD};
use bitcoin::hashes::{Hash, sha256};
use layertree_tee_lab::{
    rpc::{self, Stored},
    rules::Record,
    unroll::UnrollPackage,
};
use nostr::domain::{Event, RelaySigner, Tag};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    net::{TcpStream, ToSocketAddrs},
    time::{Duration, Instant},
};
use tungstenite::{Message, WebSocket, stream::MaybeTlsStream};

pub const MAX_FILE: usize = 32768;
pub const BLOB_KIND: u16 = 1064;
pub const HEADER_KIND: u16 = 1065;
pub const RELAY: &str = "wss://relay.openagents.com";
const SCHEMA: &str = "openagents.layertree.mirror.v1";

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct File {
    pub v: String,
    pub tree_id: String,
    pub data: Stored,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Pair {
    pub blob: Event,
    pub header: Event,
}
fn tag(name: &str, value: impl Into<String>) -> Tag {
    Tag::new(vec![name.into(), value.into()])
}
fn one<'a>(event: &'a Event, name: &str) -> Result<&'a str, String> {
    let found = event
        .tags
        .iter()
        .filter(|t| t.name() == Some(name))
        .collect::<Vec<_>>();
    if found.len() != 1 {
        return Err("missing or repeated file tag".into());
    }
    found[0].value().ok_or("missing file tag value".into())
}
fn record(file: &File) -> Result<Record, String> {
    if file.v != SCHEMA || rpc::array::<32>(&file.tree_id).is_err() {
        return Err("invalid public file identity".into());
    }
    let rec =
        Record::decode(&rpc::bytes(&file.data.record)?).map_err(|_| "invalid public record")?;
    if let Some(raw) = &file.data.unroll {
        UnrollPackage::decode(&rpc::bytes(raw)?).map_err(|_| "invalid public unroll")?;
    }
    Ok(rec)
}
pub fn prepare(
    signer: &RelaySigner,
    tree_id: &str,
    data: Stored,
    now: u64,
) -> Result<Pair, String> {
    let file = File {
        v: SCHEMA.into(),
        tree_id: tree_id.into(),
        data,
    };
    let rec = record(&file)?;
    let bytes = serde_json::to_vec(&file).map_err(|_| "cannot encode public record")?;
    if bytes.len() > MAX_FILE {
        return Err("public record file is too large".into());
    }
    let blob = signer.sign(
        now,
        BLOB_KIND,
        vec![tag("type", "application/json")],
        STANDARD.encode(&bytes),
    );
    let header = signer.sign(
        now,
        HEADER_KIND,
        vec![
            tag("e", &blob.id),
            tag("m", "application/json"),
            tag("x", sha256::Hash::hash(&bytes).to_string()),
            tag("size", bytes.len().to_string()),
            tag("t", format!("layertree:{tree_id}")),
            tag("r", rpc::hex(&rec.hash())),
        ],
        "Signet tree recovery data.".into(),
    );
    Ok(Pair { blob, header })
}
pub fn decode(
    pair: &Pair,
    author: &str,
    tree_id: &str,
    expected_record: [u8; 32],
) -> Result<File, String> {
    for event in [&pair.blob, &pair.header] {
        if event.pubkey != author
            || event.content.len() > MAX_FILE * 2
            || event.tags.len() > 16
            || event
                .tags
                .iter()
                .any(|t| t.as_slice().len() > 4 || t.as_slice().iter().any(|s| s.len() > 256))
        {
            return Err("public file exceeds its bounds or has another author".into());
        }
        event
            .validate_crypto()
            .map_err(|_| "public file signature refused")?;
    }
    if pair.blob.kind != BLOB_KIND
        || pair.header.kind != HEADER_KIND
        || one(&pair.blob, "type")? != "application/json"
        || one(&pair.header, "m")? != "application/json"
        || one(&pair.header, "e")? != pair.blob.id
        || one(&pair.header, "t")? != format!("layertree:{tree_id}")
        || one(&pair.header, "r")? != rpc::hex(&expected_record)
    {
        return Err("public file header differs".into());
    }
    let bytes = STANDARD
        .decode(&pair.blob.content)
        .map_err(|_| "invalid public file base64")?;
    if bytes.len() > MAX_FILE
        || one(&pair.header, "size")? != bytes.len().to_string()
        || one(&pair.header, "x")? != sha256::Hash::hash(&bytes).to_string()
    {
        return Err("public file size or hash differs".into());
    }
    let file: File = serde_json::from_slice(&bytes).map_err(|_| "invalid public record JSON")?;
    if file.tree_id != tree_id || record(&file)?.hash() != expected_record {
        return Err("public file names another tree or record".into());
    }
    Ok(file)
}

struct Connection {
    socket: WebSocket<MaybeTlsStream<TcpStream>>,
    auth: Option<String>,
    authenticated: bool,
    deadline: Instant,
}
impl Connection {
    fn connect() -> Result<Self, String> {
        let address = ("relay.openagents.com", 443)
            .to_socket_addrs()
            .map_err(|_| "cannot resolve research relay")?
            .find(|a| a.is_ipv4())
            .ok_or("relay has no IPv4 address")?;
        let stream = TcpStream::connect_timeout(&address, Duration::from_secs(15))
            .map_err(|_| "cannot connect to research relay")?;
        stream
            .set_read_timeout(Some(Duration::from_secs(15)))
            .map_err(|_| "cannot set relay timeout")?;
        stream
            .set_write_timeout(Some(Duration::from_secs(15)))
            .map_err(|_| "cannot set relay timeout")?;
        let config = tungstenite::protocol::WebSocketConfig::default()
            .max_message_size(Some(131072))
            .max_frame_size(Some(131072));
        let (socket, _) = tungstenite::client_tls_with_config(RELAY, stream, Some(config), None)
            .map_err(|_| "relay TLS handshake failed")?;
        Ok(Self {
            socket,
            auth: None,
            authenticated: false,
            deadline: Instant::now() + Duration::from_secs(90),
        })
    }
    fn send(&mut self, value: Value) -> Result<(), String> {
        self.socket
            .send(Message::Text(value.to_string().into()))
            .map_err(|_| "cannot write to research relay".into())
    }
    fn read(&mut self, signer: &RelaySigner, now: u64) -> Result<Value, String> {
        loop {
            if Instant::now() > self.deadline {
                return Err("research relay deadline expired".into());
            }
            let msg = self
                .socket
                .read()
                .map_err(|_| "cannot read from research relay")?;
            let Message::Text(text) = msg else { continue };
            let value: Value = serde_json::from_str(&text).map_err(|_| "invalid relay JSON")?;
            if value[0] == "AUTH" {
                if self.auth.is_some() {
                    return Err("repeated relay authentication challenge".into());
                }
                let challenge = value[1]
                    .as_str()
                    .filter(|s| s.len() <= 256)
                    .ok_or("invalid relay authentication challenge")?;
                let event = signer.sign(
                    now,
                    22242,
                    vec![tag("relay", RELAY), tag("challenge", challenge)],
                    String::new(),
                );
                self.auth = Some(event.id.clone());
                self.send(json!(["AUTH", event]))?;
                continue;
            }
            if value[0] == "OK" && value[1].as_str() == self.auth.as_deref() {
                if value[2] != true {
                    return Err("relay authentication refused".into());
                }
                self.authenticated = true;
            }
            return Ok(value);
        }
    }
    fn publish(&mut self, event: &Event, signer: &RelaySigner, now: u64) -> Result<(), String> {
        self.send(json!(["EVENT", event]))?;
        let mut waiting = false;
        let mut retried = false;
        loop {
            let value = self.read(signer, now)?;
            if waiting && self.authenticated && !retried {
                self.send(json!(["EVENT", event]))?;
                retried = true;
                waiting = false;
            }
            if value[0] == "OK" && value[1] == event.id {
                if value[2] == true {
                    return Ok(());
                }
                if !retried
                    && value[3]
                        .as_str()
                        .is_some_and(|s| s.starts_with("auth-required:"))
                    && self.auth.is_some()
                {
                    waiting = true;
                    if self.authenticated {
                        self.send(json!(["EVENT", event]))?;
                        retried = true;
                        waiting = false;
                    }
                    continue;
                }
                return Err("research relay refused the public file".into());
            }
        }
    }
}

/// Replaying the same saved pair is safe: both event IDs stay unchanged.
pub fn publish_and_readback(pair: &Pair, signer: &RelaySigner, now: u64) -> Result<Pair, String> {
    let mut writer = Connection::connect()?;
    writer.publish(&pair.blob, signer, now)?;
    writer.publish(&pair.header, signer, now)?;
    let _ = writer.socket.close(None);
    let mut reader = Connection::connect()?;
    let request = json!(["REQ","layertree-exact",{"ids":[pair.blob.id,pair.header.id],"limit":2}]);
    reader.send(request.clone())?;
    let mut waiting = false;
    let mut retried = false;
    let mut blob = None;
    let mut header = None;
    loop {
        let value = reader.read(signer, now)?;
        if waiting && reader.authenticated && !retried {
            reader.send(request.clone())?;
            waiting = false;
            retried = true;
        }
        if value[0] == "CLOSED" && value[1] == "layertree-exact" {
            if !retried
                && value[2]
                    .as_str()
                    .is_some_and(|s| s.starts_with("auth-required:"))
                && reader.auth.is_some()
            {
                waiting = true;
                if reader.authenticated {
                    reader.send(request.clone())?;
                    waiting = false;
                    retried = true;
                }
                continue;
            }
            return Err("research relay refused readback".into());
        }
        if value[0] == "EVENT" && value[1] == "layertree-exact" {
            let event: Event = serde_json::from_value(value[2].clone())
                .map_err(|_| "invalid file readback event")?;
            if event.id == pair.blob.id {
                blob = Some(event);
            } else if event.id == pair.header.id {
                header = Some(event);
            }
        }
        if value[0] == "EOSE" && value[1] == "layertree-exact" {
            let _ = reader.socket.close(None);
            return Ok(Pair {
                blob: blob.ok_or("public file was not retained")?,
                header: header.ok_or("public file header was not retained")?,
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn public_file_is_bound_to_signature_tree_and_chain_record() {
        let signer = RelaySigner::from_secret_hex(&"31".repeat(32)).unwrap();
        let tree = "42".repeat(32);
        let rec = Record {
            prev: bitcoin::OutPoint::null(),
            changes: vec![],
            new_root: layertree_tee_lab::tree::Node::EMPTY,
            next_unroll_key: None,
        };
        let pair = prepare(
            &signer,
            &tree,
            Stored {
                record: rpc::hex(&rec.encode()),
                unroll: None,
            },
            1000,
        )
        .unwrap();
        assert!(decode(&pair, signer.pubkey(), &tree, rec.hash()).is_ok());
        assert!(decode(&pair, signer.pubkey(), &"43".repeat(32), rec.hash()).is_err());
        assert!(decode(&pair, signer.pubkey(), &tree, [0; 32]).is_err());
        let mut other = pair.clone();
        other.blob.content.push('A');
        assert!(decode(&other, signer.pubkey(), &tree, rec.hash()).is_err());
        let mut other = pair.clone();
        other.header = signer.sign(
            1000,
            HEADER_KIND,
            other
                .header
                .tags
                .iter()
                .filter(|t| t.name() != Some("x"))
                .cloned()
                .chain([tag("x", "00".repeat(32))])
                .collect(),
            other.header.content,
        );
        assert!(decode(&other, signer.pubkey(), &tree, rec.hash()).is_err());
    }
}
