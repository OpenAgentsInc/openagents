//! Explicit Hub publication of one reviewed, hashed export. Recording and
//! export never call this module. API credentials are not sent to storage URLs.
use crate::{
    Error, Result,
    dataset::{FilePin, read_file, resolve},
    export::Export,
    require,
};
use base64::{Engine, engine::general_purpose::STANDARD};
use reqwest::{Client, Response, Url};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{collections::BTreeMap, path::Path, time::Duration};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Plan {
    pub schema: String,
    pub manifest_sha256: String,
    pub repository: String,
    pub parent: String,
    pub license: String,
    pub notice: String,
    pub files: BTreeMap<String, FilePin>,
}
pub struct Snapshot {
    pub plan: Plan,
    files: BTreeMap<String, Vec<u8>>,
}
/// Freeze verified bytes before any upload. Neither changed files nor files
/// omitted from the export manifest can join the resulting commit.
pub fn prepare(
    root: &Path,
    repository: &str,
    parent: &str,
    expected_manifest: &str,
) -> Result<Snapshot> {
    require(
        root.is_absolute(),
        "Choose the absolute folder of a saved dataset.",
    )?;
    let root = std::fs::canonicalize(root)?;
    require(
        repository.split('/').count() == 2
            && repository.split('/').all(|part| {
                !part.is_empty()
                    && part.len() <= 96
                    && part != "."
                    && part != ".."
                    && part
                        .bytes()
                        .all(|b| b.is_ascii_alphanumeric() || b"_.-".contains(&b))
            }),
        "Choose a Hub dataset as OWNER/NAME.",
    )?;
    require(
        parent.len() == 40
            && parent.bytes().all(|b| b.is_ascii_hexdigit())
            && expected_manifest.len() == 64,
        "Choose the reviewed dataset hash and current Hub commit.",
    )?;
    let bytes = read_file(&resolve(&root, "export.json")?, 2 << 20)?;
    let hash = format!("{:x}", Sha256::digest(&bytes));
    require(
        hash == expected_manifest,
        "The dataset changed after review. Read its export again before publishing.",
    )?;
    let export: Export = serde_json::from_slice(&bytes)?;
    require(
        export.schema == "robot-export.v1"
            && !export.spec.license.is_empty()
            && !export.spec.notice.is_empty()
            && !export.spec.license_text.is_empty()
            && export.files.len() <= 10_000,
        "Publish a recorded export with its license and notices.",
    )?;
    let mut files = BTreeMap::new();
    let mut pins = export.files.clone();
    let mut total = bytes.len();
    for (name, pin) in &export.files {
        require(
            name == "LICENSE"
                || name == "NOTICE"
                || name == "README.md"
                || name.starts_with("meta/")
                || name.starts_with("data/"),
            "Publish only the dataset's declared metadata and recordings.",
        )?;
        require(
            pin.bytes <= 64 << 20,
            "Publish episode files smaller than 64 MB.",
        )?;
        let path = resolve(&root, name)?;
        require(
            std::fs::symlink_metadata(&path)?.is_file(),
            "Publish ordinary files saved in this dataset.",
        )?;
        let data = read_file(&path, pin.bytes as usize)?;
        require(
            data.len() as u64 == pin.bytes && format!("{:x}", Sha256::digest(&data)) == pin.sha256,
            "A dataset file changed. Export it again before publishing.",
        )?;
        total = total.checked_add(data.len()).unwrap_or(usize::MAX);
        require(
            total <= 384 << 20,
            "Publish at most 384 MB per recorded export.",
        )?;
        files.insert(name.clone(), data);
    }
    require(
        files
            .get("LICENSE")
            .is_some_and(|v| v == export.spec.license_text.as_bytes())
            && files
                .get("NOTICE")
                .is_some_and(|v| v == export.spec.notice.as_bytes()),
        "Keep the reviewed license and notices with this dataset.",
    )?;
    pins.insert(
        "export.json".into(),
        FilePin {
            sha256: hash.clone(),
            bytes: bytes.len() as u64,
        },
    );
    files.insert("export.json".into(), bytes);
    Ok(Snapshot {
        plan: Plan {
            schema: "robot-publication.v1".into(),
            manifest_sha256: hash,
            repository: repository.into(),
            parent: parent.into(),
            license: export.spec.license,
            notice: export.spec.notice,
            files: pins,
        },
        files,
    })
}

pub struct Hub {
    client: Client,
    origin: String,
}
impl Hub {
    pub fn new() -> Result<Self> {
        Self::at("https://huggingface.co")
    }
    fn at(origin: &str) -> Result<Self> {
        let client = Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .connect_timeout(Duration::from_secs(10))
            .timeout(Duration::from_secs(300))
            .build()
            .map_err(|_| failure("The Hub connection couldn't start"))?;
        Ok(Self {
            client,
            origin: origin.into(),
        })
    }
    /// The caller invokes publication explicitly and chooses whether the
    /// reviewed export may go to an already-public dataset repository.
    pub async fn publish(
        &self,
        snapshot: &Snapshot,
        token: &str,
        allow_public: bool,
    ) -> Result<Value> {
        require(
            !token.trim().is_empty() && token.len() <= 4096 && !token.contains(['\n', '\r']),
            "Set HF_TOKEN before publishing to the Hub.",
        )?;
        let repo = &snapshot.plan.repository;
        let info = self
            .json(
                self.client
                    .get(format!("{}/api/datasets/{repo}", self.origin))
                    .bearer_auth(token)
                    .send()
                    .await,
            )
            .await?;
        require(
            info["sha"].as_str() == Some(&snapshot.plan.parent),
            "The Hub dataset changed. Review its current commit before publishing.",
        )?;
        require(
            info["private"] == true || allow_public,
            "This Hub dataset is public. Choose --allow-public to share the reviewed recording.",
        )?;
        let files:Vec<_>=snapshot.files.iter().map(|(name,data)|json!({"path":name,"size":data.len(),"sample":STANDARD.encode(&data[..data.len().min(512)])})).collect();
        let preupload = self
            .json(
                self.client
                    .post(format!(
                        "{}/api/datasets/{repo}/preupload/main",
                        self.origin
                    ))
                    .bearer_auth(token)
                    .json(&json!({"files":files}))
                    .send()
                    .await,
            )
            .await?;
        let modes = preupload["files"]
            .as_array()
            .ok_or_else(|| failure("The Hub didn't describe the upload"))?;
        require(
            modes.len() == snapshot.files.len(),
            "The Hub didn't accept every required dataset file.",
        )?;
        let mut uploads = BTreeMap::new();
        for file in modes {
            let path = file["path"]
                .as_str()
                .ok_or_else(|| failure("The Hub didn't describe the upload"))?;
            let mode = file["uploadMode"]
                .as_str()
                .ok_or_else(|| failure("The Hub didn't describe the upload"))?;
            require(
                snapshot.files.contains_key(path)
                    && uploads.insert(path.to_owned(), mode.to_owned()).is_none()
                    && matches!(mode, "regular" | "lfs")
                    && file["shouldIgnore"] != true,
                "The Hub excluded or changed a required dataset file.",
            )?;
        }
        let mut body = serde_json::to_string(
            &json!({"key":"header","value":{"summary":"Publish recorded Robot Lab dataset","description":format!("Reviewed export {}",snapshot.plan.manifest_sha256),"parentCommit":snapshot.plan.parent}}),
        )?;
        body.push('\n');
        for (name, bytes) in &snapshot.files {
            let file = if uploads[name] == "lfs" {
                let pin = &snapshot.plan.files[name];
                self.lfs(repo, pin, bytes, token).await?;
                json!({"key":"lfsFile","value":{"path":name,"algo":"sha256","oid":pin.sha256,"size":pin.bytes}})
            } else {
                json!({"key":"file","value":{"path":name,"encoding":"base64","content":STANDARD.encode(bytes)}})
            };
            body.push_str(&serde_json::to_string(&file)?);
            body.push('\n');
            require(
                body.len() <= 512 << 20,
                "This publication exceeds the upload size limit.",
            )?;
        }
        let reply = self
            .json(
                self.client
                    .post(format!("{}/api/datasets/{repo}/commit/main", self.origin))
                    .bearer_auth(token)
                    .header("Content-Type", "application/x-ndjson")
                    .body(body)
                    .send()
                    .await,
            )
            .await?;
        let commit = reply["commitOid"]
            .as_str()
            .ok_or_else(|| failure("The Hub didn't confirm the dataset commit"))?;
        require(
            commit.len() == 40 && commit.bytes().all(|b| b.is_ascii_hexdigit()),
            "The Hub didn't confirm the dataset commit.",
        )?;
        Ok(
            json!({"repository":repo,"commit":commit,"url":format!("https://huggingface.co/datasets/{repo}/commit/{commit}"),"manifest_sha256":snapshot.plan.manifest_sha256}),
        )
    }
    async fn lfs(&self, repo: &str, pin: &FilePin, bytes: &[u8], token: &str) -> Result<()> {
        let batch=self.json(self.client.post(format!("{}/datasets/{repo}.git/info/lfs/objects/batch",self.origin)).bearer_auth(token).header("Accept","application/vnd.git-lfs+json").header("Content-Type","application/vnd.git-lfs+json").json(&json!({"operation":"upload","transfers":["basic","multipart"],"objects":[{"oid":pin.sha256,"size":pin.bytes}],"hash_algo":"sha256","ref":{"name":"main"}})).send().await).await?;
        let objects = batch["objects"]
            .as_array()
            .ok_or_else(|| failure("The Hub didn't describe its storage upload"))?;
        require(
            objects.len() == 1,
            "The Hub didn't accept the recorded file.",
        )?;
        let object = &objects[0];
        require(
            object["oid"] == pin.sha256
                && object["size"] == pin.bytes
                && object.get("error").is_none(),
            "The Hub refused the recorded file.",
        )?;
        if let Some(upload) = object.get("actions").and_then(|a| a.get("upload")) {
            let url = self.storage_url(
                upload["href"]
                    .as_str()
                    .ok_or_else(|| failure("The Hub omitted its storage address"))?,
            )?;
            if let Some(chunk) = upload["header"]["chunk_size"].as_str() {
                let chunk: usize = chunk
                    .parse()
                    .map_err(|_| failure("The Hub returned an invalid upload size"))?;
                require(
                    (1..=64 << 20).contains(&chunk),
                    "The Hub returned an invalid upload size.",
                )?;
                let count = bytes.len().div_ceil(chunk);
                let headers = upload["header"]
                    .as_object()
                    .ok_or_else(|| failure("The Hub omitted its upload parts"))?;
                require(
                    headers
                        .keys()
                        .filter(|key| key.bytes().all(|b| b.is_ascii_digit()))
                        .count()
                        == count
                        && count <= 1000,
                    "The Hub omitted a recorded file part.",
                )?;
                let mut parts = Vec::new();
                for (index, part) in bytes.chunks(chunk).enumerate() {
                    let part_url = self.storage_url(
                        headers
                            .get(&(index + 1).to_string())
                            .and_then(|v| v.as_str())
                            .ok_or_else(|| failure("The Hub omitted a recorded file part"))?,
                    )?;
                    let response = self
                        .response(self.client.put(part_url).body(part.to_vec()).send().await)
                        .await?;
                    let etag = response
                        .headers()
                        .get("etag")
                        .and_then(|v| v.to_str().ok())
                        .filter(|v| !v.is_empty() && v.len() <= 1024)
                        .ok_or_else(|| failure("The Hub didn't confirm a recorded file part"))?;
                    parts.push(json!({"partNumber":index+1,"etag":etag}));
                }
                self.response(
                    self.client
                        .post(url)
                        .header("Content-Type", "application/vnd.git-lfs+json")
                        .json(&json!({"oid":pin.sha256,"parts":parts}))
                        .send()
                        .await,
                )
                .await?;
            } else {
                self.response(self.client.put(url).body(bytes.to_vec()).send().await)
                    .await?;
            }
        }
        if let Some(verify) = object.get("actions").and_then(|a| a.get("verify")) {
            let url = self.storage_url(
                verify["href"]
                    .as_str()
                    .ok_or_else(|| failure("The Hub omitted its verification address"))?,
            )?;
            require(
                url.origin().ascii_serialization() == self.origin,
                "The Hub returned an unsupported verification address.",
            )?;
            let mut request = self
                .client
                .post(url)
                .bearer_auth(token)
                .json(&json!({"oid":pin.sha256,"size":pin.bytes}));
            if let Some(headers) = verify["header"].as_object() {
                for (name, value) in headers {
                    require(
                        !name.eq_ignore_ascii_case("authorization"),
                        "The Hub requested an unsupported verification credential.",
                    )?;
                    request = request.header(
                        name,
                        value.as_str().ok_or_else(|| {
                            failure("The Hub returned an invalid verification header")
                        })?,
                    );
                }
            }
            self.response(request.send().await).await?;
        }
        Ok(())
    }
    fn storage_url(&self, text: &str) -> Result<Url> {
        let url =
            Url::parse(text).map_err(|_| failure("The Hub returned an invalid storage address"))?;
        let host = url.host_str().unwrap_or("");
        let allowed = host == "huggingface.co"
            || [
                ".huggingface.co",
                ".hf.co",
                ".amazonaws.com",
                ".cloudfront.net",
            ]
            .iter()
            .any(|suffix| host.ends_with(suffix));
        #[cfg(test)]
        let allowed = allowed
            || (self.origin.starts_with("http://127.0.0.1:")
                && text.starts_with(&format!("{}/", self.origin)));
        #[cfg(test)]
        let https = url.scheme() == "https"
            || (url.scheme() == "http" && text.starts_with(&format!("{}/", self.origin)));
        #[cfg(not(test))]
        let https = url.scheme() == "https";
        require(
            allowed
                && https
                && url.username().is_empty()
                && url.password().is_none()
                && url.fragment().is_none(),
            "The Hub returned an unsupported storage address.",
        )?;
        Ok(url)
    }
    async fn response(
        &self,
        response: std::result::Result<Response, reqwest::Error>,
    ) -> Result<Response> {
        let response = response.map_err(|_| {
            failure("The upload couldn't reach the Hub. Try publishing the same export again")
        })?;
        require(
            response.status().is_success(),
            &format!(
                "The Hub didn't accept the upload (HTTP {}).",
                response.status().as_u16()
            ),
        )?;
        Ok(response)
    }
    async fn json(&self, response: std::result::Result<Response, reqwest::Error>) -> Result<Value> {
        let mut response = self.response(response).await?;
        let mut bytes = Vec::new();
        while let Some(chunk) = response
            .chunk()
            .await
            .map_err(|_| failure("The Hub reply couldn't be read"))?
        {
            require(
                bytes.len().saturating_add(chunk.len()) <= 1 << 20,
                "The Hub reply is too large.",
            )?;
            bytes.extend_from_slice(&chunk);
        }
        serde_json::from_slice(&bytes).map_err(|_| failure("The Hub reply couldn't be read"))
    }
}
fn failure(text: &str) -> Error {
    Error::Invalid(text.into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{
        Json, Router,
        body::Bytes,
        extract::State,
        http::{HeaderMap, StatusCode},
        response::IntoResponse,
        routing::{get, post, put},
    };
    use std::sync::{Arc, Mutex};

    fn fixture(root: &Path) -> Snapshot {
        use crate::export::{Episode, Frame, Spec, Termination, Unit};
        let spec = Spec {
            robot_type: "fixture".into(),
            joints: vec!["shoulder.pos".into()],
            units: vec![Unit::Radians],
            fps: 30,
            task: "Move the cube".into(),
            cameras: BTreeMap::new(),
            license: "cc0-1.0".into(),
            license_text: "Original fixture under CC0-1.0".into(),
            notice: "Original test states; no imported assets".into(),
            source: json!({"fixture":true}),
        };
        crate::export::write(
            root,
            &spec,
            &[Episode {
                frames: vec![Frame {
                    state: vec![0.],
                    action: vec![0.2],
                    cameras: BTreeMap::new(),
                }],
                termination: Termination::Complete,
            }],
        )
        .unwrap();
        let hash = format!(
            "{:x}",
            Sha256::digest(std::fs::read(root.join("export.json")).unwrap())
        );
        prepare(root, "fixture/robot-lab", &"a".repeat(40), &hash).unwrap()
    }
    #[derive(Clone)]
    struct Mock {
        origin: String,
        private: bool,
        calls: Arc<Mutex<Vec<(String, bool, Vec<u8>)>>>,
    }
    impl Mock {
        fn remember(&self, path: &str, headers: &HeaderMap, body: &[u8]) {
            self.calls.lock().unwrap().push((
                path.into(),
                headers.contains_key("authorization"),
                body.to_vec(),
            ));
        }
    }
    async fn info(State(mock): State<Mock>, headers: HeaderMap) -> Json<Value> {
        mock.remember("info", &headers, &[]);
        Json(json!({"id":"fixture/robot-lab","sha":"a".repeat(40),"private":mock.private}))
    }
    async fn preupload(
        State(mock): State<Mock>,
        headers: HeaderMap,
        Json(body): Json<Value>,
    ) -> Json<Value> {
        mock.remember("preupload", &headers, &[]);
        Json(
            json!({"files":body["files"].as_array().unwrap().iter().map(|f|json!({"path":f["path"],"uploadMode":if f["path"]=="NOTICE"||f["path"].as_str().unwrap().starts_with("data/"){"lfs"}else{"regular"},"shouldIgnore":false})).collect::<Vec<_>>() }),
        )
    }
    async fn batch(
        State(mock): State<Mock>,
        headers: HeaderMap,
        Json(body): Json<Value>,
    ) -> Json<Value> {
        mock.remember("batch", &headers, &[]);
        let object = &body["objects"][0];
        let size = object["size"].as_u64().unwrap();
        let upload = if size < 1024 {
            json!({"href":format!("{}/storage/basic",mock.origin)})
        } else {
            let mut parts = serde_json::Map::new();
            parts.insert("chunk_size".into(), "1024".into());
            for i in 1..=size.div_ceil(1024) {
                parts.insert(
                    i.to_string(),
                    format!("{}/storage/part", mock.origin).into(),
                );
            }
            json!({"href":format!("{}/storage/complete",mock.origin),"header":parts})
        };
        Json(
            json!({"objects":[{"oid":object["oid"],"size":size,"actions":{"upload":upload,"verify":{"href":format!("{}/verify",mock.origin)}}}]}),
        )
    }
    async fn basic(State(mock): State<Mock>, headers: HeaderMap, body: Bytes) -> impl IntoResponse {
        mock.remember("basic", &headers, &body);
        StatusCode::OK
    }
    async fn part(State(mock): State<Mock>, headers: HeaderMap, body: Bytes) -> impl IntoResponse {
        mock.remember("part", &headers, &body);
        ([(reqwest::header::ETAG, "fixture-part")], StatusCode::OK)
    }
    async fn complete(
        State(mock): State<Mock>,
        headers: HeaderMap,
        body: Bytes,
    ) -> impl IntoResponse {
        mock.remember("complete", &headers, &body);
        StatusCode::OK
    }
    async fn verify(
        State(mock): State<Mock>,
        headers: HeaderMap,
        body: Bytes,
    ) -> impl IntoResponse {
        mock.remember("verify", &headers, &body);
        StatusCode::OK
    }
    async fn commit(State(mock): State<Mock>, headers: HeaderMap, body: Bytes) -> Json<Value> {
        mock.remember("commit", &headers, &body);
        Json(json!({"commitOid":"b".repeat(40)}))
    }
    async fn mock(private: bool) -> (Hub, Mock, tokio::task::JoinHandle<()>) {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let origin = format!("http://{}", listener.local_addr().unwrap());
        let mock = Mock {
            origin: origin.clone(),
            private,
            calls: Arc::default(),
        };
        let router = Router::new()
            .route("/api/datasets/fixture/robot-lab", get(info))
            .route(
                "/api/datasets/fixture/robot-lab/preupload/main",
                post(preupload),
            )
            .route(
                "/datasets/fixture/robot-lab.git/info/lfs/objects/batch",
                post(batch),
            )
            .route("/storage/basic", put(basic))
            .route("/storage/part", put(part))
            .route("/storage/complete", post(complete))
            .route("/verify", post(verify))
            .route("/api/datasets/fixture/robot-lab/commit/main", post(commit))
            .with_state(mock.clone());
        let task = tokio::spawn(async move {
            axum::serve(listener, router).await.unwrap();
        });
        (Hub::at(&origin).unwrap(), mock, task)
    }
    #[tokio::test]
    async fn explicit_publication_pins_parent_and_keeps_credentials_off_storage_uploads() {
        let root = tempfile::tempdir().unwrap();
        let snapshot = fixture(&root.path().join("dataset"));
        let (hub, mock, task) = mock(true).await;
        let reply = hub.publish(&snapshot, "test-only", false).await.unwrap();
        assert_eq!(reply["commit"], "b".repeat(40));
        let calls = mock.calls.lock().unwrap();
        assert!(calls.iter().any(|(path, _, _)| path == "basic"));
        assert!(calls.iter().any(|(path, _, _)| path == "part"));
        for (path, credential, _) in calls.iter() {
            assert_eq!(
                *credential,
                matches!(
                    path.as_str(),
                    "info" | "preupload" | "batch" | "verify" | "commit"
                ),
                "{path}"
            );
        }
        let body = &calls
            .iter()
            .find(|(path, _, _)| path == "commit")
            .unwrap()
            .2;
        let lines = std::str::from_utf8(body)
            .unwrap()
            .lines()
            .map(|line| serde_json::from_str::<Value>(line).unwrap())
            .collect::<Vec<_>>();
        assert_eq!(lines[0]["value"]["parentCommit"], "a".repeat(40));
        assert_eq!(lines.len(), snapshot.files.len() + 1);
        assert!(lines.iter().any(|line| line["key"] == "lfsFile"));
        drop(calls);
        task.abort();
    }
    #[tokio::test]
    async fn a_public_repository_needs_the_explicit_public_choice() {
        let root = tempfile::tempdir().unwrap();
        let snapshot = fixture(&root.path().join("dataset"));
        let (hub, mock, task) = mock(false).await;
        assert!(hub.publish(&snapshot, "test-only", false).await.is_err());
        assert_eq!(mock.calls.lock().unwrap().len(), 1);
        task.abort();
    }
    #[test]
    fn changed_files_and_unreviewed_manifest_cannot_be_published() {
        let root = tempfile::tempdir().unwrap();
        let dataset = root.path().join("dataset");
        let snapshot = fixture(&dataset);
        assert!(
            prepare(
                &dataset,
                "fixture/robot-lab",
                &"a".repeat(40),
                &"0".repeat(64)
            )
            .is_err()
        );
        std::fs::write(dataset.join("NOTICE"), "changed").unwrap();
        assert!(
            prepare(
                &dataset,
                "fixture/robot-lab",
                &"a".repeat(40),
                &snapshot.plan.manifest_sha256
            )
            .is_err()
        );
        assert!(
            Hub::new()
                .unwrap()
                .storage_url("https://huggingface.co.evil.example/upload")
                .is_err()
        );
    }
}
