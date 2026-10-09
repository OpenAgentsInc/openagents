//! Dialing a `websocket` hint.
//!
//! A `ws` URL opens a plain TCP connection and a `wss` URL a TLS one, then
//! upgrades it with the direct channel's WebSocket limits. The channel
//! authenticates and encrypts itself either way. TLS adds compatibility with
//! clients and forwarders that require `wss`, and hides the channel
//! handshake's plaintext fields from observers on the path.
//!
//! A `wss` URL is verified with WebPKI: the chain must lead to a public root
//! in the bundled root set and the certificate must be valid for the URL's
//! host name. [`Tls::test_roots`] replaces that root set, for tests only.

use std::sync::Arc;
use std::time::Duration;

use coder_reach::websocket::{self, WebSocket};
use rustls::ClientConfig;
use rustls::pki_types::CertificateDer;
use rustls::pki_types::pem::PemObject;
use tokio::net::TcpStream;
use tokio_tungstenite::{Connector, MaybeTlsStream};

use crate::{Error, Result};

/// A WebSocket direct-channel stream, plain or TLS.
pub type Stream = WebSocket<MaybeTlsStream<TcpStream>>;

/// How a `wss` hint's certificate is verified.
#[derive(Clone, Default)]
pub struct Tls {
    /// `None` verifies against the bundled WebPKI roots.
    config: Option<Arc<ClientConfig>>,
}

impl std::fmt::Debug for Tls {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Tls")
            .field("test_roots", &self.config.is_some())
            .finish()
    }
}

impl Tls {
    /// Verify against the bundled WebPKI roots. This is the default.
    #[must_use]
    pub fn webpki() -> Self {
        Self::default()
    }

    /// For tests only: trust exactly the root certificates in `pem`, in
    /// place of the WebPKI roots. Name and validity checks still apply.
    /// An application never calls this.
    ///
    /// # Errors
    /// Refuses PEM without a usable root certificate.
    pub fn test_roots(pem: &[u8]) -> Result<Self> {
        let refuse = || Error::Config("the test roots hold no usable certificate".into());
        let mut roots = rustls::RootCertStore::empty();
        for certificate in CertificateDer::pem_slice_iter(pem) {
            roots
                .add(certificate.map_err(|_| refuse())?)
                .map_err(|_| refuse())?;
        }
        if roots.is_empty() {
            return Err(refuse());
        }
        let provider = Arc::new(rustls::crypto::ring::default_provider());
        let config = ClientConfig::builder_with_provider(provider)
            .with_safe_default_protocol_versions()
            .map_err(|_| refuse())?
            .with_root_certificates(roots)
            .with_no_client_auth();
        Ok(Self {
            config: Some(Arc::new(config)),
        })
    }
}

/// Connect and upgrade within `timeout`; `None` if the route does not answer.
pub(super) async fn dial(url: &str, tls: &Tls, timeout: Duration) -> Option<Stream> {
    connect(url, tls, timeout).await.ok()
}

/// Connect to a `ws` or `wss` URL and upgrade within `timeout`, verifying a
/// `wss` certificate as `tls` says. The stream then goes to
/// [`Link::direct`](super::Link::direct).
///
/// # Errors
/// Reports a timeout, a refused connection, a certificate that fails
/// verification, or a failed upgrade as a transport error.
pub async fn connect(url: &str, tls: &Tls, timeout: Duration) -> Result<Stream> {
    let connector = tls.config.clone().map(Connector::Rustls);
    let upgrade = tokio_tungstenite::connect_async_tls_with_config(
        url,
        Some(websocket::config()),
        // No Nagle: calls wait on their answers (`serve::direct`).
        true,
        connector,
    );
    let (socket, _) = tokio::time::timeout(timeout, upgrade)
        .await
        .map_err(|_| Error::Transport("the WebSocket connect timed out".into()))?
        .map_err(|error| Error::Transport(format!("WebSocket connect: {error}")))?;
    Ok(WebSocket::new(socket))
}
