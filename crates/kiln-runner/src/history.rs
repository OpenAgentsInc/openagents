//! Retain admission before execution and the original result before publication.
use nostr::domain::Event;
use secp256k1::{SecretKey, XOnlyPublicKey};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::{
    io::{Read, Write},
    path::Path,
};

#[derive(Serialize, Deserialize)]
pub struct Completed {
    pub request: Event,
    pub reply: Event,
    pub archive: Event,
    pub check: Option<Event>,
}
pub enum Prior {
    Absent,
    Pending,
    Completed(Completed),
}
fn read<T: serde::de::DeserializeOwned>(p: &Path) -> Result<T, String> {
    let mut bytes = Vec::new();
    std::fs::File::open(p)
        .map_err(|e| e.to_string())?
        .take(512 * 1024 + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() > 512 * 1024 {
        return Err("runner history record exceeds its bound".into());
    }
    serde_json::from_slice(&bytes)
        .map_err(|_| "runner history is unreadable; it was not reset".into())
}
fn write<T: Serialize>(p: &Path, value: &T, exclusive: bool) -> Result<(), String> {
    let parent = p.parent().ok_or("history directory")?;
    std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    let bytes = serde_json::to_vec(value).map_err(|e| e.to_string())?;
    if bytes.len() > 512 * 1024 {
        return Err("runner history record exceeds its bound".into());
    }
    let target = if exclusive {
        p.to_path_buf()
    } else {
        p.with_extension("new")
    };
    let mut options = std::fs::OpenOptions::new();
    options.write(true);
    if exclusive {
        options.create_new(true);
    } else {
        options.create(true).truncate(true);
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut f = options.open(&target).map_err(|e| e.to_string())?;
    f.write_all(&bytes)
        .and_then(|_| f.sync_all())
        .map_err(|e| e.to_string())?;
    if !exclusive {
        std::fs::rename(target, p).map_err(|e| e.to_string())?;
    }
    std::fs::File::open(parent)
        .and_then(|f| f.sync_all())
        .map_err(|e| e.to_string())
}
pub fn prior(root: &Path, request: &Event) -> Result<Prior, String> {
    let dir = root.join("history");
    let done = dir.join(format!("{}.done", request.id));
    if done.exists() {
        let c: Completed = read(&done)?;
        c.request.validate_crypto().map_err(|e| e.to_string())?;
        c.reply.validate_crypto().map_err(|e| e.to_string())?;
        nostr::private_artifact::admit(&c.archive).map_err(|e| e.to_string())?;
        if let Some(check) = &c.check {
            check.validate_crypto().map_err(|e| e.to_string())?;
            if check.pubkey != c.archive.pubkey {
                return Err("runner check signer differs".into());
            }
        }
        if c.request != *request
            || !kiln::cj::answers(&c.reply, &request.id, &c.archive.pubkey, &request.pubkey)
        {
            return Err("runner history identity differs".into());
        }
        return Ok(Prior::Completed(c));
    }
    let pending = dir.join(format!("{}.intent", request.id));
    if pending.exists() {
        let old: Event = read(&pending)?;
        if old != *request {
            return Err("runner request identity conflicts".into());
        }
        return Ok(Prior::Pending);
    }
    Ok(Prior::Absent)
}
pub fn reserve(root: &Path, request: &Event) -> Result<(), String> {
    let dir = root.join("history");
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let count = std::fs::read_dir(&dir)
        .map_err(|e| e.to_string())?
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().is_some_and(|s| s == "intent"))
        .count();
    if count >= 4096 {
        return Err("runner history is full; admissions are paused".into());
    }
    write(&dir.join(format!("{}.intent", request.id)), request, true)
}
pub fn complete(
    root: &Path,
    secret: &SecretKey,
    request: &Event,
    reply: &Event,
    check: Option<&Event>,
    now: u64,
) -> Result<Event, String> {
    if !matches!(prior(root, request)?, Prior::Pending) {
        return Err("runner result has no pending admission".into());
    }
    let body = serde_json::to_value(kiln::remote::RetainedResult {
        v: kiln::remote::ARCHIVE_V.into(),
        requires: vec![],
        issuer: reply.pubkey.clone(),
        request: request.id.clone(),
        event: reply.clone(),
    })
    .map_err(|e| e.to_string())?;
    let bytes = nostr::contracts::jcs(&body).map_err(|e| e.to_string())?;
    let envelope = json!({"v":"openagents.artifact-envelope.v1","requires":[],"artifact":{"digest":nostr::contracts::digest_bytes(&bytes),"size":bytes.len(),"media_type":"application/json","schema":kiln::remote::ARCHIVE_V},"inline":body,"issued_at":now,"retain_until":now+30*86400});
    let recipient: XOnlyPublicKey = request.pubkey.parse().map_err(|_| "result recipient")?;
    let mailbox = secp256k1::rand::random::<[u8; 32]>()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect::<String>();
    let archive = nostr::private_artifact::seal(
        &envelope,
        secret,
        &recipient,
        &mailbox,
        now,
        secp256k1::rand::random(),
    )
    .map_err(|e| e.to_string())?;
    if reply.pubkey != archive.pubkey
        || !kiln::cj::answers(reply, &request.id, &archive.pubkey, &request.pubkey)
    {
        return Err("runner result signer differs".into());
    }
    write(
        &root.join("history").join(format!("{}.done", request.id)),
        &Completed {
            request: request.clone(),
            reply: reply.clone(),
            archive: archive.clone(),
            check: check.cloned(),
        },
        false,
    )?;
    Ok(archive)
}
pub fn outbox(root: &Path) -> Result<Vec<(String, Event)>, String> {
    let dir = root.join("history");
    if !dir.exists() {
        return Ok(vec![]);
    }
    let mut events = Vec::new();
    for e in std::fs::read_dir(&dir).map_err(|e| e.to_string())? {
        let p = e.map_err(|e| e.to_string())?.path();
        if p.extension().is_none_or(|s| s != "done") {
            continue;
        }
        let c: Completed = read(&p)?;
        if p.file_stem().and_then(|s| s.to_str()) != Some(c.request.id.as_str())
            || !matches!(prior(root, &c.request)?, Prior::Completed(_))
        {
            return Err("runner outbox identity differs".into());
        }
        for event in c.check.into_iter().chain(std::iter::once(c.archive)) {
            if !dir
                .join(format!("{}.{}.ack", c.request.id, event.id))
                .exists()
            {
                events.push((c.request.id.clone(), event));
            }
        }
    }
    Ok(events)
}
pub fn acknowledge(root: &Path, request: &str, event: &str) -> Result<(), String> {
    write(
        &root.join("history").join(format!("{request}.{event}.ack")),
        &true,
        false,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn pending_requests_never_become_new_and_results_survive_restart() {
        let root = std::env::temp_dir().join(format!("kiln-journal-{}", std::process::id()));
        std::fs::create_dir_all(&root).unwrap();
        let buyer = kiln::cj::Key::from_bytes([1; 32]).unwrap();
        let runner = kiln::cj::Key::from_bytes([2; 32]).unwrap();
        let request = buyer
            .seal_request(
                runner.pubkey(),
                &json!({"v":1,"requires":[],"task":"fixture","transcript":[]}),
                [3; 32],
                100,
            )
            .unwrap();
        reserve(&root, &request).unwrap();
        assert!(reserve(&root, &request).is_err());
        assert!(matches!(prior(&root, &request).unwrap(), Prior::Pending));
        let reply = runner
            .seal(
                buyer.pubkey(),
                kiln::cj::RESULT_KIND,
                vec![
                    nostr::domain::Tag::new(vec!["p".into(), buyer.pubkey().into()]),
                    nostr::domain::Tag::new(vec!["e".into(), request.id.clone()]),
                ],
                &json!({"v":1,"requires":[],"type":"result","text":"fixture result"}),
                [4; 32],
                101,
            )
            .unwrap();
        let check = runner.sign(kiln::patch::COMMENT_KIND, vec![], "passed".into(), 101);
        let archive = complete(
            &root,
            &SecretKey::from_byte_array([2; 32]).unwrap(),
            &request,
            &reply,
            Some(&check),
            101,
        )
        .unwrap();
        let opened =
            nostr::private_artifact::open(&archive, &SecretKey::from_byte_array([1; 32]).unwrap())
                .unwrap();
        let record: kiln::remote::RetainedResult =
            serde_json::from_slice(opened.inline_bytes().unwrap()).unwrap();
        assert_eq!(record.event, reply);
        match prior(&root, &request).unwrap() {
            Prior::Completed(c) => {
                assert_eq!(c.reply, reply);
                assert_eq!(c.check.as_ref(), Some(&check));
            }
            _ => panic!("result was not retained"),
        }
        assert_eq!(outbox(&root).unwrap().len(), 2);
        acknowledge(&root, &request.id, &archive.id).unwrap();
        assert_eq!(outbox(&root).unwrap().len(), 1);
        acknowledge(&root, &request.id, &check.id).unwrap();
        assert!(outbox(&root).unwrap().is_empty());
        std::fs::remove_dir_all(root).unwrap();
    }
}
