//! Pinned TLS and authenticated RPC channels. Only reads use the retry helper.
use crate::{Error, Result, macaroon::Macaroon};
use rustls::{
    DigitallySignedStruct, SignatureScheme,
    client::danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier},
    pki_types::{CertificateDer, ServerName, UnixTime},
};
use sha2::{Digest, Sha256};
use std::{sync::Arc, time::Duration};
use tonic::{
    service::interceptor::InterceptedService,
    transport::{Channel, Endpoint},
};
pub(crate) type AuthChannel = InterceptedService<Channel, Macaroon>;

#[derive(Clone, Debug)]
pub struct Config {
    pub endpoint: String,
    pub certificate_sha256: [u8; 32],
    pub timeout: Duration,
    /// An operator must opt in to using a different major/minor release.
    pub allow_version_mismatch: bool,
}
impl Config {
    pub fn new(endpoint: String, certificate_sha256: [u8; 32]) -> Self {
        Self {
            endpoint,
            certificate_sha256,
            timeout: Duration::from_secs(15),
            allow_version_mismatch: false,
        }
    }
}
#[derive(Debug)]
struct Pin {
    fingerprint: [u8; 32],
    mismatch: Arc<std::sync::atomic::AtomicBool>,
}
impl ServerCertVerifier for Pin {
    fn verify_server_cert(
        &self,
        end: &CertificateDer<'_>,
        _: &[CertificateDer<'_>],
        _: &ServerName<'_>,
        _: &[u8],
        _: UnixTime,
    ) -> std::result::Result<ServerCertVerified, rustls::Error> {
        if Sha256::digest(end.as_ref()).as_slice() != self.fingerprint {
            self.mismatch
                .store(true, std::sync::atomic::Ordering::Relaxed);
            return Err(rustls::Error::General("certificate pin mismatch".into()));
        }
        Ok(ServerCertVerified::assertion())
    }
    fn verify_tls12_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        sig: &DigitallySignedStruct,
    ) -> std::result::Result<HandshakeSignatureValid, rustls::Error> {
        rustls::crypto::verify_tls12_signature(
            message,
            cert,
            sig,
            &rustls::crypto::ring::default_provider().signature_verification_algorithms,
        )
    }
    fn verify_tls13_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        sig: &DigitallySignedStruct,
    ) -> std::result::Result<HandshakeSignatureValid, rustls::Error> {
        rustls::crypto::verify_tls13_signature(
            message,
            cert,
            sig,
            &rustls::crypto::ring::default_provider().signature_verification_algorithms,
        )
    }
    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        rustls::crypto::ring::default_provider()
            .signature_verification_algorithms
            .supported_schemes()
    }
}
pub(crate) async fn connect(config: &Config, macaroon: Macaroon) -> Result<AuthChannel> {
    // Tonic's URI uses HTTP internally because the connector below owns TLS.
    // Refuse plaintext at the public configuration boundary.
    let authority = config
        .endpoint
        .strip_prefix("https://")
        .ok_or(Error::Invalid("HTTPS endpoint"))?;
    let endpoint = Endpoint::from_shared(format!("http://{authority}"))
        .map_err(|_| Error::Invalid("endpoint"))?
        .connect_timeout(config.timeout);
    let uri = endpoint.uri();
    if uri.path() != "/"
        || uri.query().is_some()
        || uri.authority().is_none()
        || uri.authority().unwrap().as_str().contains('@')
    {
        return Err(Error::Invalid("endpoint"));
    }
    let hostname = uri.host().ok_or(Error::Invalid("hostname"))?.to_string();
    let server_name =
        ServerName::try_from(hostname.clone()).map_err(|_| Error::Invalid("hostname"))?;
    let port = uri.port_u16().unwrap_or(10009);
    let mismatch = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let mut tls = rustls::ClientConfig::builder_with_provider(Arc::new(
        rustls::crypto::ring::default_provider(),
    ))
    .with_safe_default_protocol_versions()
    .map_err(|_| Error::CertificatePin)?
    .dangerous()
    .with_custom_certificate_verifier(Arc::new(Pin {
        fingerprint: config.certificate_sha256,
        mismatch: mismatch.clone(),
    }))
    .with_no_client_auth();
    tls.alpn_protocols = vec![b"h2".to_vec()];
    let connector = tokio_rustls::TlsConnector::from(Arc::new(tls));
    let channel = endpoint
        .connect_with_connector(tower::service_fn(move |_| {
            let connector = connector.clone();
            let hostname = hostname.clone();
            let server_name = server_name.clone();
            async move {
                let tcp = tokio::net::TcpStream::connect((hostname.as_str(), port)).await?;
                let tls = connector.connect(server_name, tcp).await?;
                Ok::<_, std::io::Error>(hyper_util::rt::TokioIo::new(tls))
            }
        }))
        .await
        .map_err(|_| {
            if mismatch.load(std::sync::atomic::Ordering::Relaxed) {
                Error::CertificatePin
            } else {
                Error::Unavailable
            }
        })?;
    Ok(InterceptedService::new(channel, macaroon))
}
pub(crate) async fn read<T, F, Fut>(mut call: F) -> Result<T>
where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = std::result::Result<tonic::Response<T>, tonic::Status>>,
{
    for attempt in 0..3 {
        let response = tokio::time::timeout(Duration::from_secs(15), call())
            .await
            .map_err(|_| Error::Timeout)?;
        match response {
            Ok(response) => return Ok(response.into_inner()),
            Err(status) if status.code() == tonic::Code::Unavailable && attempt < 2 => {
                tokio::time::sleep(Duration::from_millis(50 << attempt)).await
            }
            Err(status) => return Err(status.into()),
        }
    }
    unreachable!()
}

pub(crate) async fn once<T>(
    call: impl std::future::Future<Output = std::result::Result<tonic::Response<T>, tonic::Status>>,
) -> Result<T> {
    tokio::time::timeout(Duration::from_secs(15), call)
        .await
        .map_err(|_| Error::Timeout)?
        .map(tonic::Response::into_inner)
        .map_err(Error::from)
}
