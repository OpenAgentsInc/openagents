//! `openagents playtest testflight`: TestFlight feedback from App Store
//! Connect into the triage inbox (`docs/game/playtest-triage.md`).
//!
//! It signs an App Store Connect API token (ES256, 20 minutes) with the
//! team's API key, lists the app's beta feedback screenshot and crash
//! submissions, and for each submission the triage log doesn't hold yet
//! writes a draft (never quoting the tester's comment), the screenshots,
//! and the crash log into the private drafts folder, then appends a
//! `testflight` log entry. The tester's Apple identity is never read into
//! a draft, a file, or the log. The key is read from a `.p8` file and is
//! never printed.

use std::path::{Path, PathBuf};
use std::time::Duration;

use base64::Engine as _;
use playtest::testflight::{self, Feedback, Source};
use playtest::triage::Entry;
use serde_json::{Value, json};

use super::{Failure, drafts, load, now, private_dir, private_file, usage};
use crate::Args;

/// The OpenAgents app's App Store Connect ID.
pub const APP_ID: &str = "6748620735";
const API: &str = "https://api.appstoreconnect.apple.com";
/// Pages read per source, 50 submissions each.
const MAX_PAGES: usize = 20;

/// An App Store Connect API key.
pub struct Credentials {
    pub key_id: String,
    pub issuer: String,
    /// The `.p8` file's PEM text.
    pub private_key: String,
}

impl std::fmt::Debug for Credentials {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Credentials")
            .field("key_id", &self.key_id)
            .field("issuer", &self.issuer)
            .finish_non_exhaustive()
    }
}

/// Reads `KEY=VALUE` lines, skipping comments and blanks.
fn env_file(path: &Path) -> Result<Vec<(String, String)>, String> {
    let text =
        std::fs::read_to_string(path).map_err(|e| format!("reading {}: {e}", path.display()))?;
    Ok(text
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .filter_map(|l| l.trim_start_matches("export ").split_once('='))
        .map(|(k, v)| {
            (
                k.trim().to_owned(),
                v.trim().trim_matches(['"', '\'']).to_owned(),
            )
        })
        .collect())
}

fn expand(path: &str) -> PathBuf {
    match (path.strip_prefix("~/"), std::env::var_os("HOME")) {
        (Some(rest), Some(home)) => PathBuf::from(home).join(rest),
        _ => PathBuf::from(path),
    }
}

/// The key from `--asc-env FILE`, else from `ASC_API_KEY_ID`,
/// `ASC_API_ISSUER_ID`, and `ASC_API_PRIVATE_KEY_PATH` in the environment.
///
/// # Errors
///
/// A setting that is missing, or a key file that can't be read.
pub fn credentials(env: Option<&Path>) -> Result<Credentials, String> {
    let from_file = env.map(env_file).transpose()?.unwrap_or_default();
    let get = |name: &str| {
        from_file
            .iter()
            .find(|(k, _)| k == name)
            .map(|(_, v)| v.clone())
            .or_else(|| std::env::var(name).ok())
            .filter(|v| !v.is_empty())
            .ok_or_else(|| format!("{name} isn't set (give --asc-env FILE or set it)"))
    };
    let path = expand(&get("ASC_API_PRIVATE_KEY_PATH")?);
    let private_key = std::fs::read_to_string(&path)
        .map_err(|e| format!("reading the App Store Connect key {}: {e}", path.display()))?;
    Ok(Credentials {
        key_id: get("ASC_API_KEY_ID")?,
        issuer: get("ASC_API_ISSUER_ID")?,
        private_key,
    })
}

/// The DER bytes of a PEM `PRIVATE KEY` block.
fn pem_der(pem: &str) -> Result<Vec<u8>, String> {
    let body: String = pem
        .lines()
        .map(str::trim)
        .filter(|l| !l.starts_with("-----"))
        .collect();
    base64::engine::general_purpose::STANDARD
        .decode(body.as_bytes())
        .map_err(|_| "the App Store Connect key isn't a PEM private key".to_owned())
}

/// An App Store Connect API token valid from `now` for 20 minutes: a JWT
/// signed ES256 with the API key.
///
/// # Errors
///
/// When the key isn't a P-256 PKCS#8 key.
pub fn token(credentials: &Credentials, now: u64) -> Result<String, String> {
    use ring::signature::{ECDSA_P256_SHA256_FIXED_SIGNING, EcdsaKeyPair};
    let random = ring::rand::SystemRandom::new();
    let der = pem_der(&credentials.private_key)?;
    let pair = EcdsaKeyPair::from_pkcs8(&ECDSA_P256_SHA256_FIXED_SIGNING, &der, &random)
        .map_err(|_| "the App Store Connect key isn't a P-256 key".to_owned())?;
    let encode = |value: &Value| {
        base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(value.to_string().as_bytes())
    };
    let header = json!({"alg": "ES256", "kid": credentials.key_id, "typ": "JWT"});
    let claims = json!({
        "iss": credentials.issuer, "iat": now, "exp": now + 20 * 60,
        "aud": "appstoreconnect-v1",
    });
    let signing = format!("{}.{}", encode(&header), encode(&claims));
    let signature = pair
        .sign(&random, signing.as_bytes())
        .map_err(|_| "signing the App Store Connect token failed".to_owned())?;
    Ok(format!(
        "{signing}.{}",
        base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(signature.as_ref())
    ))
}

/// Reads App Store Connect with one token.
struct Client {
    http: reqwest::Client,
    token: String,
    runtime: tokio::runtime::Runtime,
}

impl Client {
    fn new(token: String) -> Result<Self, String> {
        Ok(Self {
            http: reqwest::Client::builder()
                .timeout(Duration::from_secs(30))
                .build()
                .map_err(|e| format!("starting HTTP: {e}"))?,
            token,
            runtime: tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .map_err(|e| format!("starting the runtime: {e}"))?,
        })
    }

    fn get(&self, url: &str) -> Result<Value, String> {
        self.runtime.block_on(async {
            let response = self
                .http
                .get(url)
                .bearer_auth(&self.token)
                .send()
                .await
                .map_err(|e| format!("App Store Connect: {}", e.without_url()))?;
            let status = response.status();
            let body: Value = response
                .json()
                .await
                .map_err(|_| format!("App Store Connect answered {status} without JSON"))?;
            if !status.is_success() {
                let detail = body["errors"][0]["detail"]
                    .as_str()
                    .or_else(|| body["errors"][0]["title"].as_str())
                    .unwrap_or("no detail");
                return Err(format!("App Store Connect answered {status}: {detail}"));
            }
            Ok(body)
        })
    }

    fn bytes(&self, url: &str) -> Result<Vec<u8>, String> {
        self.runtime.block_on(async {
            let response = self
                .http
                .get(url)
                .send()
                .await
                .map_err(|e| format!("downloading a screenshot: {}", e.without_url()))?;
            if !response.status().is_success() {
                return Err(format!(
                    "a screenshot download answered {}",
                    response.status()
                ));
            }
            response
                .bytes()
                .await
                .map(|b| b.to_vec())
                .map_err(|e| format!("downloading a screenshot: {}", e.without_url()))
        })
    }
}

/// Every submission of `source` for `app`, newest first.
fn list(client: &Client, app: &str, source: Source) -> Result<Vec<Feedback>, String> {
    let mut url = Some(format!(
        "{API}/v1/apps/{app}/{}?limit=50&include=build&fields[builds]=version",
        source.resource()
    ));
    let mut out = Vec::new();
    for _ in 0..MAX_PAGES {
        let Some(next) = url.take() else { break };
        let page = testflight::parse_page(source, &client.get(&next)?)?;
        out.extend(page.feedback);
        url = page.next.filter(|n| n.starts_with(API));
    }
    Ok(out)
}

/// What one `testflight` run did.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Ingested {
    pub new: Vec<String>,
    pub repeats: usize,
    /// Screenshots or crash logs that couldn't be downloaded.
    pub missing: usize,
}

/// Writes a draft and its attachments for each submission the log doesn't
/// hold, and appends its `testflight` entry. `fetch` reads an attachment:
/// a screenshot by URL, or a crash log by `crash:SUBMISSION-ID`.
pub fn ingest(
    home: &Path,
    feedback: &[Feedback],
    at: u64,
    fetch: &mut dyn FnMut(&str) -> Result<Vec<u8>, String>,
) -> Result<Ingested, String> {
    let mut log = load(home)?;
    let dir = drafts(home);
    private_dir(&dir)?;
    let mut result = Ingested::default();
    for item in feedback {
        if log.has_submission(&item.id) {
            result.repeats += 1;
            continue;
        }
        let code = item.code();
        let draft = testflight::draft(item);
        private_file(&dir.join(format!("{code}.md")), draft.markdown().as_bytes())?;
        let meta = json!({"code": code, "labels": draft.labels, "submission": item.id, "source": item.source});
        private_file(
            &dir.join(format!("{code}.json")),
            meta.to_string().as_bytes(),
        )?;
        let mut private = item.clone();
        private.images.iter_mut().for_each(|i| i.url.clear());
        private_file(
            &dir.join(format!("{code}.testflight.json")),
            serde_json::to_string_pretty(&private)
                .unwrap_or_default()
                .as_bytes(),
        )?;
        for (index, image) in item.images.iter().enumerate() {
            let name = if index == 0 {
                format!("{code}.jpg")
            } else {
                format!("{code}-{}.jpg", index + 1)
            };
            match fetch(&image.url) {
                Ok(bytes) => private_file(&dir.join(name), &bytes)?,
                Err(_) => result.missing += 1,
            }
        }
        if item.source == Source::Crash {
            match fetch(&format!("crash:{}", item.id)) {
                Ok(bytes) => private_file(&dir.join(format!("{code}.crash.txt")), &bytes)?,
                Err(_) => result.missing += 1,
            }
        }
        super::append(
            home,
            &mut log,
            Entry::Testflight {
                at,
                code: code.clone(),
                submission: item.id.clone(),
                source: item.source,
                build: item.build_label(),
                created: item.created.clone(),
            },
        )?;
        result.new.push(code);
    }
    Ok(result)
}

pub fn run(home: &Path, args: &Args) -> Result<Value, Failure> {
    let credentials = credentials(args.option("asc-env").map(Path::new))?;
    let app = args.option("app").unwrap_or(APP_ID).to_owned();
    if app.is_empty() || !app.chars().all(|c| c.is_ascii_digit()) {
        return Err(usage("--app takes the numeric App Store Connect app ID"));
    }
    let client = Client::new(token(&credentials, now())?)?;
    let mut feedback = list(&client, &app, Source::Screenshot)?;
    feedback.extend(list(&client, &app, Source::Crash)?);
    // ISO 8601 dates in UTC compare as text.
    if let Some(since) = args.option("since") {
        feedback.retain(|f| f.created.as_str() >= since);
    }
    let log = load(home)?;
    // Marketing versions, looked up once per build, for new submissions.
    let mut versions: std::collections::BTreeMap<String, Option<String>> =
        std::collections::BTreeMap::new();
    for item in feedback.iter_mut().filter(|f| !log.has_submission(&f.id)) {
        let Some(build) = item.build_id.clone() else {
            continue;
        };
        let version =
            versions.entry(build.clone()).or_insert_with(|| {
                client
                .get(&format!(
                    "{API}/v1/builds/{build}/preReleaseVersion?fields[preReleaseVersions]=version"
                ))
                .ok()
                .and_then(|v| v["data"]["attributes"]["version"].as_str().map(str::to_owned))
            });
        item.app_version.clone_from(version);
    }
    feedback.sort_by(|a, b| a.created.cmp(&b.created));
    let mut fetch = |what: &str| -> Result<Vec<u8>, String> {
        if let Some(id) = what.strip_prefix("crash:") {
            let body = client.get(&format!(
                "{API}/v1/{}/{id}/crashLog",
                Source::Crash.resource()
            ))?;
            body["data"]["attributes"]["logText"]
                .as_str()
                .map(|t| t.as_bytes().to_vec())
                .ok_or_else(|| "the crash log has no text".to_owned())
        } else {
            client.bytes(what)
        }
    };
    let result = ingest(home, &feedback, now(), &mut fetch)?;
    Ok(json!({
        "app": app,
        "read": feedback.len(),
        "new": result.new,
        "repeats": result.repeats,
        "missing_attachments": result.missing,
        "drafts": drafts(home).display().to_string(),
        "text": format!(
            "{} TestFlight submissions read: {} new, {} already in the log{}.{}",
            feedback.len(), result.new.len(), result.repeats,
            if result.missing == 0 { String::new() } else { format!(", {} screenshots or crash logs no longer available", result.missing) },
            if result.new.is_empty() { String::new() } else {
                format!("\nWrite each draft in your own words in {}, then file or decide it:\n  {}", drafts(home).display(), result.new.join("\n  "))
            }
        ),
    }))
}

#[cfg(test)]
mod tests;
