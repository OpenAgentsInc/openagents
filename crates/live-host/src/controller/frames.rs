//! Original-signer frame admission. Frame plaintext and tokens are never stored.
use super::*;
use crate::{
    delivery::{Delivery, FrameContext, Sink},
    resident::Reply,
};
impl Controller {
    /// Operator registration; a plan cannot choose a recipient implementation.
    pub fn install_sink(&self, sink: Box<dyn Sink>) -> Result<()> {
        let mut current = self.sink.borrow_mut();
        if let Some(old) = current.as_mut() {
            old.withdraw()?;
        }
        *current = Some(sink);
        Ok(())
    }
    /// Withdraw the recipient and clear its retained copies before releasing it.
    pub fn remove_sink(&self) -> Result<()> {
        let mut current = self.sink.borrow_mut();
        if let Some(old) = current.as_mut() {
            old.withdraw()?;
        }
        *current = None;
        Ok(())
    }
    /// Actual mailbox identity and processing, independent of session admission.
    pub fn media_operation(&self) -> Result<Value> {
        let sink = self.sink.borrow();
        let sink = sink
            .as_ref()
            .ok_or("unsupported_feature: no media recipient")?;
        let binding = self.store.put(&sink.binding(), "application/json", None)?;
        let destination = self.store.put_json(
            &json!({"v":"openagents.livehost-recipient.v1","binding":binding,"media":sink.media()}),
        )?;
        let input = self.store.put(
            &contracts::jcs(&json!({"type":"string","maxLength":8192})).map_err(contract)?,
            contracts::SCHEMA_MEDIA_TYPE,
            None,
        )?;
        let output=self.store.put(&contracts::jcs(&json!({"type":"object","required":["delivered"],"additionalProperties":false,"properties":{"delivered":{"const":true}}})).map_err(contract)?,contracts::SCHEMA_MEDIA_TYPE,None)?;
        let id = format!("{}:live/text-mailbox", self.pubkey());
        let definition = json!({"v":1,"requires":[],"id":id,"profile":"native","summary":"Deliver one admitted text frame to a private mailbox.",
            "input":input,"output":output,"effects":{"reads":["artifact"],"writes":["media-mailbox"],"network":[],"process":false,"delegates":false,"spend":false},
            "minimum":{},"support":{"bounds":{},"cancellation":"before_dispatch","idempotency":"none","evidence":[]},
            "binding_contract":{"operation":"live-text-frame","interface":"openagents.livehost-frame.v1"},"meta":{"binding":binding,"destination":destination}});
        nostr::cap::parse_definition(&definition).map_err(|e| e.to_string())?;
        let operation = json!({"id":id,"artifact":self.store.put(&contracts::jcs(&definition).map_err(contract)?,"application/json",Some("openagents.cap.v1"))?});
        Ok(
            json!({"operation":operation,"destination":destination,"binding":binding,"media":sink.media()}),
        )
    }
    pub fn frame(&self, event: &Event, now: u64) -> Result<Reply> {
        let received = std::time::Instant::now();
        let opened =
            nostr::private_artifact::open(event, &self.secret).map_err(|e| e.to_string())?;
        if opened.recipient() != self.pubkey()
            || opened.body().issued_at > now.saturating_add(300)
            || opened.body().retain_until < now
        {
            return Err("not_admitted: media envelope".into());
        }
        let body = contracts::parse_strict_bounded(
            opened.inline_bytes().ok_or("malformed: inline frame")?,
            1_048_576,
        )
        .map_err(contract)?;
        let fields = [
            "v",
            "requires",
            "session",
            "plan",
            "track",
            "epoch",
            "nonce",
            "sequence",
            "token",
            "bytes",
            "direction",
        ];
        if body["v"] != "openagents.livehost-frame.v1"
            || body.as_object().is_none_or(|m| {
                m.len() != fields.len() || m.keys().any(|k| !fields.contains(&k.as_str()))
            })
            || body["requires"] != json!([])
        {
            return Err("malformed: media frame".into());
        }
        let bytes: Vec<u8> =
            serde_json::from_value(body["bytes"].clone()).map_err(|_| "malformed: media bytes")?;
        if bytes.len() > 8192 {
            return Err("limit_exceeded: media frame".into());
        }
        let input = workspace_host::artifact::json_ref(&body)?;
        let _guard = self.store.lock()?;
        let mut state = self.state()?;
        let id = body["session"].as_str().ok_or("malformed: media session")?;
        let session = state
            .sessions
            .get(id)
            .ok_or("not_admitted: media session")?;
        let participant = opened.signer();
        let member = session
            .members
            .get(participant)
            .ok_or("not_admitted: source participant")?;
        let track_id = body["track"].as_str().ok_or("malformed: media track")?;
        let track = session.track(track_id).ok_or("not_admitted: media track")?;
        let right = match body["direction"].as_str() {
            Some("input") => "publish",
            Some("playback") => "playback",
            _ => return Err("malformed: frame direction".into()),
        };
        if session.state != "active"
            || !state.policy.owners.contains(&session.owner)
            || body["plan"] != session.plan
            || body["epoch"] != session.epoch
            || member.expires_at <= now
            || !member.rights.iter().any(|r| r == right)
            || !member.tracks.iter().any(|t| t == track_id)
            || (right == "publish" && track["direction"] != "capture")
            || (right == "playback" && track["direction"] != "playback")
        {
            return Err("not_admitted: media scope".into());
        }
        let token = body["token"]
            .as_str()
            .ok_or("malformed: media credential")?;
        let grant =
            self.adapters
                .borrow()
                .broker
                .redeem(token, participant, track_id, right, now)?;
        if grant.session != id || grant.epoch != session.epoch {
            return Err("not_admitted: media credential scope".into());
        }
        let nonce = body["nonce"].as_str().ok_or("malformed: floor nonce")?;
        let floor = if right == "publish" {
            session.input_floors.values().find(|f| f.nonce == nonce)
        } else {
            session.speak_floors.get(track_id)
        }
        .ok_or("not_admitted: media floor")?;
        let expiry = floor.until.min(member.expires_at).min(
            session.body["expires_at"]
                .as_u64()
                .ok_or("malformed: plan expiry")?,
        );
        if floor.nonce != nonce
            || floor.participant != participant
            || floor.track != track_id
            || floor.epoch != session.epoch
            || expiry <= now
        {
            return Err("not_admitted: current media floor".into());
        }
        let installed = self.media_operation()?;
        if track["media"] != installed["media"]
            || !track["recipients"]
                .as_array()
                .is_some_and(|r| r.contains(&installed["destination"]))
        {
            return Err("not_admitted: native media recipient".into());
        }
        if right == "publish" {
            let lease = self.store.json(&floor.lease)?;
            if lease["destination"] != installed["destination"]
                || lease["processing"] != installed["operation"]
            {
                return Err("identity_mismatch: media processing".into());
            }
        }
        self.retention_consent(&state, session, track, &["record"], now)?;
        self.consented(
            &state,
            session,
            participant,
            &["record".into()],
            &[track_id.into()],
            &member.consent,
            now,
        )?;
        let retain_until = track["retention"]["until"]
            .as_u64()
            .ok_or("malformed: retention")?;
        let sequence = body["sequence"]
            .as_u64()
            .filter(|n| i128::from(*n) < contracts::SAFE_INTEGER)
            .ok_or("malformed: media sequence")?;
        let head = format!("{id}:{}:{right}:{nonce}", session.epoch);
        let key = format!("frame:{head}:{sequence}");
        if let Some(prior) = state.priors.get(&key) {
            if prior.fingerprint != digest_of(&input) {
                return Err("idempotency_conflict: media sequence".into());
            }
            return self.reply(participant, prior.result.clone(), BTreeMap::new(), now);
        }
        if sequence != state.media_heads.get(&head).copied().unwrap_or(0) {
            return Err("conflict: media sequence gap".into());
        }
        let ceiling = session.body["bounds"]
            .as_array()
            .into_iter()
            .flatten()
            .find(|b| b["bound"] == "storage_bytes")
            .and_then(|b| b["ceiling"].as_u64())
            .ok_or("cannot_enforce: media byte ceiling")?;
        if session.disclosed_bytes.saturating_add(bytes.len() as u64) > ceiling {
            return Err("limit_exceeded: media bytes".into());
        }
        let context = FrameContext {
            id: digest_of(&input).into(),
            session: id.into(),
            participant: participant.into(),
            epoch: session.epoch,
            sequence,
            deadline: received
                .checked_add(std::time::Duration::from_secs(
                    expiry.min(retain_until) - now,
                ))
                .ok_or("limit_exceeded: media deadline")?,
            retain_until,
        };
        let owner = session.owner.clone();
        let session = state.sessions.get_mut(id).ok_or("media session")?;
        let intent=self.record(session,"dispatched",json!({"request":digest_of(&input),"effect":if right=="publish" {"live-input-frame"} else {"live-playback-frame"}}),now)?;
        self.remember(&mut state, participant, &intent)?;
        self.remember(&mut state, &owner, &intent)?;
        let mut result = json!({"v":"openagents.livehost-frame-result.v1","request":input,"status":"unknown","dispatched":true,"sequence":sequence,"bytes":bytes.len(),"participant":participant,"session":id,"record":intent,"output":null});
        state.priors.insert(
            key.clone(),
            Prior {
                fingerprint: digest_of(&input).into(),
                result: result.clone(),
            },
        );
        state.media_heads.insert(head, sequence + 1);
        state
            .sessions
            .get_mut(id)
            .ok_or("media session")?
            .disclosed_bytes += bytes.len() as u64;
        self.save(&state)?;
        let delivered = self
            .sink
            .borrow_mut()
            .as_mut()
            .ok_or("unsupported_feature: media recipient")?
            .push(&context, &bytes);
        match delivered {
            Delivery::Delivered => {
                result["status"] = json!("accepted");
                result["output"] = json!({"delivered":true});
            }
            Delivery::Refused(reason) => {
                result["status"] = json!("refused");
                result["dispatched"] = json!(false);
                result["reason"] = json!(reason);
            }
            Delivery::Unknown => {}
        }
        let session = state.sessions.get_mut(id).ok_or("media session")?;
        let record = self.record(
            session,
            if result["status"] == "unknown" {
                "unknown"
            } else {
                "observed"
            },
            json!({"frame":digest_of(&input),"status":result["status"]}),
            now,
        )?;
        self.remember(&mut state, participant, &record)?;
        self.remember(&mut state, &owner, &record)?;
        result["record"] = record;
        state.priors.insert(
            key,
            Prior {
                fingerprint: digest_of(&input).into(),
                result: result.clone(),
            },
        );
        self.save(&state)?;
        self.reply(participant, result, BTreeMap::new(), now)
    }
}
