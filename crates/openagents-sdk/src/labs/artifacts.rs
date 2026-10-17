//! Lab file metadata and checked file downloads.
use super::*;
use reqwest::{Method, Response, header};
use sha2::{Digest, Sha256};
use std::ops::Range;

/// A lab file's details and its download link.
pub struct ArtifactHandle {
    binding: Binding,
    space: String,
    link: lab_kit::api::ArtifactLink,
    invalidated: CancellationToken,
    received: tokio::time::Instant,
}
impl ArtifactHandle {
    /// Returns the file's details, such as its name, size, and checksum.
    pub fn metadata(&self) -> &Artifact {
        &self.link.artifact
    }
    /// A public signed link, without an account bearer. Refresh metadata after expiry.
    pub fn signed_url(&self) -> Option<&str> {
        self.link.expires_seconds.map(|_| self.link.url.as_str())
    }
    /// Returns how many seconds the signed link stays valid, if it is signed.
    pub fn expires_seconds(&self) -> Option<u32> {
        self.link.expires_seconds
    }
}
impl fmt::Debug for ArtifactHandle {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("LabArtifact").finish_non_exhaustive()
    }
}

/// Read to EOF before accepting a complete download. Full reads verify the
/// file digest; ranged reads verify the server's range and exact byte count.
pub struct ArtifactDownload {
    response: Option<Response>,
    snapshot: Snapshot,
    expected: u64,
    received: u64,
    digest: Option<Sha256>,
    expected_digest: String,
}
impl fmt::Debug for ArtifactDownload {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("LabArtifactDownload")
            .finish_non_exhaustive()
    }
}
impl ArtifactDownload {
    /// Reads the next part of the file, or `None` when the whole file has arrived.
    pub async fn next_chunk(
        &mut self,
        cancel: &CancellationToken,
    ) -> Result<Option<Vec<u8>>, Error> {
        let Some(response) = self.response.as_mut() else {
            return Ok(None);
        };
        let chunk = wire::guarded(&self.snapshot, cancel, async {
            response.chunk().await.map_err(|e| {
                crate::Error::Network {
                    timeout: e.is_timeout(),
                }
                .into()
            })
        })
        .await;
        match chunk {
            Err(error) => {
                self.response = None;
                Err(error)
            }
            Ok(Some(chunk)) => {
                if chunk.len() as u64 > self.expected.saturating_sub(self.received) {
                    self.response = None;
                    return Err(Error::TooLarge);
                }
                self.received += chunk.len() as u64;
                if let Some(digest) = &mut self.digest {
                    digest.update(&chunk);
                }
                Ok(Some(chunk.to_vec()))
            }
            Ok(None) => {
                self.response = None;
                if self.received != self.expected {
                    return Err(Error::InvalidResponse);
                }
                if let Some(digest) = self.digest.take() {
                    if format!("{:x}", digest.finalize()) != self.expected_digest {
                        return Err(Error::InvalidResponse);
                    }
                }
                Ok(None)
            }
        }
    }
}
impl Client {
    /// Looks up a lab file in a space and returns its details and link.
    pub async fn artifact(
        &self,
        space: &str,
        id: &str,
        cancel: &CancellationToken,
    ) -> Result<ArtifactHandle, Error> {
        valid_space(space)?;
        valid_id(id)?;
        let snapshot = self.snapshot()?;
        let link: lab_kit::api::ArtifactLink = self
            .json(
                &snapshot,
                Method::GET,
                &["labs", "artifacts", id],
                &[("space", space.into())],
                None,
                None,
                cancel,
            )
            .await?;
        if link.artifact.id != id
            || link.artifact.sha256.len() != 64
            || !link.artifact.sha256.bytes().all(|b| b.is_ascii_hexdigit())
            || link
                .expires_seconds
                .is_some_and(|seconds| seconds == 0 || seconds > 3600)
        {
            return Err(Error::InvalidResponse);
        }
        Ok(ArtifactHandle {
            binding: snapshot.binding(),
            space: space.into(),
            link,
            invalidated: snapshot.invalidated,
            received: tokio::time::Instant::now(),
        })
    }
    /// Starts downloading a lab file, or only a byte range of it.
    pub async fn download(
        &self,
        artifact: &ArtifactHandle,
        range: Option<Range<u64>>,
        cancel: &CancellationToken,
    ) -> Result<ArtifactDownload, Error> {
        let snapshot = self.bound(&artifact.binding)?;
        if artifact.invalidated.is_cancelled() {
            return Err(Error::AccountChanged);
        }
        if artifact.link.expires_seconds.is_some_and(|seconds| {
            artifact.received.elapsed() >= Duration::from_secs(seconds as u64)
        }) {
            return Err(Error::ExpiredLink);
        }
        let size = artifact.link.artifact.size;
        let (expected, expected_range) = match &range {
            Some(range) if range.start < range.end && range.end <= size => (
                range.end - range.start,
                Some(format!("bytes {}-{}/{}", range.start, range.end - 1, size)),
            ),
            Some(_) => return Err(Error::InvalidInput),
            None => (size, None),
        };
        let url = self
            .transport
            .deployment()
            .api_base()
            .join(&artifact.link.url)
            .map_err(|_| Error::InvalidResponse)?;
        if url.username() != "" || url.password().is_some() || url.fragment().is_some() {
            return Err(Error::InvalidResponse);
        }
        let mut request = if artifact.link.expires_seconds.is_none() {
            let segments = [
                "labs",
                "artifacts",
                artifact.link.artifact.id.as_str(),
                "content",
            ];
            let query = [("space", artifact.space.clone())];
            let expected_url = self.url(&segments, &query)?;
            if url.origin() != expected_url.origin()
                || url.path() != expected_url.path()
                || url.query_pairs().collect::<Vec<_>>()
                    != vec![("space".into(), artifact.space.as_str().into())]
            {
                return Err(Error::InvalidResponse);
            }
            self.request(&snapshot, Method::GET, &segments, &query, None)?
        } else {
            let loopback = url.host_str().is_some_and(|host| {
                host == "localhost"
                    || host
                        .trim_matches(['[', ']'])
                        .parse::<std::net::IpAddr>()
                        .is_ok_and(|ip| ip.is_loopback())
            });
            if url.scheme() != "https"
                && !(url.scheme() == "http"
                    && loopback
                    && self.transport.deployment().api_base().scheme() == "http")
            {
                return Err(Error::InvalidResponse);
            }
            // Signed file links never receive the account credential.
            self.transport.http.get(url)
        };
        if let Some(range) = &range {
            request = request.header(
                header::RANGE,
                format!("bytes={}-{}", range.start, range.end - 1),
            );
        }
        let response = wire::guarded(&snapshot, cancel, async {
            let response = request.send().await.map_err(|e| crate::Error::Network {
                timeout: e.is_timeout(),
            })?;
            wire::checked(response).await
        })
        .await?;
        if response.status().as_u16() != if range.is_some() { 206 } else { 200 }
            || expected_range.as_ref().is_some_and(|wanted| {
                response
                    .headers()
                    .get(header::CONTENT_RANGE)
                    .and_then(|h| h.to_str().ok())
                    != Some(wanted.as_str())
            })
            || response
                .content_length()
                .is_some_and(|length| length != expected)
        {
            return Err(Error::InvalidResponse);
        }
        Ok(ArtifactDownload {
            response: Some(response),
            snapshot,
            expected,
            received: 0,
            digest: range.is_none().then(Sha256::new),
            expected_digest: artifact.link.artifact.sha256.clone(),
        })
    }
}
