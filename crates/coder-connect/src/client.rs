//! Portable client. It never reads desktop roots or acquires an engine credential.
use crate::{Error, ErrorCode, Result, fail, protocol::*, transport, unix_time};
use base64::{Engine, engine::general_purpose::STANDARD};
use nostr::domain::Event;
use secp256k1::SecretKey;
use serde::{Deserialize, Serialize};

pub struct Client {
    code: ConnectionCode,
    secret: SecretKey,
    policy: RelayPolicy,
    connection: tokio::sync::Mutex<Option<transport::Session>>,
}

/// Retaining this exact packet permits a bounded retry without a new read ID.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Pending {
    pub request: Request,
    pub event: Event,
}

impl Client {
    pub fn new(code: ConnectionCode, secret: SecretKey) -> Result<Self> {
        Self::new_with_policy(code, secret, RelayPolicy::Production)
    }
    pub fn new_with_policy(
        code: ConnectionCode,
        secret: SecretKey,
        policy: RelayPolicy,
    ) -> Result<Self> {
        code.verify(&secret, unix_time()?, policy)?;
        Ok(Self {
            code,
            secret,
            policy,
            connection: tokio::sync::Mutex::new(None),
        })
    }
    pub fn connection(&self) -> &ConnectionCode {
        &self.code
    }
    pub fn prepare(&self, query: Query, now: u64) -> Result<Pending> {
        self.code.verify(&self.secret, now, self.policy)?;
        query.validate()?;
        let request = Request {
            v: REQUEST.into(),
            requires: vec![],
            request: random_id(),
            grant: self.code.grant.clone(),
            authorization: self.code.authorization.id.clone(),
            issued_at: now,
            expires_at: self
                .code
                .expires_at
                .min(now.saturating_add(MAX_REQUEST_LIFETIME)),
            query,
        };
        request.validate()?;
        let event = seal(
            &request,
            REQUEST,
            &self.secret,
            &self.code.host,
            &request.request,
            now,
            request.expires_at,
        )?;
        Ok(Pending { request, event })
    }
    pub async fn observe(&self, query: Query) -> Result<Observation> {
        let pending = self.prepare(query, unix_time()?)?;
        self.send(&pending).await
    }
    pub async fn send(&self, pending: &Pending) -> Result<Observation> {
        self.check_pending(pending, unix_time()?)?;
        let mut slot = self.connection.lock().await;
        self.check_pending(pending, unix_time()?)?;
        // Take ownership before awaiting. Cancellation or any error drops the
        // uncertain socket; only a completely checked exchange can reuse it.
        let previous = slot.take().filter(transport::Session::reusable);
        let (session, response) = tokio::time::timeout(std::time::Duration::from_secs(8), async {
            let mut session = match previous {
                Some(session) => session,
                None => {
                    transport::Session::connect(&self.code.relay, &self.secret, self.policy).await?
                }
            };
            let response = session
                .exchange(pending, &self.code.host, &self.code.client)
                .await?;
            Ok::<_, Error>((session, response))
        })
        .await
        .map_err(|_| Error::new(ErrorCode::Transport, "observation deadline exceeded"))??;
        let observation = self.verify_reply(pending, &response, unix_time()?);
        if observation.is_ok() {
            *slot = Some(session);
        }
        observation
    }
    fn check_pending(&self, pending: &Pending, now: u64) -> Result<()> {
        self.code.verify(&self.secret, now, self.policy)?;
        pending.request.validate()?;
        fresh(pending.request.issued_at, pending.request.expires_at, now)?;
        let original: Request = open(
            &pending.event,
            &self.secret,
            &self.code.client,
            &self.code.host,
            REQUEST,
        )?;
        if encoded(&original)? != encoded(&pending.request)?
            || original.grant != self.code.grant
            || original.authorization != self.code.authorization.id
            || original.expires_at > self.code.expires_at
            || pending.event.tag_values("h").collect::<Vec<_>>() != [original.request.as_str()]
        {
            return fail(
                ErrorCode::Forbidden,
                "pending packet differs from the exact connection request",
            );
        }
        Ok(())
    }
    pub fn verify_reply(&self, pending: &Pending, event: &Event, now: u64) -> Result<Observation> {
        self.check_pending(pending, now)?;
        let reply: Reply = open(
            event,
            &self.secret,
            &self.code.host,
            &self.code.client,
            REPLY,
        )?;
        schema(&reply.v, REPLY, &reply.requires)?;
        window(reply.issued_at, reply.expires_at, MAX_REQUEST_LIFETIME)?;
        fresh(reply.issued_at, reply.expires_at, now)?;
        if reply.request != pending.request.request
            || reply.request_event != pending.event.id
            || reply.grant != self.code.grant
            || reply.issued_at < pending.request.issued_at
            || reply.expires_at > pending.request.expires_at
            || event.tag_values("h").collect::<Vec<_>>() != [reply.request.as_str()]
        {
            return fail(
                ErrorCode::Forbidden,
                "reply does not bind the exact pending request",
            );
        }
        match reply.result {
            ReplyResult::Refused {
                code: ErrorCode::Transport,
            } => fail(
                ErrorCode::Malformed,
                "remote transport refusal is not a domain result",
            ),
            ReplyResult::Refused { code } => Err(Error::new(code, "host refused this observation")),
            ReplyResult::Ok { observation } => {
                check_observation(&pending.request.query, &observation)?;
                Ok(*observation)
            }
        }
    }
}

pub(crate) fn check_observation(query: &Query, observation: &Observation) -> Result<()> {
    let page_bytes = match observation {
        Observation::Catalog(page) => encoded(page)?,
        Observation::Page(page) => encoded(page)?,
    };
    if page_bytes.len() > coder_history::MAX_RESPONSE_BYTES {
        return fail(
            ErrorCode::Bounds,
            "history page exceeds the negotiated bound",
        );
    }
    match (query, observation) {
        (Query::Catalog(request), Observation::Catalog(page)) => {
            if page.entries.len() > usize::from(request.limit) || page.notices.len() > 256 {
                return fail(
                    ErrorCode::Bounds,
                    "catalog page exceeds the requested bound",
                );
            }
        }
        (Query::Page(request), Observation::Page(page)) => {
            if page.source_id != request.source_id
                || page.next.source_id != request.source_id
                || page.next.incarnation != page.incarnation
                || page.next.offset > page.snapshot_bytes
                || page.next.record_offset > page.next.offset
                || page.chunks.len() > 128
            {
                return fail(
                    ErrorCode::Malformed,
                    "transcript page identity or progress differs",
                );
            }
            let mut offset = request.cursor.as_ref().map_or(0, |c| c.offset);
            let mut record_offset = request.cursor.as_ref().map_or(0, |c| c.record_offset);
            let mut record_index = request.cursor.as_ref().map_or(0, |c| c.record_index);
            let start = offset;
            if request
                .cursor
                .as_ref()
                .is_some_and(|c| c.incarnation != page.incarnation)
            {
                return fail(ErrorCode::SourceChanged, "source incarnation changed");
            }
            for chunk in &page.chunks {
                if chunk.offset != offset
                    || chunk.end_offset <= chunk.offset
                    || chunk.end_offset > page.snapshot_bytes
                    || chunk.record_offset != record_offset
                    || chunk.index != record_index
                    || chunk.id
                        != coder_history::record_id(
                            &page.source_id,
                            &page.incarnation,
                            record_offset,
                        )
                    || chunk.raw_base64.len() > 4 * coder_history::MAX_CHUNK_BYTES / 3 + 4
                {
                    return fail(
                        ErrorCode::Malformed,
                        "transcript chunks are discontinuous or exceed their source cut",
                    );
                }
                let raw = STANDARD.decode(&chunk.raw_base64).map_err(|_| {
                    Error::new(
                        ErrorCode::Malformed,
                        "transcript chunk is not canonical base64",
                    )
                })?;
                if raw.len() as u64 != chunk.end_offset - chunk.offset
                    || raw.len() > coder_history::MAX_CHUNK_BYTES
                    || STANDARD.encode(&raw) != chunk.raw_base64
                    || chunk.complete != raw.ends_with(b"\n")
                    || raw[..raw.len().saturating_sub(1)].contains(&b'\n')
                {
                    return fail(
                        ErrorCode::Malformed,
                        "transcript bytes differ from their bounds or record completion",
                    );
                }
                offset = chunk.end_offset;
                if chunk.complete {
                    record_offset = offset;
                    record_index = record_index.checked_add(1).ok_or_else(|| {
                        Error::new(
                            ErrorCode::Bounds,
                            "transcript record index exceeds its bound",
                        )
                    })?;
                }
            }
            if page.next.offset != offset
                || page.next.record_offset != record_offset
                || page.next.record_index != record_index
                || page.pending_line != (record_offset < offset)
                || page.has_more != (offset < page.snapshot_bytes)
                || offset.saturating_sub(start) > u64::from(request.max_bytes)
            {
                return fail(
                    ErrorCode::Bounds,
                    "transcript cursor advances beyond returned bounded bytes",
                );
            }
        }
        _ => {
            return fail(
                ErrorCode::Malformed,
                "reply variant differs from requested observation",
            );
        }
    }
    Ok(())
}
