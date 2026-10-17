//! Authenticated access to the app's existing Loro rooms on Coder sync.
//!
//! The caller supplies the service address and account session. Room bytes stay
//! opaque here; `openagents-sync` owns the shared room protocol and retry loop.

use reqwest::{Method, header::HeaderMap};
use url::Url;

use crate::{CancellationToken, Client, Error, Session};

pub const MAX_CHECKPOINT_BYTES: usize = 32 * 1024 * 1024;
pub const MAX_ROW_BYTES: usize = 1024 * 1024 - 4096;

/// One explicitly trusted sync service, bound to an authenticated account.
#[derive(Clone)]
pub struct Service {
    client: Client,
    session: Session,
    base: Url,
    cancel: CancellationToken,
}

impl std::fmt::Debug for Service {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Service")
            .field("account", &self.session.account().id)
            .finish_non_exhaustive()
    }
}

fn identifier(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"_-".contains(&b))
}

impl Service {
    pub fn new(
        client: Client,
        session: Session,
        base: &str,
        cancel: CancellationToken,
    ) -> Result<Self, Error> {
        client.check_session(&session)?;
        let base = Url::parse(base).map_err(|_| Error::InvalidDeployment)?;
        if !crate::transport::allowed(&base) || base.path() != "/" {
            return Err(Error::InvalidDeployment);
        }
        Ok(Self {
            client,
            session,
            base,
            cancel,
        })
    }

    pub fn room(&self, id: &str) -> Result<Room, Error> {
        if !identifier(id) {
            return Err(Error::InvalidReference);
        }
        Ok(Room {
            service: self.clone(),
            id: id.into(),
        })
    }

    fn url(&self, segments: &[&str]) -> Result<Url, Error> {
        self.client.check_session(&self.session)?;
        if self.cancel.is_cancelled() {
            return Err(Error::Cancelled);
        }
        let mut url = self.base.clone();
        url.path_segments_mut()
            .map_err(|_| Error::InvalidDeployment)?
            .pop_if_empty()
            .extend(segments.iter().copied());
        Ok(url)
    }

    fn socket(&self, segments: &[&str], device: &str) -> Result<String, Error> {
        if !identifier(device) {
            return Err(Error::InvalidReference);
        }
        let mut url = self.url(segments)?;
        let scheme = if url.scheme() == "https" { "wss" } else { "ws" };
        url.set_scheme(scheme)
            .map_err(|_| Error::InvalidDeployment)?;
        url.query_pairs_mut()
            .append_pair("token", self.session.bearer_token())
            .append_pair("device", device);
        Ok(url.into())
    }

    async fn request(
        &self,
        method: Method,
        url: Url,
        bytes: Option<Vec<u8>>,
        headers: HeaderMap,
        limit: usize,
    ) -> Result<Vec<u8>, Error> {
        tokio::select! {
            biased;
            _ = self.cancel.cancelled() => Err(Error::Cancelled),
            result = async {
                let response = self.client.room_request(&self.session, method, url, bytes, headers).await?;
                let status = response.status();
                if !status.is_success() {
                    if status.as_u16() == 404 { return Err(Error::NotFound); }
                    if status.as_u16() == 409 { return Err(Error::Changed); }
                    if status.as_u16() == 413 { return Err(Error::TooLarge); }
                    return Err(Error::Refused { status: status.as_u16(), code: None });
                }
                Client::read_limited(response, limit).await
            } => result,
        }
    }
}

/// A room reference cannot be moved to a different account or deployment.
#[derive(Clone, Debug)]
pub struct Room {
    service: Service,
    id: String,
}

impl Room {
    pub fn id(&self) -> &str {
        &self.id
    }

    pub fn socket(&self, device: &str) -> Result<String, Error> {
        self.service.socket(&["chat2", &self.id, "ws"], device)
    }

    fn url(&self, leaf: &str) -> Result<Url, Error> {
        self.service.url(&["chat2", &self.id, leaf])
    }

    pub async fn checkpoint(&self) -> Result<Vec<u8>, Error> {
        self.service
            .request(
                Method::GET,
                self.url("checkpoint")?,
                None,
                HeaderMap::new(),
                MAX_CHECKPOINT_BYTES,
            )
            .await
    }

    pub async fn rows(&self, after: u64) -> Result<Vec<u8>, Error> {
        let mut url = self.url("rows")?;
        url.query_pairs_mut()
            .append_pair("after", &after.to_string());
        // A pull is bounded separately from a single row (the server pages rows).
        self.service
            .request(
                Method::GET,
                url,
                None,
                HeaderMap::new(),
                MAX_CHECKPOINT_BYTES,
            )
            .await
    }

    pub async fn push(&self, device: &str, batch: &str, bytes: Vec<u8>) -> Result<String, Error> {
        if !identifier(device) || !identifier(batch) {
            return Err(Error::InvalidReference);
        }
        if bytes.is_empty() || bytes.len() > MAX_ROW_BYTES {
            return Err(Error::TooLarge);
        }
        let mut url = self.url("rows")?;
        url.query_pairs_mut().append_pair("batchId", batch);
        url.query_pairs_mut().append_pair("device", device);
        let response = self
            .service
            .request(Method::POST, url, Some(bytes), HeaderMap::new(), 4096)
            .await?;
        String::from_utf8(response).map_err(|_| Error::InvalidResponse)
    }
}
