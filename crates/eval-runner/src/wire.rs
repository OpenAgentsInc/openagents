//! The runner's two outside services, behind traits so tests run it in
//! memory: the relay ([`Wire`]) and the Blossom blob store ([`Blobs`]).

use std::sync::Arc;
use std::time::Duration;

use futures_util::StreamExt as _;
use futures_util::future::BoxFuture;
use nostr::domain::{Event, RelaySigner};
use serde_json::{Value, json};
use tokio_tungstenite::tungstenite;

/// How long one relay operation may take, connection included.
pub const RELAY_BOUND: Duration = Duration::from_secs(20);

/// The relay: publish an event and wait for its `OK`, or read the stored
/// events a filter matches.
pub trait Wire: Send + Sync + 'static {
    /// Publishes `event`; `Ok` once the relay accepted it.
    fn publish(&self, event: Event) -> BoxFuture<'_, Result<(), String>>;
    /// The stored events `filter` matches, until the relay's end of stored
    /// events.
    fn query(&self, filter: Value) -> BoxFuture<'_, Result<Vec<Event>, String>>;
}

/// The blob store a suite's files live in.
pub trait Blobs: Send + Sync + 'static {
    /// The bytes with `digest` (`sha256:…`), checked.
    ///
    /// # Errors
    ///
    /// Why they can't be fetched.
    fn fetch(&self, digest: &str) -> Result<Vec<u8>, String>;
    /// Stores `bytes` unless the server holds them already.
    ///
    /// # Errors
    ///
    /// The server's refusal.
    fn upload(&self, signer: &RelaySigner, bytes: &[u8], media: &str) -> Result<(), String>;
}

/// A Blossom server through `ext_eval::blob::Blossom`, whose blocking HTTP
/// client may be neither built nor dropped on an async thread: it is built
/// and dropped on a thread of its own, and used only from blocking tasks.
pub struct Blossom(Option<ext_eval::blob::Blossom>);

impl Blossom {
    /// The server at `base`, or the one a relay serves when `base` is
    /// `None`.
    ///
    /// # Errors
    ///
    /// Why the URL or the client doesn't work.
    pub fn new(base: Option<&str>, relay: &str) -> Result<Self, String> {
        let (base, relay) = (base.map(str::to_string), relay.to_string());
        std::thread::spawn(move || match base {
            Some(base) => ext_eval::blob::Blossom::new(&base),
            None => ext_eval::blob::Blossom::for_relay(&relay),
        })
        .join()
        .map_err(|_| "the blob client's thread ended".to_string())?
        .map(|inner| Self(Some(inner)))
    }

    fn inner(&self) -> Result<&ext_eval::blob::Blossom, String> {
        self.0
            .as_ref()
            .ok_or_else(|| "the blob client is closed".to_string())
    }
}

impl Drop for Blossom {
    fn drop(&mut self) {
        if let Some(inner) = self.0.take() {
            let _ = std::thread::spawn(move || drop(inner)).join();
        }
    }
}

impl Blobs for Blossom {
    fn fetch(&self, digest: &str) -> Result<Vec<u8>, String> {
        self.inner()?.fetch(digest)
    }
    fn upload(&self, signer: &RelaySigner, bytes: &[u8], media: &str) -> Result<(), String> {
        self.inner()?
            .upload(signer, bytes, media, crate::unix_now())
    }
}

/// A Google Cloud Storage bucket as the blob store: read over public
/// HTTPS like any Blossom server (`GET <base>/<sha256 hex>`), written with
/// `gcloud storage cp` under a service account that may only create
/// objects in that bucket. `relay.openagents.com` serves no Blossom media
/// yet (`docs/deployment/eval-runner.md`), so the runner's suites live here.
pub struct Bucket {
    read: Blossom,
    bucket: String,
    gcloud: std::path::PathBuf,
}

impl Bucket {
    /// The bucket `gs://…` read at `base` and written with the gcloud
    /// configuration directory `gcloud`.
    ///
    /// # Errors
    ///
    /// Why the read client doesn't build.
    pub fn new(base: &str, bucket: &str, gcloud: &std::path::Path) -> Result<Self, String> {
        if !bucket.starts_with("gs://") {
            return Err(format!("{bucket} is not a gs:// bucket"));
        }
        Ok(Self {
            read: Blossom::new(Some(base), "")?,
            bucket: bucket.trim_end_matches('/').to_string(),
            gcloud: gcloud.to_path_buf(),
        })
    }
}

impl Blobs for Bucket {
    fn fetch(&self, digest: &str) -> Result<Vec<u8>, String> {
        self.read.fetch(digest)
    }
    fn upload(&self, _: &RelaySigner, bytes: &[u8], media: &str) -> Result<(), String> {
        let digest = nostr::contracts::digest_bytes(bytes);
        let hex = digest.trim_start_matches("sha256:");
        if self.read.inner()?.has(&digest) {
            return Ok(());
        }
        let mut file = tempfile::NamedTempFile::new().map_err(|error| error.to_string())?;
        std::io::Write::write_all(&mut file, bytes).map_err(|error| error.to_string())?;
        let out = std::process::Command::new("gcloud")
            .args(["storage", "cp", "--quiet"])
            .arg(file.path())
            .arg(format!("{}/{hex}", self.bucket))
            .arg(format!("--content-type={media}"))
            .env("CLOUDSDK_CONFIG", &self.gcloud)
            .stdin(std::process::Stdio::null())
            .output()
            .map_err(|error| format!("gcloud: {error}"))?;
        if !out.status.success() {
            let stderr = String::from_utf8_lossy(&out.stderr);
            return Err(format!(
                "the bucket refused {hex}: {}",
                stderr.lines().last().unwrap_or_default()
            ));
        }
        Ok(())
    }
}

/// A relay reached over NIP-42 as the runner.
pub struct Relay {
    url: String,
    identity: Arc<coder::relay::Identity>,
}

impl Relay {
    /// The relay at `url`, as `identity`.
    #[must_use]
    pub fn new(url: &str, identity: Arc<coder::relay::Identity>) -> Self {
        Self {
            url: url.to_string(),
            identity,
        }
    }

    async fn socket(&self) -> Result<coder::relay::Socket, String> {
        coder::relay::connect_within(&self.url, &self.identity, RELAY_BOUND)
            .await
            .map_err(|error| error.to_string())
    }
}

async fn next_value(socket: &mut coder::relay::Socket) -> Result<Value, String> {
    loop {
        match socket.next().await {
            Some(Ok(tungstenite::Message::Text(text))) => {
                if let Ok(value) = serde_json::from_str::<Value>(&text) {
                    return Ok(value);
                }
            }
            Some(Ok(_)) => {}
            Some(Err(error)) => return Err(format!("socket: {error}")),
            None => return Err("the relay closed the socket".into()),
        }
    }
}

impl Wire for Relay {
    fn publish(&self, event: Event) -> BoxFuture<'_, Result<(), String>> {
        Box::pin(async move {
            tokio::time::timeout(RELAY_BOUND, async {
                let mut socket = self.socket().await?;
                coder::relay::send(&mut socket, json!(["EVENT", event]))
                    .await
                    .map_err(|error| error.to_string())?;
                loop {
                    let value = next_value(&mut socket).await?;
                    if value[0] == "OK" && value[1].as_str() == Some(event.id.as_str()) {
                        let _ = socket.close(None).await;
                        return if value[2] == true {
                            Ok(())
                        } else {
                            Err(format!(
                                "the relay refused {}: {}",
                                event.kind,
                                value[3].as_str().unwrap_or_default()
                            ))
                        };
                    }
                }
            })
            .await
            .map_err(|_| "the relay didn't answer in time".to_string())?
        })
    }

    fn query(&self, filter: Value) -> BoxFuture<'_, Result<Vec<Event>, String>> {
        Box::pin(async move {
            tokio::time::timeout(RELAY_BOUND, async {
                let mut socket = self.socket().await?;
                coder::relay::send(&mut socket, json!(["REQ", "q", filter]))
                    .await
                    .map_err(|error| error.to_string())?;
                let mut events = Vec::new();
                loop {
                    let value = next_value(&mut socket).await?;
                    match value[0].as_str() {
                        Some("EVENT") if value[1] == "q" => {
                            if let Ok(event) = serde_json::from_value::<Event>(value[2].clone())
                                && event.validate_crypto().is_ok()
                            {
                                events.push(event);
                            }
                        }
                        Some("EOSE") if value[1] == "q" => break,
                        Some("CLOSED") if value[1] == "q" => {
                            return Err(format!(
                                "the relay closed the query: {}",
                                value[2].as_str().unwrap_or_default()
                            ));
                        }
                        _ => {}
                    }
                }
                let _ = socket.close(None).await;
                Ok(events)
            })
            .await
            .map_err(|_| "the relay didn't answer in time".to_string())?
        })
    }
}

/// An in-memory relay and blob store, for tests.
pub mod memory {
    use std::collections::BTreeMap;
    use std::sync::Mutex;

    use super::{Blobs, BoxFuture, Event, RelaySigner, Value, Wire};

    /// Keeps what's published; answers a query with the stored events that
    /// match its `ids`, `kinds`, `authors`, and `#x`/`#t`/`#p`/`#e` tags.
    #[derive(Default)]
    pub struct Memory {
        /// Everything published, in order.
        pub events: Mutex<Vec<Event>>,
        /// Blobs by digest.
        pub blobs: Mutex<BTreeMap<String, Vec<u8>>>,
    }

    fn matches(filter: &Value, event: &Event) -> bool {
        let listed = |key: &str, value: &str| {
            filter
                .get(key)
                .and_then(Value::as_array)
                .is_none_or(|list| list.iter().any(|item| item.as_str() == Some(value)))
        };
        let kinds = filter
            .get("kinds")
            .and_then(Value::as_array)
            .is_none_or(|list| {
                list.iter()
                    .any(|kind| kind.as_u64() == Some(u64::from(event.kind)))
            });
        let tags = ["x", "t", "p", "e"].iter().all(|name| {
            filter
                .get(format!("#{name}"))
                .and_then(Value::as_array)
                .is_none_or(|list| {
                    event
                        .tag_values(name)
                        .any(|value| list.iter().any(|item| item.as_str() == Some(value)))
                })
        });
        kinds && tags && listed("ids", &event.id) && listed("authors", &event.pubkey)
    }

    impl Wire for Memory {
        fn publish(&self, event: Event) -> BoxFuture<'_, Result<(), String>> {
            Box::pin(async move {
                event
                    .validate_crypto()
                    .map_err(|_| "a bad signature".to_string())?;
                self.events.lock().expect("events").push(event);
                Ok(())
            })
        }
        fn query(&self, filter: Value) -> BoxFuture<'_, Result<Vec<Event>, String>> {
            Box::pin(async move {
                Ok(self
                    .events
                    .lock()
                    .expect("events")
                    .iter()
                    .filter(|event| !(20_000..30_000).contains(&event.kind))
                    .filter(|event| matches(&filter, event))
                    .cloned()
                    .collect())
            })
        }
    }

    impl Blobs for Memory {
        fn fetch(&self, digest: &str) -> Result<Vec<u8>, String> {
            self.blobs
                .lock()
                .expect("blobs")
                .get(digest)
                .cloned()
                .ok_or_else(|| format!("no blob {digest}"))
        }
        fn upload(&self, _: &RelaySigner, bytes: &[u8], _: &str) -> Result<(), String> {
            self.blobs
                .lock()
                .expect("blobs")
                .insert(nostr::contracts::digest_bytes(bytes), bytes.to_vec());
            Ok(())
        }
    }
}
