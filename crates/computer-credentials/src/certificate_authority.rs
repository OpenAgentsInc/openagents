//! Google CA signing for the private native authority. No root signing key is
//! exported, and the authority credential is separate from every node token.
use crate::{
    Failure, Secret,
    secret_manager::{AccessToken, MetadataToken},
};
use axum::{
    Json, Router,
    extract::{DefaultBodyLimit, State},
    http::{HeaderMap, StatusCode, header},
    response::{IntoResponse, Response},
    routing::post,
};
use reqwest::{Client, Response as HttpResponse};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    sync::Arc,
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use subtle::ConstantTimeEq;
use tokio::sync::Semaphore;

const MAX_RESPONSE: usize = 262144;
pub const ISSUE_PATH: &str = "/v1/certificates/issue";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Authority {
    name: String,
    pool: String,
    id: String,
}
impl Authority {
    pub fn parse(name: &str) -> Result<Self, Failure> {
        let p: Vec<_> = name.split('/').collect();
        let identifier = |s: &str| {
            !s.is_empty()
                && s.len() <= 63
                && s.bytes()
                    .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
        };
        if p.len() != 8
            || p[0] != "projects"
            || p[2] != "locations"
            || p[4] != "caPools"
            || p[6] != "certificateAuthorities"
            || !p[1]
                .parse::<u64>()
                .is_ok_and(|n| n > 0 && n.to_string() == p[1])
            || [p[3], p[5], p[7]].iter().any(|s| !identifier(s))
        {
            return Err(Failure::Invalid);
        }
        Ok(Self {
            name: name.into(),
            pool: p[..6].join("/"),
            id: p[7].into(),
        })
    }
}
#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Request {
    csr_pem: String,
    lifetime_seconds: u64,
}
impl Request {
    fn validate(&self) -> Result<(), Failure> {
        if !(1..=604800).contains(&self.lifetime_seconds)
            || self.csr_pem.len() > 12000
            || !self
                .csr_pem
                .starts_with("-----BEGIN CERTIFICATE REQUEST-----\n")
            || !self
                .csr_pem
                .ends_with("-----END CERTIFICATE REQUEST-----\n")
            || !self
                .csr_pem
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"-+/=\n ".contains(&b))
        {
            return Err(Failure::Invalid);
        }
        Ok(())
    }
}
fn http() -> Result<Client, Failure> {
    Client::builder()
        .https_only(true)
        .no_proxy()
        .redirect(reqwest::redirect::Policy::none())
        .connect_timeout(Duration::from_secs(5))
        .timeout(Duration::from_secs(15))
        .build()
        .map_err(|_| Failure::Unavailable)
}
async fn bounded(mut response: HttpResponse) -> Result<Vec<u8>, Failure> {
    let mut bytes = Vec::new();
    while let Some(part) = response.chunk().await.map_err(|_| Failure::Unavailable)? {
        if part.len() > MAX_RESPONSE.saturating_sub(bytes.len()) {
            return Err(Failure::Unavailable);
        }
        bytes.extend_from_slice(&part);
    }
    Ok(bytes)
}
fn certificate(pem: String) -> Result<String, Failure> {
    if pem.len() > 32768
        || !pem.starts_with("-----BEGIN CERTIFICATE-----\n")
        || !pem.trim_end().ends_with("-----END CERTIFICATE-----")
        || !pem
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"-+/=\r\n ".contains(&b))
    {
        return Err(Failure::Unavailable);
    }
    Ok(pem)
}
fn request_id(authority: &Authority, request: &Request, day: u64) -> String {
    let mut h = Sha256::new();
    h.update(b"openagents.computers.node-ca.v1\0");
    h.update(authority.name.as_bytes());
    h.update([0]);
    h.update(request.csr_pem.as_bytes());
    h.update(request.lifetime_seconds.to_be_bytes());
    h.update(day.to_be_bytes());
    let digest = h.finalize();
    let mut bytes = [0; 16];
    bytes.copy_from_slice(&digest[..16]);
    bytes[6] = (bytes[6] & 15) | 0x50;
    bytes[8] = (bytes[8] & 63) | 0x80;
    uuid::Uuid::from_bytes(bytes).to_string()
}

pub struct GoogleCa {
    http: Client,
    authority: Authority,
    tokens: Arc<dyn AccessToken>,
}
impl GoogleCa {
    pub fn from_metadata(authority: &str) -> Result<Self, Failure> {
        Ok(Self {
            http: http()?,
            authority: Authority::parse(authority)?,
            tokens: Arc::new(MetadataToken::new()?),
        })
    }
    async fn issue(&self, request: &Request) -> Result<String, Failure> {
        request.validate()?;
        let day = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|_| Failure::Unavailable)?
            .as_secs()
            / 86400;
        let id = request_id(&self.authority, request, day);
        let token = self.tokens.token().await?;
        let endpoint = format!(
            "https://privateca.googleapis.com/v1/{}/certificates",
            self.authority.pool
        );
        let body = serde_json::json!({
            "pemCsr": request.csr_pem,
            "lifetime": format!("{}s", request.lifetime_seconds),
        });
        let response = self
            .http
            .post(endpoint)
            .query(&[
                ("issuingCertificateAuthorityId", self.authority.id.as_str()),
                ("requestId", id.as_str()),
            ])
            .bearer_auth(std::str::from_utf8(token.bytes()).map_err(|_| Failure::Unavailable)?)
            .json(&body)
            .send()
            .await
            .map_err(|_| Failure::Unavailable)?;
        if !response.status().is_success() {
            return Err(Failure::Unavailable);
        }
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct Certificate {
            pem_certificate: String,
            issuer_certificate_authority: String,
        }
        let result: Certificate =
            serde_json::from_slice(&bounded(response).await?).map_err(|_| Failure::Unavailable)?;
        if result.issuer_certificate_authority != self.authority.name {
            return Err(Failure::Unavailable);
        }
        certificate(result.pem_certificate)
    }
}

pub struct Service {
    issuer: GoogleCa,
    caller: [u8; 32],
    slots: Semaphore,
}
impl Service {
    pub fn new(issuer: GoogleCa, caller: Secret) -> Result<Self, Failure> {
        if !(32..=8192).contains(&caller.bytes().len()) {
            return Err(Failure::Invalid);
        }
        Ok(Self {
            issuer,
            caller: Sha256::digest(caller.bytes()).into(),
            slots: Semaphore::new(2),
        })
    }
    pub fn router(self: Arc<Self>) -> Router {
        Router::new()
            .route(ISSUE_PATH, post(issue))
            .layer(DefaultBodyLimit::max(16384))
            .with_state(self)
    }
}
async fn issue(
    State(s): State<Arc<Service>>,
    headers: HeaderMap,
    Json(request): Json<Request>,
) -> Response {
    let token = headers
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.strip_prefix("Bearer "));
    if headers.get_all(header::AUTHORIZATION).iter().count() != 1 || token.is_none() {
        return StatusCode::FORBIDDEN.into_response();
    }
    let hash: [u8; 32] = Sha256::digest(token.unwrap().as_bytes()).into();
    if s.caller.ct_eq(&hash).unwrap_u8() != 1 {
        return StatusCode::FORBIDDEN.into_response();
    }
    if request.validate().is_err() {
        return StatusCode::BAD_REQUEST.into_response();
    }
    let Ok(_slot) = s.slots.try_acquire() else {
        return StatusCode::SERVICE_UNAVAILABLE.into_response();
    };
    match s.issuer.issue(&request).await {
        Ok(pem) => Json(serde_json::json!({"certificate_pem":pem})).into_response(),
        Err(_) => (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(serde_json::json!({"error":"certificate_signing_unavailable"})),
        )
            .into_response(),
    }
}

/// Used only by the authenticated native central authority, never by a worker.
pub struct RemoteIssuer {
    http: Client,
    origin: String,
    token: Secret,
}
impl RemoteIssuer {
    pub fn new(origin: &str, token: Secret) -> Result<Self, Failure> {
        let url = url::Url::parse(origin).map_err(|_| Failure::Invalid)?;
        if url.scheme() != "https"
            || url.host_str().is_none()
            || !url.username().is_empty()
            || url.password().is_some()
            || url.path() != "/"
            || url.query().is_some()
            || url.fragment().is_some()
            || !(32..=8192).contains(&token.bytes().len())
        {
            return Err(Failure::Invalid);
        }
        Ok(Self {
            http: http()?,
            origin: origin.trim_end_matches('/').into(),
            token,
        })
    }
    pub async fn issue(&self, csr_pem: String, lifetime_seconds: u64) -> Result<String, Failure> {
        let request = Request {
            csr_pem,
            lifetime_seconds,
        };
        request.validate()?;
        let response = self
            .http
            .post(format!("{}{ISSUE_PATH}", self.origin))
            .bearer_auth(std::str::from_utf8(self.token.bytes()).map_err(|_| Failure::Invalid)?)
            .json(&request)
            .send()
            .await
            .map_err(|_| Failure::Unavailable)?;
        if !response.status().is_success() {
            return Err(Failure::Unavailable);
        }
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Reply {
            certificate_pem: String,
        }
        let reply: Reply =
            serde_json::from_slice(&bounded(response).await?).map_err(|_| Failure::Unavailable)?;
        certificate(reply.certificate_pem)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn a_node_token_or_invalid_request_cannot_reach_google() {
        struct NoToken;
        #[async_trait::async_trait]
        impl AccessToken for NoToken {
            async fn token(&self) -> Result<Secret, Failure> {
                panic!("An unauthorized or invalid request reached Google authentication.");
            }
        }
        let issuer = GoogleCa { http:http().unwrap(), authority:Authority::parse("projects/157437760789/locations/us-central1/caPools/node-ca/certificateAuthorities/root").unwrap(), tokens:Arc::new(NoToken) };
        let service = Arc::new(Service::new(issuer, Secret::new(vec![b'a'; 48]).unwrap()).unwrap());
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let task = tokio::spawn(async move {
            axum::serve(listener, service.router()).await.unwrap();
        });
        let client = Client::builder().no_proxy().build().unwrap();
        let body = serde_json::json!({"csr_pem":"not a CSR","lifetime_seconds":604800});
        for (token, status) in [
            ("node-token", StatusCode::FORBIDDEN),
            (&"a".repeat(48), StatusCode::BAD_REQUEST),
        ] {
            let reply = client
                .post(format!("http://{address}{ISSUE_PATH}"))
                .bearer_auth(token)
                .json(&body)
                .send()
                .await
                .unwrap();
            assert_eq!(reply.status(), status);
        }
        task.abort();
    }
    #[test]
    fn signing_is_pinned_bounded_and_retry_stable() {
        let authority=Authority::parse("projects/157437760789/locations/us-central1/caPools/node-ca/certificateAuthorities/root").unwrap();
        for bad in [
            "https://example.com",
            "projects/project-name/locations/l/caPools/a/certificateAuthorities/b",
            "projects/x/locations/../caPools/a/certificateAuthorities/b",
            "projects/p/locations/l/caPools/p/certificateAuthorities/c?x=1",
        ] {
            assert!(Authority::parse(bad).is_err());
        }
        let mut r = Request {
            csr_pem:
                "-----BEGIN CERTIFICATE REQUEST-----\nYWJj\n-----END CERTIFICATE REQUEST-----\n"
                    .into(),
            lifetime_seconds: 604800,
        };
        assert!(r.validate().is_ok());
        assert_eq!(request_id(&authority, &r, 1), request_id(&authority, &r, 1));
        assert_ne!(request_id(&authority, &r, 1), request_id(&authority, &r, 2));
        r.lifetime_seconds += 1;
        assert!(r.validate().is_err());
        assert!(certificate("arbitrary value".into()).is_err());
        assert!(
            RemoteIssuer::new("http://private.example", Secret::new(vec![1; 32]).unwrap()).is_err()
        );
    }
}
