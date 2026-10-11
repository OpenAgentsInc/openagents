//! The Compute Engine calls the service makes, behind a trait so tests can
//! run the whole API on a fake.
//!
//! The REST client gets its token from the metadata server (the Cloud Run
//! service's own account) or, off GCP, from `gcloud auth
//! print-access-token` (which honors `CLOUDSDK_CONFIG`). Tokens are cached
//! until five minutes before they expire and never logged.

use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::future::Future;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::Mutex;

pub type Result<T> = std::result::Result<T, GceError>;

/// A Compute Engine failure. `code` is GCE's reason when it gave one
/// (`ZONE_RESOURCE_POOL_EXHAUSTED`, `notFound`, ...).
#[derive(Clone, Debug)]
pub struct GceError {
    pub status: u16,
    pub code: String,
    pub message: String,
}

impl GceError {
    pub fn new(status: u16, code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            status,
            code: code.into(),
            message: message.into(),
        }
    }
    pub fn not_found(&self) -> bool {
        self.status == 404 || self.code == "notFound"
    }
    /// The zone had no room for this machine; another zone may.
    pub fn capacity(&self) -> bool {
        matches!(
            self.code.as_str(),
            "ZONE_RESOURCE_POOL_EXHAUSTED"
                | "ZONE_RESOURCE_POOL_EXHAUSTED_WITH_DETAILS"
                | "QUOTA_EXCEEDED"
                | "resourceExhausted"
                | "UNSUPPORTED_OPERATION"
        ) || self.message.contains("does not have enough resources")
    }
}

impl std::fmt::Display for GceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "GCE {} {}: {}", self.status, self.code, self.message)
    }
}

/// What the service reads of an instance.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Instance {
    pub name: String,
    pub zone: String,
    /// PROVISIONING, STAGING, RUNNING, STOPPING, SUSPENDING, SUSPENDED,
    /// TERMINATED, REPAIRING.
    pub status: String,
    pub labels: BTreeMap<String, String>,
    pub label_fingerprint: String,
    pub ip: Option<String>,
    pub machine: String,
    pub creation: Option<String>,
    pub last_start: Option<String>,
    pub last_stop: Option<String>,
    /// The boot disk's name.
    pub disk: Option<String>,
    pub disk_gb: Option<i64>,
    pub spot: bool,
}

impl Instance {
    pub fn from_json(v: &Value) -> Self {
        let s = |k: &str| v.get(k).and_then(Value::as_str).map(str::to_owned);
        let labels = v
            .get("labels")
            .and_then(Value::as_object)
            .map(|m| {
                m.iter()
                    .filter_map(|(k, v)| Some((k.clone(), v.as_str()?.to_owned())))
                    .collect()
            })
            .unwrap_or_default();
        let disk = v
            .pointer("/disks/0/source")
            .and_then(Value::as_str)
            .map(|s| s.rsplit('/').next().unwrap_or(s).to_owned());
        Self {
            name: s("name").unwrap_or_default(),
            zone: s("zone")
                .map(|z| z.rsplit('/').next().unwrap_or(&z).to_owned())
                .unwrap_or_default(),
            status: s("status").unwrap_or_default(),
            labels,
            label_fingerprint: s("labelFingerprint").unwrap_or_default(),
            ip: v
                .pointer("/networkInterfaces/0/networkIP")
                .and_then(Value::as_str)
                .map(str::to_owned),
            machine: s("machineType")
                .map(|m| m.rsplit('/').next().unwrap_or(&m).to_owned())
                .unwrap_or_default(),
            creation: s("creationTimestamp"),
            last_start: s("lastStartTimestamp"),
            last_stop: s("lastStopTimestamp"),
            disk,
            disk_gb: v
                .pointer("/disks/0/diskSizeGb")
                .and_then(|d| d.as_str().and_then(|s| s.parse().ok()).or(d.as_i64())),
            spot: v
                .pointer("/scheduling/provisioningModel")
                .and_then(Value::as_str)
                == Some("SPOT"),
        }
    }
}

/// What the service reads of an image.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Image {
    pub name: String,
    pub id: String,
    /// PENDING, READY, FAILED, DELETING.
    pub status: String,
    pub labels: BTreeMap<String, String>,
    pub disk_gb: i64,
    pub archive_bytes: Option<i64>,
    pub creation: Option<String>,
    pub self_link: String,
    pub family: Option<String>,
}

impl Image {
    pub fn from_json(v: &Value) -> Self {
        let s = |k: &str| v.get(k).and_then(Value::as_str).map(str::to_owned);
        let n = |k: &str| {
            v.get(k)
                .and_then(|d| d.as_str().and_then(|s| s.parse().ok()).or(d.as_i64()))
        };
        Self {
            name: s("name").unwrap_or_default(),
            id: s("id").unwrap_or_default(),
            status: s("status").unwrap_or_default(),
            labels: v
                .get("labels")
                .and_then(Value::as_object)
                .map(|m| {
                    m.iter()
                        .filter_map(|(k, v)| Some((k.clone(), v.as_str()?.to_owned())))
                        .collect()
                })
                .unwrap_or_default(),
            disk_gb: n("diskSizeGb").unwrap_or(10),
            archive_bytes: n("archiveSizeBytes"),
            creation: s("creationTimestamp"),
            self_link: s("selfLink").unwrap_or_default(),
            family: s("family"),
        }
    }
}

/// The calls the service makes. Every long operation is waited for.
pub trait Compute: Send + Sync + 'static {
    /// Insert an instance and wait for GCE to accept or refuse it.
    fn insert_instance(&self, zone: &str, body: Value) -> impl Future<Output = Result<()>> + Send;
    fn get_instance(
        &self,
        zone: &str,
        name: &str,
    ) -> impl Future<Output = Result<Option<Instance>>> + Send;
    /// Every instance carrying `label=value`, in any zone.
    fn list_instances(
        &self,
        label: &str,
        value: &str,
    ) -> impl Future<Output = Result<Vec<Instance>>> + Send;
    /// Start, stop, or delete; returns once GCE accepted the request.
    fn instance_action(
        &self,
        zone: &str,
        name: &str,
        action: &str,
    ) -> impl Future<Output = Result<()>> + Send;
    fn set_labels(
        &self,
        zone: &str,
        name: &str,
        labels: &BTreeMap<String, String>,
        fingerprint: &str,
    ) -> impl Future<Output = Result<()>> + Send;
    fn get_image(&self, name: &str) -> impl Future<Output = Result<Option<Image>>> + Send;
    fn image_from_family(&self, family: &str)
    -> impl Future<Output = Result<Option<Image>>> + Send;
    fn list_images(
        &self,
        label: &str,
        value: &str,
    ) -> impl Future<Output = Result<Vec<Image>>> + Send;
    /// Start an image insert; returns once GCE accepted it.
    fn insert_image(&self, body: Value) -> impl Future<Output = Result<()>> + Send;
    fn delete_image(&self, name: &str) -> impl Future<Output = Result<()>> + Send;
    /// Snapshot a disk and wait until the snapshot is ready.
    fn snapshot_disk(
        &self,
        zone: &str,
        disk: &str,
        snapshot: &str,
    ) -> impl Future<Output = Result<()>> + Send;
    fn delete_snapshot(&self, name: &str) -> impl Future<Output = Result<()>> + Send;
}

/// Where the access token comes from.
#[derive(Clone, Debug)]
pub enum TokenSource {
    Metadata,
    Gcloud,
}

/// The REST client.
#[derive(Clone)]
pub struct Rest {
    http: reqwest::Client,
    project: String,
    source: TokenSource,
    token: Arc<Mutex<Option<(String, Instant)>>>,
}

const API: &str = "https://compute.googleapis.com/compute/v1";

impl Rest {
    pub fn new(project: impl Into<String>, source: TokenSource) -> Self {
        Self {
            http: reqwest::Client::builder()
                .timeout(Duration::from_secs(60))
                .build()
                .expect("http client"),
            project: project.into(),
            source,
            token: Arc::new(Mutex::new(None)),
        }
    }

    async fn token(&self) -> Result<String> {
        let mut held = self.token.lock().await;
        if let Some((t, until)) = held.as_ref()
            && Instant::now() < *until
        {
            return Ok(t.clone());
        }
        let (token, life) = match self.source {
            TokenSource::Metadata => {
                let v: Value = self
                    .http
                    .get("http://metadata.google.internal/computeMetadata/v1/instance/service-accounts/default/token")
                    .header("Metadata-Flavor", "Google")
                    .send()
                    .await
                    .map_err(|_| GceError::new(0, "token", "the metadata server is unreachable"))?
                    .json()
                    .await
                    .map_err(|_| GceError::new(0, "token", "the metadata token is malformed"))?;
                let t = v["access_token"].as_str().unwrap_or_default().to_owned();
                let life = v["expires_in"].as_u64().unwrap_or(600);
                (t, life)
            }
            TokenSource::Gcloud => {
                let out = tokio::process::Command::new("gcloud")
                    .args(["auth", "print-access-token"])
                    .stdin(std::process::Stdio::null())
                    .stderr(std::process::Stdio::null())
                    .output()
                    .await
                    .map_err(|_| GceError::new(0, "token", "gcloud cannot run"))?;
                (String::from_utf8_lossy(&out.stdout).trim().to_owned(), 1800)
            }
        };
        if token.is_empty() {
            return Err(GceError::new(0, "token", "no access token"));
        }
        let until = Instant::now() + Duration::from_secs(life.saturating_sub(300).max(30));
        *held = Some((token.clone(), until));
        Ok(token)
    }

    async fn call(&self, method: reqwest::Method, url: &str, body: Option<Value>) -> Result<Value> {
        let token = self.token().await?;
        let post = method == reqwest::Method::POST;
        let mut req = self.http.request(method, url).bearer_auth(token);
        match body {
            Some(b) => req = req.json(&b),
            // Google answers 411 to a POST without a length.
            None if post => req = req.json(&json!({})),
            None => {}
        }
        let resp = req
            .send()
            .await
            .map_err(|_| GceError::new(0, "transport", "Compute Engine is unreachable"))?;
        let status = resp.status().as_u16();
        let v: Value = resp.json().await.unwrap_or(Value::Null);
        if status >= 400 {
            let code = v
                .pointer("/error/errors/0/reason")
                .and_then(Value::as_str)
                .unwrap_or("error")
                .to_owned();
            let message = v
                .pointer("/error/message")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_owned();
            return Err(GceError::new(status, code, message));
        }
        Ok(v)
    }

    /// Wait for a zonal, regional or global operation; its error becomes ours.
    async fn wait(&self, op: Value, limit: Duration) -> Result<()> {
        let Some(link) = op.get("selfLink").and_then(Value::as_str) else {
            return Ok(());
        };
        let start = Instant::now();
        let mut current = op.clone();
        loop {
            if current.get("status").and_then(Value::as_str) == Some("DONE") {
                if let Some(e) = current.pointer("/error/errors/0") {
                    return Err(GceError::new(
                        409,
                        e.get("code").and_then(Value::as_str).unwrap_or("error"),
                        e.get("message").and_then(Value::as_str).unwrap_or(""),
                    ));
                }
                return Ok(());
            }
            if start.elapsed() > limit {
                return Err(GceError::new(
                    504,
                    "timeout",
                    "the operation is still running",
                ));
            }
            current = self
                .call(reqwest::Method::POST, &format!("{link}/wait"), None)
                .await?;
        }
    }

    fn zone_url(&self, zone: &str, rest: &str) -> String {
        format!("{API}/projects/{}/zones/{zone}/{rest}", self.project)
    }
    fn global_url(&self, rest: &str) -> String {
        format!("{API}/projects/{}/global/{rest}", self.project)
    }
}

fn filter(label: &str, value: &str) -> String {
    format!("labels.{label}={value}")
}

impl Compute for Rest {
    async fn insert_instance(&self, zone: &str, body: Value) -> Result<()> {
        let op = self
            .call(
                reqwest::Method::POST,
                &self.zone_url(zone, "instances"),
                Some(body),
            )
            .await?;
        self.wait(op, Duration::from_secs(300)).await
    }

    async fn get_instance(&self, zone: &str, name: &str) -> Result<Option<Instance>> {
        match self
            .call(
                reqwest::Method::GET,
                &self.zone_url(zone, &format!("instances/{name}")),
                None,
            )
            .await
        {
            Ok(v) => Ok(Some(Instance::from_json(&v))),
            Err(e) if e.not_found() => Ok(None),
            Err(e) => Err(e),
        }
    }

    async fn list_instances(&self, label: &str, value: &str) -> Result<Vec<Instance>> {
        let mut out = Vec::new();
        let mut page = String::new();
        loop {
            let mut url = reqwest::Url::parse(&format!(
                "{API}/projects/{}/aggregated/instances",
                self.project
            ))
            .expect("url");
            url.query_pairs_mut()
                .append_pair("filter", &filter(label, value))
                .append_pair("maxResults", "500");
            if !page.is_empty() {
                url.query_pairs_mut().append_pair("pageToken", &page);
            }
            let v = self.call(reqwest::Method::GET, url.as_str(), None).await?;
            if let Some(items) = v.get("items").and_then(Value::as_object) {
                for scope in items.values() {
                    for i in scope
                        .get("instances")
                        .and_then(Value::as_array)
                        .into_iter()
                        .flatten()
                    {
                        out.push(Instance::from_json(i));
                    }
                }
            }
            match v.get("nextPageToken").and_then(Value::as_str) {
                Some(t) if !t.is_empty() => page = t.to_owned(),
                _ => return Ok(out),
            }
        }
    }

    async fn instance_action(&self, zone: &str, name: &str, action: &str) -> Result<()> {
        let (method, url) = match action {
            "delete" => (
                reqwest::Method::DELETE,
                self.zone_url(zone, &format!("instances/{name}")),
            ),
            "stop" => (
                reqwest::Method::POST,
                self.zone_url(
                    zone,
                    &format!("instances/{name}/stop?discardLocalSsd=false"),
                ),
            ),
            other => (
                reqwest::Method::POST,
                self.zone_url(zone, &format!("instances/{name}/{other}")),
            ),
        };
        self.call(method, &url, None).await.map(|_| ())
    }

    async fn set_labels(
        &self,
        zone: &str,
        name: &str,
        labels: &BTreeMap<String, String>,
        fingerprint: &str,
    ) -> Result<()> {
        let op = self
            .call(
                reqwest::Method::POST,
                &self.zone_url(zone, &format!("instances/{name}/setLabels")),
                Some(json!({"labels": labels, "labelFingerprint": fingerprint})),
            )
            .await?;
        self.wait(op, Duration::from_secs(60)).await
    }

    async fn get_image(&self, name: &str) -> Result<Option<Image>> {
        match self
            .call(
                reqwest::Method::GET,
                &self.global_url(&format!("images/{name}")),
                None,
            )
            .await
        {
            Ok(v) => Ok(Some(Image::from_json(&v))),
            Err(e) if e.not_found() => Ok(None),
            Err(e) => Err(e),
        }
    }

    async fn image_from_family(&self, family: &str) -> Result<Option<Image>> {
        match self
            .call(
                reqwest::Method::GET,
                &self.global_url(&format!("images/family/{family}")),
                None,
            )
            .await
        {
            Ok(v) => Ok(Some(Image::from_json(&v))),
            Err(e) if e.not_found() => Ok(None),
            Err(e) => Err(e),
        }
    }

    async fn list_images(&self, label: &str, value: &str) -> Result<Vec<Image>> {
        let mut url = reqwest::Url::parse(&self.global_url("images")).expect("url");
        url.query_pairs_mut()
            .append_pair("filter", &filter(label, value))
            .append_pair("maxResults", "500");
        let v = self.call(reqwest::Method::GET, url.as_str(), None).await?;
        Ok(v.get("items")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .map(Image::from_json)
            .collect())
    }

    async fn insert_image(&self, body: Value) -> Result<()> {
        // Accepted is enough: the image's own status says when it is ready.
        self.call(
            reqwest::Method::POST,
            &format!("{}?forceCreate=true", self.global_url("images")),
            Some(body),
        )
        .await
        .map(|_| ())
    }

    async fn delete_image(&self, name: &str) -> Result<()> {
        self.call(
            reqwest::Method::DELETE,
            &self.global_url(&format!("images/{name}")),
            None,
        )
        .await
        .map(|_| ())
    }

    async fn snapshot_disk(&self, zone: &str, disk: &str, snapshot: &str) -> Result<()> {
        let op = self
            .call(
                reqwest::Method::POST,
                &self.zone_url(zone, &format!("disks/{disk}/createSnapshot")),
                Some(json!({
                    "name": snapshot,
                    "labels": {"openagents-managed": "oa-boat-fork"},
                })),
            )
            .await?;
        self.wait(op, Duration::from_secs(1800)).await
    }

    async fn delete_snapshot(&self, name: &str) -> Result<()> {
        self.call(
            reqwest::Method::DELETE,
            &self.global_url(&format!("snapshots/{name}")),
            None,
        )
        .await
        .map(|_| ())
    }
}
