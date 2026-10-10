//! The attested serve mode (NIP-ATT, `docs/security/private-inference.md`):
//! a pylon that runs only inside a Google Confidential Space workload on
//! Intel TDX and answers sealed NIP-DEC decisions with Psionic's Clef lane.
//!
//! At start it:
//!
//! 1. makes its endpoint key in memory (it is never written anywhere);
//! 2. fetches the pinned Clef weights and refuses to go on unless their
//!    SHA-256 is the one compiled into this image;
//! 3. starts `psionic-openai-server` on loopback and refuses to serve
//!    unless Psionic reports the same artifact digest;
//! 4. asks the Confidential Space launcher for a PKI attestation token whose
//!    nonce is the NIP-ATT binding of the endpoint key and the release, and
//!    publishes a `30203` endpoint carrying it, signed by the endpoint key,
//!    again every half hour with a fresh token.
//!
//! A sealed job must require `openagents.attested.v1` and name this
//! endpoint and release; its answer carries `response.attested` (the
//! endpoint, release, level, measurement, model and the request's
//! ciphertext digest), which the receipt's result digest covers. Nothing
//! here logs a prompt or an answer.

use std::path::{Path, PathBuf};
use std::time::Duration;

use base64::Engine as _;
use nostr::att::{self, Endpoint, Evidence, Level};
use nostr::domain::Event;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

use crate::identity::Identity;
use crate::now;

/// Where the Confidential Space launcher serves tokens to the workload.
pub const TEESERVER: &str = "/run/container_launcher/teeserver.sock";
/// How often a fresh endpoint goes out (the token lives an hour).
pub const REFRESH: Duration = Duration::from_secs(30 * 60);

/// What a sealed answer names, fixed for this instance.
#[derive(Debug, Clone)]
pub struct Attestation {
    /// `30203:<endpoint key>:<instance>`.
    pub address: String,
    /// The release event ID.
    pub release: String,
    /// The image digest the hardware reported.
    pub measurement: String,
    pub level: Level,
    /// The served model name and its weights digest (`sha256:…`).
    pub model: String,
    pub model_digest: String,
}

/// The settings of one attested instance.
#[derive(Debug, Clone)]
pub struct Setup {
    pub release: String,
    pub publisher: String,
    pub workload: String,
    pub operator: String,
    pub socket: PathBuf,
}

impl Setup {
    /// The head address, `30202:<publisher>:<workload>`.
    #[must_use]
    pub fn head_address(&self) -> String {
        format!("{}:{}:{}", att::HEAD_KIND, self.publisher, self.workload)
    }
}

/// Fetch `url` into `path` unless a file with `sha256` is already there,
/// and check the digest either way.
///
/// # Errors
///
/// When the download fails or the digest is not `sha256`.
pub async fn fetch_weights(url: &str, sha256: &str, path: &Path) -> Result<(), String> {
    if path.exists() && file_sha256(path)? == sha256 {
        eprintln!("pylon: weights already present and verified");
        return Ok(());
    }
    eprintln!("pylon: fetching weights from {url}");
    let client = reqwest::Client::builder()
        .connect_timeout(Duration::from_secs(30))
        .build()
        .map_err(|e| e.to_string())?;
    let mut response = client
        .get(url)
        .send()
        .await
        .map_err(|e| format!("the weights download failed: {e}"))?;
    if !response.status().is_success() {
        return Err(format!(
            "the weights download answered {}",
            response.status()
        ));
    }
    let partial = path.with_extension("partial");
    let mut file = std::fs::File::create(&partial).map_err(|e| e.to_string())?;
    let mut hasher = Sha256::new();
    let mut bytes = 0u64;
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|e| format!("the weights download broke off: {e}"))?
    {
        hasher.update(&chunk);
        std::io::Write::write_all(&mut file, &chunk).map_err(|e| e.to_string())?;
        bytes += chunk.len() as u64;
    }
    let digest = hex(&hasher.finalize());
    if digest != sha256 {
        let _ = std::fs::remove_file(&partial);
        return Err(format!(
            "the weights' SHA-256 is {digest}, not the pinned {sha256}; refusing to serve"
        ));
    }
    std::fs::rename(&partial, path).map_err(|e| e.to_string())?;
    eprintln!("pylon: weights verified ({bytes} bytes, sha256 {digest})");
    Ok(())
}

fn file_sha256(path: &Path) -> Result<String, String> {
    let mut file = std::fs::File::open(path).map_err(|e| e.to_string())?;
    let mut hasher = Sha256::new();
    std::io::copy(&mut file, &mut hasher).map_err(|e| e.to_string())?;
    Ok(hex(&hasher.finalize()))
}

/// Ask the launcher at `socket` for a PKI token whose nonce is `binding`.
///
/// # Errors
///
/// When the launcher is not there or refuses.
pub async fn token(socket: &Path, binding: &str) -> Result<String, String> {
    let body = json!({
        "audience": att::TOKEN_AUDIENCE,
        "token_type": "PKI",
        "nonces": [binding],
    })
    .to_string();
    let request = format!(
        "POST /v1/token HTTP/1.1\r\nHost: localhost\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    let mut stream = tokio::net::UnixStream::connect(socket).await.map_err(|e| {
        format!(
            "the Confidential Space launcher at {} is not there: {e}",
            socket.display()
        )
    })?;
    stream
        .write_all(request.as_bytes())
        .await
        .map_err(|e| e.to_string())?;
    let mut raw = Vec::new();
    tokio::time::timeout(Duration::from_secs(30), stream.read_to_end(&mut raw))
        .await
        .map_err(|_| "the launcher did not answer in 30 s".to_string())?
        .map_err(|e| e.to_string())?;
    let text = String::from_utf8_lossy(&raw);
    let (head, body) = text
        .split_once("\r\n\r\n")
        .ok_or("the launcher's answer is not HTTP")?;
    let status = head.lines().next().unwrap_or_default();
    let body = if head
        .to_ascii_lowercase()
        .contains("transfer-encoding: chunked")
    {
        unchunk(body)
    } else {
        body.to_string()
    };
    if !status.contains(" 200") {
        return Err(format!(
            "the launcher refused a token: {status}: {}",
            body.trim()
        ));
    }
    Ok(body.trim().to_string())
}

fn unchunk(body: &str) -> String {
    let mut out = String::new();
    let mut rest = body;
    while let Some((size, after)) = rest.split_once("\r\n") {
        let Ok(n) = usize::from_str_radix(size.trim(), 16) else {
            break;
        };
        if n == 0 || after.len() < n {
            break;
        }
        out.push_str(&after[..n]);
        rest = after[n..].trim_start_matches("\r\n");
    }
    out
}

/// The claims of a JWT, decoded without verifying (the worker reads its
/// own token for the measurement and expiry; clients verify).
///
/// # Errors
///
/// When the token is not a JWT.
pub fn claims(token: &str) -> Result<Value, String> {
    let payload = token.split('.').nth(1).ok_or("the token is not a JWT")?;
    let bytes = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(payload)
        .map_err(|e| e.to_string())?;
    serde_json::from_slice(&bytes).map_err(|e| e.to_string())
}

/// A fresh token and the `30203` endpoint carrying it, signed by
/// `identity`, for instance `instance`. Returns the event and the token's
/// claims.
///
/// # Errors
///
/// When the launcher refuses or the record does not validate.
pub async fn endpoint_event(
    identity: &Identity,
    setup: &Setup,
    instance: &str,
) -> Result<(Event, Value), String> {
    let binding = att::binding(identity.pubkey(), None, &setup.release)?;
    let token = token(&setup.socket, &binding).await?;
    let claims = claims(&token)?;
    let exp = claims["exp"].as_u64().ok_or("the token has no exp")?;
    let issued_at = now();
    let valid_until = (issued_at + att::MAX_ENDPOINT_VALIDITY_SECS).min(exp.saturating_sub(60));
    let endpoint = Endpoint {
        v: att::ENDPOINT_V.into(),
        requires: Vec::new(),
        release: setup.release.clone(),
        endpoint: identity.pubkey().into(),
        hpke: None,
        binding,
        evidence: vec![Evidence {
            kind: "gcp-confidential-space-token".into(),
            format: "pki".into(),
            token,
        }],
        operator: setup.operator.clone(),
        issued_at,
        valid_until,
    };
    let event = att::endpoint_event(
        identity.signer(),
        &endpoint,
        instance,
        &setup.head_address(),
    )?;
    Ok((event, claims))
}

/// The GPU model Google names for an H100 in a Confidential VM.
pub const H100_HWMODEL: &str = "GCP_NVIDIA_H100";

/// NIP-ATT: a GPU workload MUST refuse to start unless the GPU is in
/// confidential-computing mode. Google verifies the GPU's NVIDIA evidence
/// and signs `submods.nvidia_gpu` into the same token whose nonce binds the
/// endpoint key; this reads those claims and requires `cc_mode` `ON` and at
/// least one GPU, every one of them an H100. Returns the GPU count and the
/// driver version.
///
/// # Errors
///
/// When the token carries no GPU claims, the mode is not `ON`, or a GPU is
/// not an H100.
pub fn check_gpu_cc(claims: &Value) -> Result<(usize, String), String> {
    let gpu = claims
        .get("submods")
        .and_then(|s| s.get("nvidia_gpu"))
        .ok_or("the token carries no submods.nvidia_gpu: no confidential GPU is attested")?;
    let mode = gpu
        .get("cc_mode")
        .and_then(Value::as_str)
        .unwrap_or("absent");
    if mode != "ON" {
        return Err(format!(
            "the GPU's confidential-computing mode is {mode}, not ON; refusing to start"
        ));
    }
    let gpus = gpu
        .get("gpus")
        .and_then(Value::as_array)
        .filter(|gpus| !gpus.is_empty())
        .ok_or("the token lists no GPU under submods.nvidia_gpu.gpus")?;
    for (index, one) in gpus.iter().enumerate() {
        let model = one
            .get("hwmodel")
            .and_then(Value::as_str)
            .unwrap_or("absent");
        if model != H100_HWMODEL {
            return Err(format!(
                "GPU {index} is {model}, not {H100_HWMODEL}; refusing to start"
            ));
        }
    }
    let driver = gpu
        .get("driver_version")
        .and_then(Value::as_str)
        .unwrap_or("unknown")
        .to_string();
    Ok((gpus.len(), driver))
}

/// A random 64-hex instance ID.
#[must_use]
pub fn instance_id() -> String {
    let bytes: [u8; 32] = secp256k1::rand::random();
    hex(&bytes)
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chunked_bodies_are_joined() {
        assert_eq!(
            unchunk("5\r\nhello\r\n6\r\n world\r\n0\r\n\r\n"),
            "hello world"
        );
    }

    #[test]
    fn claims_decode_from_the_middle_part() {
        let payload = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(br#"{"exp":5}"#);
        assert_eq!(claims(&format!("a.{payload}.c")).unwrap()["exp"], 5);
        assert!(claims("nope").is_err());
    }

    #[test]
    fn a_gpu_in_cc_mode_passes_and_anything_else_refuses() {
        let on = json!({"submods": {"nvidia_gpu": {
            "cc_mode": "ON", "cc_feature": "SPT", "driver_version": "580.95.05",
            "gpus": [{"hwmodel": "GCP_NVIDIA_H100", "ueid": "1"}],
        }}});
        assert_eq!(check_gpu_cc(&on).unwrap(), (1, "580.95.05".into()));
        for mode in ["OFF", "DEVTOOLS"] {
            let mut claims = on.clone();
            claims["submods"]["nvidia_gpu"]["cc_mode"] = json!(mode);
            assert!(check_gpu_cc(&claims).unwrap_err().contains(mode));
        }
        let mut no_mode = on.clone();
        no_mode["submods"]["nvidia_gpu"]
            .as_object_mut()
            .unwrap()
            .remove("cc_mode");
        assert!(check_gpu_cc(&no_mode).is_err());
        let mut none = on.clone();
        none["submods"]["nvidia_gpu"]["gpus"] = json!([]);
        assert!(check_gpu_cc(&none).is_err());
        let mut other = on.clone();
        other["submods"]["nvidia_gpu"]["gpus"] =
            json!([{"hwmodel": "GCP_NVIDIA_H100"}, {"hwmodel": "GCP_NVIDIA_A100"}]);
        assert!(check_gpu_cc(&other).unwrap_err().contains("A100"));
        let cpu_only = json!({"submods": {"container": {"image_digest": "sha256:aa"}}});
        assert!(check_gpu_cc(&cpu_only).unwrap_err().contains("nvidia_gpu"));
    }

    #[tokio::test]
    async fn a_token_comes_from_the_launcher_socket() {
        let dir = PathBuf::from(format!("/tmp/pt-{}", &instance_id()[..12]));
        std::fs::create_dir_all(&dir).unwrap();
        let socket = dir.join("teeserver.sock");
        let listener = tokio::net::UnixListener::bind(&socket).unwrap();
        let server = tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            let mut buf = vec![0u8; 4096];
            let n = stream.read(&mut buf).await.unwrap();
            let request = String::from_utf8_lossy(&buf[..n]).to_string();
            stream
                .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 7\r\n\r\na.b.cde")
                .await
                .unwrap();
            request
        });
        let got = token(&socket, &"ab".repeat(32)).await.unwrap();
        assert_eq!(got, "a.b.cde");
        let request = server.await.unwrap();
        assert!(request.starts_with("POST /v1/token "));
        assert!(request.contains("\"token_type\":\"PKI\""));
        assert!(request.contains(&"ab".repeat(32)));
        let _ = std::fs::remove_dir_all(dir);
    }
}
