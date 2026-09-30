//! TLS for the WebSocket listener, from the operator's certificate chain and
//! private key.
//!
//! The host reads both files once, at start, and refuses to serve when:
//!
//! - The key file is missing, unreadable, not a regular file, not owned by
//!   this user, or open to group or others.
//! - Either file holds no usable PEM item.
//! - The key does not match the chain's leaf certificate.
//! - The leaf certificate is not valid for the configured name.
//!
//! The host does not check the chain against any root or the certificate's
//! validity period; a client does both when it dials. To rotate the files,
//! replace them and restart the host.

#[cfg(unix)]
use std::os::unix::fs::MetadataExt;
use std::path::Path;
use std::sync::Arc;

use rustls::ServerConfig;
use rustls::client::verify_server_name;
use rustls::crypto::CryptoProvider;
use rustls::pki_types::pem::PemObject;
use rustls::pki_types::{CertificateDer, PrivateKeyDer, ServerName};
use rustls::server::ParsedCertificate;
use rustls::sign::CertifiedKey;
use tokio_rustls::TlsAcceptor;

use crate::config::WebsocketTls;
use crate::{Error, Result};

/// Load and check the operator's files and return the listener's acceptor.
///
/// # Errors
/// Refuses each condition in the module documentation.
pub fn acceptor(tls: &WebsocketTls) -> Result<TlsAcceptor> {
    let key = read_key(&tls.key)?;
    let chain = read_chain(&tls.cert)?;
    let provider = Arc::new(rustls::crypto::ring::default_provider());
    check_pair(&chain, &key, &provider)?;
    check_name(&chain, &tls.name)?;
    let mut config = ServerConfig::builder_with_provider(provider)
        .with_safe_default_protocol_versions()
        .map_err(|_| refuse("the TLS provider offers no safe protocol version"))?
        .with_no_client_auth()
        .with_single_cert(chain, key)
        .map_err(|_| refuse("the WebSocket TLS certificate and key cannot be used"))?;
    // A WebSocket upgrade is an HTTP/1.1 request.
    config.alpn_protocols = vec![b"http/1.1".to_vec()];
    Ok(TlsAcceptor::from(Arc::new(config)))
}

/// Read the private key after checking who can read the file.
fn read_key(path: &Path) -> Result<PrivateKeyDer<'static>> {
    let shown = path.display();
    let metadata = std::fs::metadata(path)
        .map_err(|_| refuse(&format!("the WebSocket TLS key {shown} cannot be read")))?;
    if !metadata.is_file() {
        return Err(refuse(&format!(
            "the WebSocket TLS key {shown} is not a regular file"
        )));
    }
    #[cfg(unix)]
    {
        // SAFETY: `geteuid` has no preconditions and cannot fail.
        let user = unsafe { libc::geteuid() };
        if metadata.uid() != user {
            return Err(refuse(&format!(
                "the WebSocket TLS key {shown} is not owned by this user"
            )));
        }
        if metadata.mode() & 0o077 != 0 {
            return Err(refuse(&format!(
                "the WebSocket TLS key {shown} is open to group or others; run chmod 600 on it"
            )));
        }
    }
    #[cfg(windows)]
    if !private_fs::is_private_path(path).unwrap_or(false) {
        return Err(refuse(&format!(
            "the WebSocket TLS key {shown} must be owned by this user and open to no other; run icacls on it with /inheritance:r /grant:r and your user name"
        )));
    }
    let bytes = std::fs::read(path)
        .map_err(|_| refuse(&format!("the WebSocket TLS key {shown} cannot be read")))?;
    PrivateKeyDer::from_pem_slice(&bytes).map_err(|_| {
        refuse(&format!(
            "the WebSocket TLS key {shown} holds no PEM private key"
        ))
    })
}

/// Read the certificate chain, leaf first.
fn read_chain(path: &Path) -> Result<Vec<CertificateDer<'static>>> {
    let shown = path.display();
    let bytes = std::fs::read(path).map_err(|_| {
        refuse(&format!(
            "the WebSocket TLS certificate {shown} cannot be read"
        ))
    })?;
    let chain = CertificateDer::pem_slice_iter(&bytes)
        .collect::<std::result::Result<Vec<_>, _>>()
        .map_err(|_| {
            refuse(&format!(
                "the WebSocket TLS certificate {shown} is not valid PEM"
            ))
        })?;
    if chain.is_empty() {
        return Err(refuse(&format!(
            "the WebSocket TLS certificate {shown} holds no certificate"
        )));
    }
    Ok(chain)
}

/// Refuse a key that does not belong to the leaf certificate.
fn check_pair(
    chain: &[CertificateDer<'static>],
    key: &PrivateKeyDer<'static>,
    provider: &CryptoProvider,
) -> Result<()> {
    let signer = provider
        .key_provider
        .load_private_key(key.clone_key())
        .map_err(|_| refuse("the WebSocket TLS key is not a supported private key"))?;
    CertifiedKey::new(chain.to_vec(), signer)
        .keys_match()
        .map_err(|_| refuse("the WebSocket TLS key does not match the certificate"))
}

/// Refuse a leaf certificate that is not valid for the name clients dial.
fn check_name(chain: &[CertificateDer<'static>], name: &str) -> Result<()> {
    let leaf = chain
        .first()
        .ok_or_else(|| refuse("the WebSocket TLS certificate holds no certificate"))?;
    let parsed = ParsedCertificate::try_from(leaf)
        .map_err(|_| refuse("the WebSocket TLS certificate cannot be parsed"))?;
    let server =
        ServerName::try_from(name).map_err(|_| refuse("--websocket-name takes a DNS name"))?;
    verify_server_name(&parsed, &server).map_err(|_| {
        refuse(&format!(
            "the WebSocket TLS certificate is not valid for {name}"
        ))
    })
}

fn refuse(message: &str) -> Error {
    Error::Config(message.to_owned())
}
