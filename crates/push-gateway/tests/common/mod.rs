//! Local fake provider servers and throwaway credentials.
//!
//! Every key here is generated for the test run and lives in a temporary
//! directory. No provider credential or device token is used.

#![allow(dead_code)]

use std::{
    collections::{HashMap, VecDeque},
    net::SocketAddr,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};

use axum::{
    Router,
    body::Bytes,
    extract::State,
    http::{HeaderMap, Method, StatusCode, Uri, Version},
    response::{IntoResponse, Response},
};
use base64::{
    Engine as _,
    engine::general_purpose::{STANDARD, URL_SAFE_NO_PAD},
};
use push_gateway::server::{ApnsConfig, Config, FcmConfig, Limits, Secret};
use ring::{
    rand::SystemRandom,
    signature::{
        ECDSA_P256_SHA256_FIXED, ECDSA_P256_SHA256_FIXED_SIGNING, EcdsaKeyPair, KeyPair as _,
        RSA_PKCS1_2048_8192_SHA256, UnparsedPublicKey,
    },
};
use serde_json::Value;

pub const APNS_PROFILE: &str = "app.test/ios";
pub const FCM_PROFILE: &str = "app.test/android";
pub const TEAM_ID: &str = "TEAM123456";
pub const KEY_ID: &str = "KEY1234567";
pub const TOPIC: &str = "com.example.app";

/// One request a fake server received.
#[derive(Debug, Clone)]
pub struct Recorded {
    pub version: Version,
    pub path: String,
    pub headers: HashMap<String, String>,
    pub body: Vec<u8>,
}

impl Recorded {
    pub fn json(&self) -> Value {
        serde_json::from_slice(&self.body).unwrap()
    }
}

/// A scripted response.
#[derive(Debug, Clone)]
pub struct Scripted {
    pub status: u16,
    pub headers: Vec<(&'static str, String)>,
    pub body: String,
}

impl Scripted {
    pub fn new(status: u16, body: &str) -> Self {
        Self {
            status,
            headers: Vec::new(),
            body: body.to_owned(),
        }
    }
}

/// A local server that records requests and answers from a script, then
/// with its default.
pub struct Fake {
    pub base: String,
    pub address: SocketAddr,
    received: Mutex<Vec<Recorded>>,
    script: Mutex<VecDeque<Scripted>>,
    default: Scripted,
}

impl Fake {
    /// Start a fake that speaks HTTP/1.1 and cleartext HTTP/2 with prior
    /// knowledge.
    pub async fn start(default: Scripted) -> Arc<Self> {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let fake = Arc::new(Self {
            base: format!("http://{address}"),
            address,
            received: Mutex::new(Vec::new()),
            script: Mutex::new(VecDeque::new()),
            default,
        });
        let router = Router::new().fallback(record).with_state(Arc::clone(&fake));
        tokio::spawn(async move {
            axum::serve(listener, router).await.unwrap();
        });
        fake
    }

    pub fn script(&self, responses: impl IntoIterator<Item = Scripted>) {
        self.script.lock().unwrap().extend(responses);
    }

    pub fn received(&self) -> Vec<Recorded> {
        self.received.lock().unwrap().clone()
    }
}

async fn record(
    State(fake): State<Arc<Fake>>,
    _method: Method,
    version: Version,
    uri: Uri,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    fake.received.lock().unwrap().push(Recorded {
        version,
        path: uri.path().to_owned(),
        headers: headers
            .iter()
            .map(|(name, value)| {
                (
                    name.as_str().to_owned(),
                    value.to_str().unwrap_or_default().to_owned(),
                )
            })
            .collect(),
        body: body.to_vec(),
    });
    let response = fake
        .script
        .lock()
        .unwrap()
        .pop_front()
        .unwrap_or_else(|| fake.default.clone());
    let mut builder = Response::builder()
        .status(StatusCode::from_u16(response.status).unwrap())
        .header("content-type", "application/json");
    for (name, value) in response.headers {
        builder = builder.header(name, value);
    }
    builder
        .body(axum::body::Body::from(response.body))
        .unwrap()
        .into_response()
}

/// Write a file only its owner can read.
pub fn write_private(dir: &Path, name: &str, contents: &str) -> PathBuf {
    let path = dir.join(name);
    std::fs::write(&path, contents).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).unwrap();
    }
    path
}

/// A fresh P-256 key as `.p8` PEM, and its public key.
pub fn es256_key() -> (String, Vec<u8>) {
    let rng = SystemRandom::new();
    let document = EcdsaKeyPair::generate_pkcs8(&ECDSA_P256_SHA256_FIXED_SIGNING, &rng).unwrap();
    let pair = EcdsaKeyPair::from_pkcs8(&ECDSA_P256_SHA256_FIXED_SIGNING, document.as_ref(), &rng)
        .unwrap();
    (
        format!(
            "-----BEGIN PRIVATE KEY-----\n{}\n-----END PRIVATE KEY-----\n",
            STANDARD.encode(document.as_ref())
        ),
        pair.public_key().as_ref().to_vec(),
    )
}

/// A fresh 2048-bit RSA key as PKCS#8 PEM, and its public key. Uses the
/// system `openssl`, because the test must not carry a committed key.
pub fn rsa_key(dir: &Path) -> (String, Vec<u8>) {
    let path = dir.join("service-account-key.pem");
    let status = std::process::Command::new("openssl")
        .args([
            "genpkey",
            "-algorithm",
            "RSA",
            "-pkeyopt",
            "rsa_keygen_bits:2048",
            "-out",
        ])
        .arg(&path)
        .stderr(std::process::Stdio::null())
        .status()
        .expect("the FCM test needs openssl to generate a throwaway RSA key");
    assert!(status.success());
    let pem = std::fs::read_to_string(&path).unwrap();
    std::fs::remove_file(&path).unwrap();
    let der = STANDARD
        .decode(
            pem.lines()
                .filter(|line| !line.starts_with("-----"))
                .collect::<String>(),
        )
        .unwrap();
    let pair = ring::signature::RsaKeyPair::from_pkcs8(&der).unwrap();
    (pem, pair.public().as_ref().to_vec())
}

/// Split and verify a compact JWT; returns its header and claims.
pub fn verify_jwt(token: &str, public_key: &[u8], rsa: bool) -> (Value, Value) {
    let (input, signature) = token.rsplit_once('.').unwrap();
    let signature = URL_SAFE_NO_PAD.decode(signature).unwrap();
    if rsa {
        UnparsedPublicKey::new(&RSA_PKCS1_2048_8192_SHA256, public_key)
            .verify(input.as_bytes(), &signature)
            .unwrap();
    } else {
        UnparsedPublicKey::new(&ECDSA_P256_SHA256_FIXED, public_key)
            .verify(input.as_bytes(), &signature)
            .unwrap();
    }
    let (header, claims) = input.split_once('.').unwrap();
    (
        serde_json::from_slice(&URL_SAFE_NO_PAD.decode(header).unwrap()).unwrap(),
        serde_json::from_slice(&URL_SAFE_NO_PAD.decode(claims).unwrap()).unwrap(),
    )
}

/// A gateway configuration on ephemeral loopback ports.
pub fn config(
    dir: &Path,
    relay_pubkey: &str,
    apns: Option<(&str, &str)>,
    fcm: Option<(&str, &str)>,
) -> Config {
    Config {
        delivery_addr: "127.0.0.1:0".parse().unwrap(),
        registration_addr: "127.0.0.1:0".parse().unwrap(),
        state_dir: dir.join("state"),
        state_key: [0x5c; 32],
        relay_pubkeys: vec![relay_pubkey.to_owned()],
        apns: apns.map(|(base_url, pem)| ApnsConfig {
            app_profile: APNS_PROFILE.into(),
            key_pem: Secret::new(pem.to_owned()),
            key_id: KEY_ID.into(),
            team_id: TEAM_ID.into(),
            topic: TOPIC.into(),
            base_url: base_url.to_owned(),
        }),
        fcm: fcm.map(|(base_url, account)| FcmConfig {
            app_profile: FCM_PROFILE.into(),
            service_account_json: Secret::new(account.to_owned()),
            project_id: None,
            base_url: base_url.to_owned(),
        }),
        limits: Limits::default(),
    }
}
