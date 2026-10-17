use super::*;
use reqwest::{Method, Response, header};
use serde::Deserialize;
use std::collections::VecDeque;

const MAX_EVENT_BYTES: usize = 1024 * 1024;
pub struct Events {
    client: Client,
    document: DocumentRef,
    snapshot: Snapshot,
    cursor: u64,
    pending: VecDeque<Event>,
    response: Option<Response>,
    bytes: Vec<u8>,
    reconnects: u8,
}
impl fmt::Debug for Events {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("LabEvents")
            .field("cursor", &self.cursor)
            .finish_non_exhaustive()
    }
}
impl Client {
    pub fn events(&self, document: &DocumentRef, since: u64) -> Result<Events, Error> {
        valid_space(&document.space)?;
        valid_id(&document.id)?;
        Ok(Events {
            client: self.clone(),
            document: document.clone(),
            snapshot: self.bound(&document.binding)?,
            cursor: since,
            pending: VecDeque::new(),
            response: None,
            bytes: Vec::new(),
            reconnects: 0,
        })
    }
}
impl Events {
    /// The last event delivered to the caller, suitable for caller-owned storage.
    pub fn cursor(&self) -> u64 {
        self.cursor
    }
    pub async fn next(&mut self, cancel: &CancellationToken) -> Result<Event, Error> {
        loop {
            if self.snapshot.invalidated.is_cancelled() {
                return Err(Error::AccountChanged);
            }
            if cancel.is_cancelled() {
                return Err(Error::Cancelled);
            }
            if let Some(event) = self.pending.pop_front() {
                self.cursor = event.cursor;
                return Ok(event);
            }
            if self.response.is_none() {
                if self.reconnects > 0 {
                    let delay = Duration::from_millis(100)
                        .saturating_mul(1u32 << self.reconnects.min(6))
                        .min(Duration::from_secs(10));
                    wire::guarded(&self.snapshot, cancel, async {
                        tokio::time::sleep(delay).await;
                        Ok(())
                    })
                    .await?;
                }
                let query = [
                    ("space", self.document.space.clone()),
                    ("since", self.cursor.to_string()),
                ];
                let request = self
                    .client
                    .request(
                        &self.snapshot,
                        Method::GET,
                        &["labs", "documents", &self.document.id, "events"],
                        &query,
                        None,
                    )?
                    .header(header::ACCEPT, "text/event-stream")
                    .header("Last-Event-ID", self.cursor.to_string())
                    .timeout(Duration::from_secs(310));
                let response = wire::guarded(&self.snapshot, cancel, async {
                    let response = request.send().await.map_err(|e| crate::Error::Network {
                        timeout: e.is_timeout(),
                    })?;
                    wire::checked(response).await
                })
                .await;
                match response {
                    Ok(response) => {
                        if !response
                            .headers()
                            .get(header::CONTENT_TYPE)
                            .and_then(|h| h.to_str().ok())
                            .is_some_and(|s| {
                                s.split(';')
                                    .next()
                                    .is_some_and(|v| v.trim() == "text/event-stream")
                            })
                        {
                            return Err(Error::InvalidResponse);
                        }
                        self.response = Some(response);
                        self.bytes.clear();
                    }
                    Err(error) if wire::retryable(&error) => {
                        if let Some(delay) = error.retry_after() {
                            if delay > self.client.retries.max_delay {
                                return Err(error);
                            }
                            wire::guarded(&self.snapshot, cancel, async {
                                tokio::time::sleep(delay).await;
                                Ok(())
                            })
                            .await?;
                        }
                        self.reconnects = self.reconnects.saturating_add(1);
                        continue;
                    }
                    Err(error) => return Err(error),
                }
            }
            let chunk = wire::guarded(&self.snapshot, cancel, async {
                self.response.as_mut().unwrap().chunk().await.map_err(|e| {
                    crate::Error::Network {
                        timeout: e.is_timeout(),
                    }
                    .into()
                })
            })
            .await;
            match chunk {
                Ok(Some(chunk)) => {
                    for byte in chunk {
                        self.bytes.push(byte);
                        if self.bytes.len() > MAX_EVENT_BYTES {
                            return Err(Error::TooLarge);
                        }
                        if self.bytes.ends_with(b"\n\n") || self.bytes.ends_with(b"\r\n\r\n") {
                            self.parse()?;
                            self.bytes.clear();
                        }
                    }
                }
                Ok(None) | Err(Error::Session(crate::Error::Network { .. })) => {
                    self.response = None;
                    self.bytes.clear();
                    self.reconnects = self.reconnects.saturating_add(1);
                }
                Err(error) => {
                    self.response = None;
                    return Err(error);
                }
            }
        }
    }
    fn parse(&mut self) -> Result<(), Error> {
        let text = std::str::from_utf8(&self.bytes).map_err(|_| Error::InvalidResponse)?;
        let mut data = String::new();
        let mut id = None;
        for line in text.lines() {
            let Some((field, value)) = line.split_once(':') else {
                continue;
            };
            let value = value.strip_prefix(' ').unwrap_or(value);
            match field {
                "data" => {
                    if !data.is_empty() {
                        data.push('\n');
                    }
                    data.push_str(value);
                }
                "id" => id = Some(value.parse::<u64>().map_err(|_| Error::InvalidResponse)?),
                _ => {}
            }
        }
        if data.is_empty() {
            return Ok(());
        }
        #[derive(Deserialize)]
        struct Batch {
            data: Vec<Event>,
        }
        let batch: Batch = serde_json::from_str(&data).map_err(|_| Error::InvalidResponse)?;
        if batch.data.len() > 100 || self.pending.len() + batch.data.len() > 1000 {
            return Err(Error::TooLarge);
        }
        let mut after = self
            .pending
            .back()
            .map_or(self.cursor, |event| event.cursor);
        for event in &batch.data {
            if event.document != self.document.id || event.cursor <= after {
                return Err(Error::InvalidResponse);
            }
            after = event.cursor;
        }
        if id.is_some_and(|id| id != after) {
            return Err(Error::InvalidResponse);
        }
        self.pending.extend(batch.data);
        self.reconnects = 0;
        Ok(())
    }
}
