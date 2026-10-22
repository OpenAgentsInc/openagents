//! One relay exchange per call: publish an event and wait for `OK`, or read
//! stored events to the end-of-stored-events marker.

use std::time::Duration;

use nostr::domain::Event;
use secp256k1::SecretKey;
use serde_json::{Value, json};

/// How long one exchange may take.
pub const WAIT: Duration = Duration::from_secs(20);

/// Publish `event` and wait for the relay to accept it.
///
/// # Errors
///
/// The relay's refusal, or why it couldn't be reached.
pub async fn publish(url: &str, secret: &SecretKey, event: &Event) -> Result<(), String> {
    tokio::time::timeout(WAIT, async {
        let mut socket = nostr_transport::Connection::connect(url, secret, WAIT).await?;
        socket.send(json!(["EVENT", event])).await?;
        loop {
            let frame = socket.next().await?;
            if frame[0] == "OK" && frame[1] == event.id.as_str() {
                let _ = socket.close().await;
                return if frame[2] == true {
                    Ok(())
                } else {
                    Err(format!("the relay refused it: {}", frame[3]))
                };
            }
        }
    })
    .await
    .map_err(|_| format!("{url} did not answer in {} s", WAIT.as_secs()))?
}

/// Read the stored events that match `filter`.
///
/// # Errors
///
/// Why the relay couldn't be read.
pub async fn query(url: &str, secret: &SecretKey, filter: Value) -> Result<Vec<Event>, String> {
    tokio::time::timeout(WAIT, async {
        let mut socket = nostr_transport::Connection::connect(url, secret, WAIT).await?;
        socket.send(json!(["REQ", "q", filter])).await?;
        let mut found = Vec::new();
        loop {
            let frame = socket.next().await?;
            match frame[0].as_str() {
                Some("EVENT") if frame[1] == "q" => {
                    if let Ok(event) = serde_json::from_value::<Event>(frame[2].clone())
                        && event.validate_crypto().is_ok()
                    {
                        found.push(event);
                    }
                }
                Some("EOSE") if frame[1] == "q" => break,
                Some("CLOSED") if frame[1] == "q" => {
                    return Err(format!("the relay closed the query: {}", frame[2]));
                }
                _ => {}
            }
        }
        let _ = socket.close().await;
        Ok(found)
    })
    .await
    .map_err(|_| format!("{url} did not answer in {} s", WAIT.as_secs()))?
}

/// Send `job` to the runner `runner` as `key`, with `extra` tags on the
/// request (an agent's identity), and wait up to `wait` for its result.
/// `progress` gets each progress line.
///
/// # Errors
///
/// The runner's refusal, or why it didn't answer.
pub async fn ask_runner(
    url: &str,
    key: &crate::keys::Signer,
    runner: &str,
    job: &kiln::remote::Job,
    extra: &[nostr::domain::Tag],
    wait: Duration,
    progress: &dyn Fn(&str),
) -> Result<kiln::remote::Outcome, String> {
    let request = runner_request(key, runner, job, extra)?;
    ask_runner_event(url, key, runner, &request, wait, progress)
        .await
        .map(|r| r.0)
}

/// Prepare exact bytes for a caller's durable intent before sending anything.
pub fn runner_request(
    key: &crate::keys::Signer,
    runner: &str,
    job: &kiln::remote::Job,
    extra: &[nostr::domain::Tag],
) -> Result<Event, String> {
    let body = json!({
        "v": 1,
        "requires": [],
        "task": serde_json::to_string(job).map_err(|e| e.to_string())?,
        "transcript": [],
        "client": concat!("kiln-git ", env!("CARGO_PKG_VERSION")),
    });
    let nonce: [u8; 32] = secp256k1::rand::random();
    key.key
        .seal_request_tagged(runner, &body, nonce, now(), extra)
}

/// Send an already retained request. Reconnects keep that same event identity.
pub async fn ask_runner_event(
    url: &str,
    key: &crate::keys::Signer,
    runner: &str,
    request: &Event,
    wait: Duration,
    progress: &dyn Fn(&str),
) -> Result<(kiln::remote::Outcome, Event), String> {
    use kiln::cj;
    request.validate_crypto().map_err(|e| e.to_string())?;
    if request.kind != cj::REQUEST_KIND
        || request.pubkey != key.key.pubkey()
        || !request.tag_values("p").any(|p| p == runner)
    {
        return Err("saved runner request differs from its selected principals".into());
    }
    // A relay connection lasts at most two minutes and a run can take ten,
    // so reconnect and read the stored answers again until one is final.
    tokio::time::timeout(wait, async {
        let mut sent = false;
        loop {
            let mut socket =
                match nostr_transport::Connection::connect(url, &key.secret, LINK).await {
                    Ok(socket) => socket,
                    Err(e) if sent => {
                        progress(&format!("reconnecting: {e}"));
                        tokio::time::sleep(Duration::from_secs(2)).await;
                        continue;
                    }
                    Err(e) => return Err(e),
                };
            socket
                .send(json!([
                    "REQ",
                    "a",
                    cj::answer_filter(key.key.pubkey(), &request.id)
                ]))
                .await?;
            if !sent {
                socket.send(json!(["EVENT", request])).await?;
                sent = true;
            }
            loop {
                let Ok(frame) = socket.next().await else {
                    break;
                };
                match frame[0].as_str() {
                    Some("OK") if frame[1] == request.id.as_str() && frame[2] == false => {
                        return Err(format!("the relay refused the request: {}", frame[3]));
                    }
                    Some("EVENT") if frame[1] == "a" => {
                        let Ok(event) = serde_json::from_value::<Event>(frame[2].clone()) else {
                            continue;
                        };
                        if !cj::answers(&event, &request.id, runner, key.key.pubkey()) {
                            continue;
                        }
                        let Ok(plain) = key.key.open(&event) else {
                            continue;
                        };
                        match cj::read(event.kind, &plain) {
                            cj::Answer::Processing(Some(line)) => progress(&line),
                            cj::Answer::Result { text, .. } => {
                                let _ = socket.close().await;
                                return serde_json::from_str(&text)
                                    .map(|outcome| (outcome, event))
                                    .map_err(|_| {
                                        "the runner's answer is not a result".to_string()
                                    });
                            }
                            cj::Answer::Error(e) => return Err(e),
                            _ => {}
                        }
                    }
                    _ => {}
                }
            }
        }
    })
    .await
    .map_err(|_| format!("the runner did not answer in {} s", wait.as_secs()))?
}

/// Recover a known result by reading only. This never republishes a request.
pub async fn runner_result(
    url: &str,
    key: &crate::keys::Signer,
    runner: &str,
    request: &Event,
) -> Result<Option<(kiln::remote::Outcome, Event)>, String> {
    let mut events = query(
        url,
        &key.secret,
        kiln::cj::answer_filter(key.key.pubkey(), &request.id),
    )
    .await?;
    let archives=query(url,&key.secret,json!({"kinds":[3188],"authors":[runner],"#p":[key.key.pubkey()],"since":request.created_at.saturating_sub(300),"limit":256})).await?;
    if archives.len() >= 256 {
        return Err("retained runner result page is incomplete".into());
    }
    for archive in archives {
        if archive.pubkey != runner {
            continue;
        }
        let opened =
            nostr::private_artifact::open(&archive, &key.secret).map_err(|e| e.to_string())?;
        if opened.artifact().schema.as_deref() != Some(kiln::remote::ARCHIVE_V) {
            continue;
        }
        let record: kiln::remote::RetainedResult =
            serde_json::from_slice(opened.inline_bytes().ok_or("retained result bytes")?)
                .map_err(|_| "invalid retained runner result")?;
        if record.v != kiln::remote::ARCHIVE_V
            || !record.requires.is_empty()
            || record.issuer != runner
        {
            return Err("retained runner result authority differs".into());
        }
        if record.request == request.id {
            events.push(record.event);
        }
    }
    runner_result_from(&events, key, runner, &request.id)
}

pub fn runner_result_from(
    events: &[Event],
    key: &crate::keys::Signer,
    runner: &str,
    request: &str,
) -> Result<Option<(kiln::remote::Outcome, Event)>, String> {
    let mut result = None;
    for event in events {
        if !kiln::cj::answers(event, request, runner, key.key.pubkey()) {
            continue;
        }
        let plain = key.key.open(event)?;
        let answer = kiln::cj::read(event.kind, &plain);
        let value = match answer {
            kiln::cj::Answer::Result { text, .. } => {
                serde_json::from_str::<kiln::remote::Outcome>(&text)
                    .map_err(|_| "invalid runner result")?
            }
            kiln::cj::Answer::Error(error) if error.starts_with("unknown:") => continue,
            kiln::cj::Answer::Error(error) => kiln::remote::Outcome {
                v: kiln::remote::RESULT_V.into(),
                code: None,
                summary: error,
                log: String::new(),
                ms: 0,
                check: None,
                published: None,
            },
            _ => continue,
        };
        if value.v != kiln::remote::RESULT_V {
            return Err("unsupported runner result".into());
        }
        if result
            .as_ref()
            .is_some_and(|old: &(kiln::remote::Outcome, Event)| old.0 != value)
        {
            return Err("runner result conflicts with its earlier result".into());
        }
        result = Some((value, event.clone()));
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn recovery_authenticates_the_original_request_and_refuses_conflicting_results() {
        let buyer = crate::keys::Signer {
            key: kiln::cj::Key::from_bytes([1; 32]).unwrap(),
            secret: SecretKey::from_byte_array([1; 32]).unwrap(),
        };
        let provider = kiln::cj::Key::from_bytes([2; 32]).unwrap();
        let request = "11".repeat(32);
        let tags = vec![
            nostr::domain::Tag::new(vec!["p".into(), buyer.key.pubkey().into()]),
            nostr::domain::Tag::new(vec!["e".into(), request.clone()]),
        ];
        let outcome = kiln::remote::Outcome {
            v: kiln::remote::RESULT_V.into(),
            code: Some(0),
            summary: "2 passed".into(),
            log: String::new(),
            ms: 50,
            check: None,
            published: None,
        };
        let reply=provider.seal(buyer.key.pubkey(),kiln::cj::RESULT_KIND,tags.clone(),&json!({"v":1,"requires":[],"type":"result","text":serde_json::to_string(&outcome).unwrap()}),[3;32],100).unwrap();
        assert!(
            runner_result_from(
                std::slice::from_ref(&reply),
                &buyer,
                provider.pubkey(),
                &"22".repeat(32)
            )
            .unwrap()
            .is_none()
        );
        assert_eq!(
            runner_result_from(
                &[reply.clone(), reply.clone()],
                &buyer,
                provider.pubkey(),
                &request
            )
            .unwrap(),
            Some((outcome.clone(), reply.clone()))
        );
        let mut changed = outcome;
        changed.code = Some(1);
        let other=provider.seal(buyer.key.pubkey(),kiln::cj::RESULT_KIND,tags,&json!({"v":1,"requires":[],"type":"result","text":serde_json::to_string(&changed).unwrap()}),[4;32],101).unwrap();
        assert!(runner_result_from(&[reply, other], &buyer, provider.pubkey(), &request).is_err());
    }
}

/// How long one relay connection lasts while waiting on a runner.
const LINK: Duration = Duration::from_secs(110);

fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}
