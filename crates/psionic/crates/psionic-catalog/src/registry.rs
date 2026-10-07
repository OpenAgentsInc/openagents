use std::{
    fs,
    path::{Path, PathBuf},
};

use reqwest::{
    Url,
    blocking::{Client, RequestBuilder},
    header::{ACCEPT, USER_AGENT},
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{
    BlobError, CatalogError, LocalBlob, LocalBlobKind, LocalBlobOpenOptions, OllamaManifest,
    OllamaManifestLayer, OllamaModelCatalog, OllamaModelName, canonical_ollama_digest,
    ollama::parse_manifest_bytes, ollama_blob_path, ollama_manifest_path,
};

const OCI_MANIFEST_ACCEPT: &str = concat!(
    "application/vnd.docker.distribution.manifest.v2+json,",
    " application/vnd.oci.image.manifest.v1+json,",
    " application/json"
);

/// Transport scheme used for OCI/Docker-registry pull requests.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RegistryScheme {
    /// HTTPS transport.
    #[default]
    Https,
    /// Plain HTTP transport, intended for explicit local/self-hosted development use.
    Http,
}

impl RegistryScheme {
    fn as_str(self) -> &'static str {
        match self {
            Self::Https => "https",
            Self::Http => "http",
        }
    }
}

/// Optional authentication applied to OCI/Docker-registry pull requests.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RegistryAuth {
    /// HTTP bearer token authentication.
    BearerToken(String),
    /// HTTP basic authentication.
    Basic { username: String, password: String },
}

/// Pull options for OCI/Docker-registry ingestion into the local Ollama-style store.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct OllamaRegistryPullOptions {
    /// Transport scheme to use when building registry URLs from the model host.
    pub scheme: RegistryScheme,
    /// Optional request authentication.
    pub auth: Option<RegistryAuth>,
    /// Optional user agent override.
    pub user_agent: Option<String>,
}

/// Blob-level outcome for one pulled OCI manifest layer.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct OllamaRegistryBlobPull {
    /// Canonical blob digest.
    pub digest: String,
    /// Declared media type from the manifest.
    pub media_type: String,
    /// Declared blob size in bytes.
    pub size_bytes: u64,
    /// Local blob path inside the Ollama-style store.
    pub blob_path: PathBuf,
    /// Whether the local blob was already present and validated.
    pub reused_existing: bool,
}

/// Pull outcome for one registry-backed model ingestion operation.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct OllamaRegistryPullReport {
    /// Canonical fully qualified model name that was pulled.
    pub name: OllamaModelName,
    /// Local manifest path written or reused by the pull.
    pub manifest_path: PathBuf,
    /// Stable SHA-256 digest of the manifest payload without the `sha256:` prefix.
    pub manifest_sha256: String,
    /// Manifest byte length.
    pub manifest_byte_length: u64,
    /// Whether the pull wrote or updated the local manifest file.
    pub wrote_manifest: bool,
    /// Blob outcomes in config-then-layer order.
    pub blobs: Vec<OllamaRegistryBlobPull>,
    /// Resolved local manifest after ingestion.
    pub manifest: OllamaManifest,
}

/// Registry pull failures.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum RegistryPullError {
    /// The derived registry base URL was invalid.
    #[error("invalid registry base url `{base_url}`: {message}")]
    InvalidBaseUrl {
        /// Base URL that failed parsing.
        base_url: String,
        /// Validation failure summary.
        message: String,
    },
    /// Sending a registry request failed.
    #[error("registry request failed for `{url}`: {message}")]
    Request {
        /// Target request URL.
        url: String,
        /// Failure summary.
        message: String,
    },
    /// The registry returned a non-success HTTP status.
    #[error("registry request to `{url}` returned status {status}: {message}")]
    HttpStatus {
        /// Target request URL.
        url: String,
        /// HTTP status code.
        status: u16,
        /// Response summary or body snippet.
        message: String,
    },
    /// Creating a local directory for pulled content failed.
    #[error("failed to create directory `{path}`: {message}")]
    CreateDirectory {
        /// Directory path.
        path: String,
        /// Failure summary.
        message: String,
    },
    /// Writing a pulled manifest or blob failed.
    #[error("failed to write file `{path}`: {message}")]
    WriteFile {
        /// Target file path.
        path: String,
        /// Failure summary.
        message: String,
    },
    /// A pulled blob digest did not match its manifest identity.
    #[error("pulled blob digest mismatch for `{url}`: expected `{expected}`, actual `{actual}`")]
    DigestMismatch {
        /// Blob URL.
        url: String,
        /// Expected canonical digest.
        expected: String,
        /// Actual canonical digest.
        actual: String,
    },
    /// A pulled blob byte length did not match its manifest declaration.
    #[error(
        "pulled blob size mismatch for `{url}`: expected {expected_bytes} bytes, actual {actual_bytes} bytes"
    )]
    BlobSizeMismatch {
        /// Blob URL.
        url: String,
        /// Declared byte length from the manifest.
        expected_bytes: u64,
        /// Actual downloaded byte length.
        actual_bytes: u64,
    },
    /// Manifest/catalog parsing failed.
    #[error(transparent)]
    Catalog(#[from] CatalogError),
    /// Existing local blob validation failed.
    #[error(transparent)]
    Blob(#[from] BlobError),
}

/// Minimal OCI/Docker-registry pull client that ingests manifests and blobs into the local Ollama-style store.
#[derive(Clone, Debug, Default)]
pub struct OllamaRegistryClient {
    client: Client,
    options: OllamaRegistryPullOptions,
}

impl OllamaRegistryClient {
    /// Creates a registry client with explicit pull options.
    #[must_use]
    pub fn new(options: OllamaRegistryPullOptions) -> Self {
        Self {
            client: Client::new(),
            options,
        }
    }

    /// Pulls a fully-qualified or defaultable model reference into the provided local models root.
    pub fn pull_model(
        &self,
        models_root: impl AsRef<Path>,
        reference: &str,
    ) -> Result<OllamaRegistryPullReport, RegistryPullError> {
        let name = OllamaModelName::parse(reference)?;
        self.pull_name(models_root, &name)
    }

    /// Pulls an already-normalized model name into the provided local models root.
    pub fn pull_name(
        &self,
        models_root: impl AsRef<Path>,
        name: &OllamaModelName,
    ) -> Result<OllamaRegistryPullReport, RegistryPullError> {
        let models_root = models_root.as_ref();
        let manifest_path = ollama_manifest_path(models_root, name);
        let manifest_url = self.manifest_url(name)?;
        let manifest_bytes = self.fetch_bytes(&manifest_url, Some(OCI_MANIFEST_ACCEPT))?;
        let manifest =
            parse_manifest_bytes(&manifest_bytes, &manifest_path, name.clone(), models_root)?;

        let mut blobs = Vec::new();
        if let Some(config) = manifest.config.as_ref() {
            blobs.push(self.ensure_blob(models_root, name, config)?);
        }
        for layer in &manifest.layers {
            blobs.push(self.ensure_blob(models_root, name, layer)?);
        }

        let wrote_manifest = write_if_changed(&manifest_path, &manifest_bytes)?;
        let manifest = OllamaModelCatalog::new(models_root).resolve_name(name)?;
        Ok(OllamaRegistryPullReport {
            name: name.clone(),
            manifest_path,
            manifest_sha256: manifest.manifest_sha256.clone(),
            manifest_byte_length: manifest.manifest_byte_length,
            wrote_manifest,
            blobs,
            manifest,
        })
    }

    fn ensure_blob(
        &self,
        models_root: &Path,
        name: &OllamaModelName,
        layer: &OllamaManifestLayer,
    ) -> Result<OllamaRegistryBlobPull, RegistryPullError> {
        let blob_path = ollama_blob_path(models_root, layer.digest.as_str())?;
        let open_options =
            LocalBlobOpenOptions::default().with_expected_sha256(layer.digest.as_str());
        let reused_existing = if blob_path.exists() {
            LocalBlob::open_path(&blob_path, LocalBlobKind::OllamaBlob, open_options).is_ok()
        } else {
            false
        };
        if !reused_existing {
            let blob_url = self.blob_url(name, layer.digest.as_str())?;
            let blob_bytes = self.fetch_bytes(&blob_url, None)?;
            let actual_digest = format!(
                "sha256:{}",
                hex::encode(Sha256::digest(blob_bytes.as_slice()))
            );
            let expected_digest = canonical_ollama_digest(layer.digest.as_str())?;
            if actual_digest != expected_digest {
                return Err(RegistryPullError::DigestMismatch {
                    url: blob_url.to_string(),
                    expected: expected_digest,
                    actual: actual_digest,
                });
            }
            let actual_bytes = blob_bytes.len() as u64;
            if layer.size_bytes != 0 && layer.size_bytes != actual_bytes {
                return Err(RegistryPullError::BlobSizeMismatch {
                    url: blob_url.to_string(),
                    expected_bytes: layer.size_bytes,
                    actual_bytes,
                });
            }
            write_if_changed(&blob_path, &blob_bytes)?;
        }

        Ok(OllamaRegistryBlobPull {
            digest: layer.digest.clone(),
            media_type: layer.media_type.raw.clone(),
            size_bytes: layer.size_bytes,
            blob_path,
            reused_existing,
        })
    }

    fn manifest_url(&self, name: &OllamaModelName) -> Result<Url, RegistryPullError> {
        self.base_url(name)?
            .join(
                format!(
                    "/v2/{}/{}/manifests/{}",
                    name.namespace, name.model, name.tag
                )
                .as_str(),
            )
            .map_err(|error| RegistryPullError::InvalidBaseUrl {
                base_url: format!("{}://{}", self.options.scheme.as_str(), name.host),
                message: error.to_string(),
            })
    }

    fn blob_url(&self, name: &OllamaModelName, digest: &str) -> Result<Url, RegistryPullError> {
        self.base_url(name)?
            .join(format!("/v2/{}/{}/blobs/{digest}", name.namespace, name.model).as_str())
            .map_err(|error| RegistryPullError::InvalidBaseUrl {
                base_url: format!("{}://{}", self.options.scheme.as_str(), name.host),
                message: error.to_string(),
            })
    }

    fn base_url(&self, name: &OllamaModelName) -> Result<Url, RegistryPullError> {
        let base_url = format!("{}://{}", self.options.scheme.as_str(), name.host);
        Url::parse(base_url.as_str()).map_err(|error| RegistryPullError::InvalidBaseUrl {
            base_url,
            message: error.to_string(),
        })
    }

    fn request(&self, url: &Url) -> RequestBuilder {
        let mut request = self.client.get(url.clone());
        if let Some(user_agent) = self.options.user_agent.as_deref() {
            request = request.header(USER_AGENT, user_agent);
        }
        match &self.options.auth {
            Some(RegistryAuth::BearerToken(token)) => request.bearer_auth(token),
            Some(RegistryAuth::Basic { username, password }) => {
                request.basic_auth(username, Some(password))
            }
            None => request,
        }
    }

    fn fetch_bytes(&self, url: &Url, accept: Option<&str>) -> Result<Vec<u8>, RegistryPullError> {
        let mut request = self.request(url);
        if let Some(accept) = accept {
            request = request.header(ACCEPT, accept);
        }
        let response = request.send().map_err(|error| RegistryPullError::Request {
            url: url.to_string(),
            message: error.to_string(),
        })?;
        if !response.status().is_success() {
            let status = response.status().as_u16();
            let message = response
                .text()
                .ok()
                .map(|body| body.trim().to_string())
                .filter(|body| !body.is_empty())
                .unwrap_or_else(|| String::from("empty response body"));
            return Err(RegistryPullError::HttpStatus {
                url: url.to_string(),
                status,
                message,
            });
        }
        response
            .bytes()
            .map(|bytes| bytes.to_vec())
            .map_err(|error| RegistryPullError::Request {
                url: url.to_string(),
                message: error.to_string(),
            })
    }
}

fn write_if_changed(path: &Path, bytes: &[u8]) -> Result<bool, RegistryPullError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| RegistryPullError::CreateDirectory {
            path: parent.display().to_string(),
            message: error.to_string(),
        })?;
    }
    if path.exists()
        && fs::read(path)
            .map(|existing| existing == bytes)
            .unwrap_or(false)
    {
        return Ok(false);
    }

    let temp_path = path.with_extension(format!("partial-{}", std::process::id()));
    fs::write(&temp_path, bytes).map_err(|error| RegistryPullError::WriteFile {
        path: temp_path.display().to_string(),
        message: error.to_string(),
    })?;
    fs::rename(&temp_path, path).map_err(|error| RegistryPullError::WriteFile {
        path: path.display().to_string(),
        message: error.to_string(),
    })?;
    Ok(true)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used, clippy::panic_in_result_fn)]

    use std::{
        collections::BTreeMap,
        io::{Read, Write},
        net::{TcpListener, TcpStream},
        sync::{
            Arc,
            atomic::{AtomicBool, Ordering},
        },
        thread,
        time::Duration,
    };

    use serde_json::json;
    use sha2::{Digest, Sha256};
    use tempfile::tempdir;

    use crate::OllamaLayerKind;

    use super::{
        OllamaRegistryClient, OllamaRegistryPullOptions, RegistryPullError, RegistryScheme,
    };

    #[derive(Clone)]
    struct TestHttpResponse {
        status: &'static str,
        body: Vec<u8>,
        content_type: &'static str,
    }

    impl TestHttpResponse {
        fn json(body: Vec<u8>) -> Self {
            Self {
                status: "200 OK",
                body,
                content_type: "application/json",
            }
        }

        fn bytes(body: Vec<u8>) -> Self {
            Self {
                status: "200 OK",
                body,
                content_type: "application/octet-stream",
            }
        }
    }

    struct TestRegistryServer {
        authority: String,
        shutdown: Arc<AtomicBool>,
        handle: Option<thread::JoinHandle<()>>,
    }

    impl TestRegistryServer {
        fn authority(&self) -> &str {
            self.authority.as_str()
        }
    }

    impl Drop for TestRegistryServer {
        fn drop(&mut self) {
            self.shutdown.store(true, Ordering::SeqCst);
            let _ = TcpStream::connect(self.authority.as_str());
            if let Some(handle) = self.handle.take() {
                let _ = handle.join();
            }
        }
    }

    #[test]
    fn registry_client_pulls_manifest_and_reuses_existing_blobs()
    -> Result<(), Box<dyn std::error::Error>> {
        let temp = tempdir()?;
        let config_bytes = br#"{"model_family":"qwen2"}"#.to_vec();
        let model_bytes = b"gguf-model-bytes".to_vec();
        let template_bytes = b"{{ prompt }}".to_vec();
        let config_digest = sha256_digest(config_bytes.as_slice());
        let model_digest = sha256_digest(model_bytes.as_slice());
        let template_digest = sha256_digest(template_bytes.as_slice());
        let manifest_bytes = serde_json::to_vec(&json!({
            "schemaVersion": 2,
            "mediaType": "application/vnd.docker.distribution.manifest.v2+json",
            "config": {
                "mediaType": "application/vnd.docker.container.image.v1+json",
                "digest": config_digest,
                "size": config_bytes.len()
            },
            "layers": [
                {
                    "mediaType": "application/vnd.ollama.image.model",
                    "digest": model_digest,
                    "size": model_bytes.len()
                },
                {
                    "mediaType": "application/vnd.ollama.image.template",
                    "digest": template_digest,
                    "size": template_bytes.len()
                }
            ]
        }))?;

        let server = start_test_registry(BTreeMap::from([
            (
                String::from("/v2/library/qwen2/manifests/latest"),
                TestHttpResponse::json(manifest_bytes.clone()),
            ),
            (
                format!("/v2/library/qwen2/blobs/{config_digest}"),
                TestHttpResponse::bytes(config_bytes.clone()),
            ),
            (
                format!("/v2/library/qwen2/blobs/{model_digest}"),
                TestHttpResponse::bytes(model_bytes.clone()),
            ),
            (
                format!("/v2/library/qwen2/blobs/{template_digest}"),
                TestHttpResponse::bytes(template_bytes.clone()),
            ),
        ]))?;
        let reference = format!("{}/library/qwen2:latest", server.authority());
        let client = OllamaRegistryClient::new(OllamaRegistryPullOptions {
            scheme: RegistryScheme::Http,
            auth: None,
            user_agent: Some(String::from("psionic-catalog-test")),
        });

        let first = client.pull_model(temp.path(), &reference)?;
        assert!(first.wrote_manifest);
        assert_eq!(first.blobs.len(), 3);
        assert!(first.blobs.iter().all(|blob| !blob.reused_existing));
        assert_eq!(
            first
                .manifest
                .first_layer_of_kind(OllamaLayerKind::Template)
                .map(|layer| layer.digest.as_str()),
            Some(template_digest.as_str())
        );
        assert!(first.manifest.layers.iter().all(|layer| layer.blob_present));

        let second = client.pull_model(temp.path(), &reference)?;
        assert!(!second.wrote_manifest);
        assert!(second.blobs.iter().all(|blob| blob.reused_existing));
        assert_eq!(
            second.manifest.manifest_sha256,
            first.manifest.manifest_sha256
        );
        Ok(())
    }

    #[test]
    fn registry_client_rejects_blob_digest_mismatch() -> Result<(), Box<dyn std::error::Error>> {
        let temp = tempdir()?;
        let manifest_bytes = serde_json::to_vec(&json!({
            "schemaVersion": 2,
            "mediaType": "application/vnd.docker.distribution.manifest.v2+json",
            "layers": [
                {
                    "mediaType": "application/vnd.ollama.image.model",
                    "digest": "sha256:1111111111111111111111111111111111111111111111111111111111111111",
                    "size": 5
                }
            ]
        }))?;
        let server = start_test_registry(BTreeMap::from([
            (
                String::from("/v2/library/qwen2/manifests/latest"),
                TestHttpResponse::json(manifest_bytes),
            ),
            (
                String::from(
                    "/v2/library/qwen2/blobs/sha256:1111111111111111111111111111111111111111111111111111111111111111",
                ),
                TestHttpResponse::bytes(b"wrong".to_vec()),
            ),
        ]))?;
        let reference = format!("{}/library/qwen2:latest", server.authority());
        let client = OllamaRegistryClient::new(OllamaRegistryPullOptions {
            scheme: RegistryScheme::Http,
            auth: None,
            user_agent: None,
        });

        let error = client
            .pull_model(temp.path(), &reference)
            .expect_err("digest mismatch should fail");
        assert!(matches!(error, RegistryPullError::DigestMismatch { .. }));
        Ok(())
    }

    fn sha256_digest(bytes: &[u8]) -> String {
        format!("sha256:{}", hex::encode(Sha256::digest(bytes)))
    }

    fn start_test_registry(
        routes: BTreeMap<String, TestHttpResponse>,
    ) -> Result<TestRegistryServer, Box<dyn std::error::Error>> {
        let listener = TcpListener::bind("127.0.0.1:0")?;
        listener.set_nonblocking(true)?;
        let authority = listener.local_addr()?.to_string();
        let shutdown = Arc::new(AtomicBool::new(false));
        let shutdown_flag = Arc::clone(&shutdown);
        let handle = thread::spawn(move || {
            while !shutdown_flag.load(Ordering::SeqCst) {
                match listener.accept() {
                    Ok((mut stream, _)) => {
                        let _ = handle_connection(&mut stream, &routes);
                    }
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                        thread::sleep(Duration::from_millis(10));
                    }
                    Err(_) => break,
                }
            }
        });
        Ok(TestRegistryServer {
            authority,
            shutdown,
            handle: Some(handle),
        })
    }

    fn handle_connection(
        stream: &mut TcpStream,
        routes: &BTreeMap<String, TestHttpResponse>,
    ) -> std::io::Result<()> {
        let mut request = Vec::new();
        let mut buffer = [0_u8; 1024];
        loop {
            let read = stream.read(&mut buffer)?;
            if read == 0 {
                break;
            }
            request.extend_from_slice(&buffer[..read]);
            if request.windows(4).any(|window| window == b"\r\n\r\n") {
                break;
            }
        }
        let request = String::from_utf8_lossy(&request);
        let path = request
            .lines()
            .next()
            .and_then(|line| line.split_whitespace().nth(1))
            .unwrap_or("/");
        let response = routes
            .get(path)
            .cloned()
            .unwrap_or_else(|| TestHttpResponse {
                status: "404 Not Found",
                body: b"missing".to_vec(),
                content_type: "text/plain",
            });
        write!(
            stream,
            "HTTP/1.1 {}\r\nContent-Length: {}\r\nContent-Type: {}\r\nConnection: close\r\n\r\n",
            response.status,
            response.body.len(),
            response.content_type
        )?;
        stream.write_all(&response.body)?;
        stream.flush()
    }
}
