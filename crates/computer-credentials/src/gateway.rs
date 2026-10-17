//! Host-only SPACE adapter. No resolved value or node credential enters the VM.
use crate::{
    Binding, Broker, Failure, Secret, Use, opaque,
    policy::{Host, Profile},
};
use async_trait::async_trait;
use axum::{
    Json, Router,
    extract::{DefaultBodyLimit, State},
    http::{HeaderMap, StatusCode, header},
    response::{IntoResponse, Response},
    routing::post,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{collections::BTreeMap, sync::Arc, time::Duration};
use subtle::ConstantTimeEq;
use uuid::Uuid;
use zeroize::{Zeroize, Zeroizing};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Context {
    pub native_resource: String,
    pub node: String,
    /// Zero asks the host broker to derive the current execution from authority.
    /// Core credential uses always carry the positive generation it returns.
    pub execution_generation: u64,
    pub native_policy_ref: String,
}
impl Context {
    pub fn validate(&self) -> Result<(), Failure> {
        if !opaque(&self.native_resource, 128)
            || !opaque(&self.node, 128)
            || !opaque(&self.native_policy_ref, 256)
        {
            return Err(Failure::Denied);
        }
        Ok(())
    }
    fn request(&self, id: Uuid, r: &Request) -> Use {
        Use {
            reference: id,
            native_resource: self.native_resource.clone(),
            node: self.node.clone(),
            execution_generation: self.execution_generation,
            native_policy_ref: self.native_policy_ref.clone(),
            host: r.host.clone(),
            port: r.port,
            path: r.path.clone(),
            method: r.method.clone(),
            tls_inspected: true,
        }
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Request {
    pub context: Context,
    pub host: String,
    pub port: u16,
    pub path: String,
    pub method: String,
    #[serde(default)]
    pub scope: Option<String>,
    #[serde(default)]
    pub selectors: BTreeMap<String, Uuid>,
}
/// Computed from current computer, command, grant, account, and native state.
/// Revoked or missing authority is an error, including when no rule matches.
#[derive(PartialEq, Eq)]
pub struct Admission {
    pub profile: Profile,
    pub references: Vec<Uuid>,
}
#[async_trait]
pub trait Authority: Send + Sync {
    async fn admit(&self, context: &Context) -> Result<Admission, Failure>;
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Header {
    pub name: String,
    pub value: String,
}
impl Drop for Header {
    fn drop(&mut self) {
        self.value.zeroize();
    }
}
impl std::fmt::Debug for Header {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Header")
            .field("name", &self.name)
            .finish_non_exhaustive()
    }
}

pub struct Service {
    pub broker: Arc<Broker>,
    pub authority: Arc<dyn Authority>,
    /// SHA-256(node token) -> node identity. Tokens are issued only to the host.
    pub nodes: BTreeMap<[u8; 32], String>,
}
impl Service {
    fn node(&self, headers: &HeaderMap, context: &Context) -> Result<(), Failure> {
        let values = headers.get_all(header::AUTHORIZATION);
        if values.iter().count() != 1 {
            return Err(Failure::Denied);
        }
        let token = values
            .iter()
            .next()
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.strip_prefix("Bearer "))
            .ok_or(Failure::Denied)?;
        let hash: [u8; 32] = Sha256::digest(token.as_bytes()).into();
        // Compare all configured identities so unknown tokens share the path.
        let permitted = self
            .nodes
            .iter()
            .any(|(key, node)| key.ct_eq(&hash).unwrap_u8() == 1 && node == &context.node);
        if !permitted {
            return Err(Failure::Denied);
        }
        context.validate()
    }
    async fn admission(
        &self,
        headers: &HeaderMap,
        c: &Context,
    ) -> Result<(Admission, String), Failure> {
        self.node(headers, c)?;
        let a = self.authority.admit(c).await?;
        a.profile.validate().map_err(|_| Failure::Denied)?;
        if a.references.len() > 16 {
            return Err(Failure::Denied);
        }
        let mut seen = std::collections::BTreeSet::new();
        let mut scope = Sha256::new();
        scope.update(b"openagents.computers.credential-stream.v1\0");
        for id in &a.references {
            if !seen.insert(*id) {
                return Err(Failure::Denied);
            }
            let b = self.broker.store.get(*id).await?.ok_or(Failure::Denied)?;
            if b.profile != a.profile || !same_context(&b, c) {
                return Err(Failure::Denied);
            }
            let source = self.broker.sources.get(&b.source).ok_or(Failure::Denied)?;
            let rule = source.rules.get(&b.rule).ok_or(Failure::Denied)?;
            let method = rule
                .destination
                .methods
                .iter()
                .next()
                .ok_or(Failure::Denied)?;
            let method = serde_json::to_value(method).map_err(|_| Failure::Denied)?;
            let request = Request {
                context: c.clone(),
                host: rule.destination.host.as_str().into(),
                port: rule.destination.port,
                path: rule.destination.path_prefix.clone(),
                method: method.as_str().ok_or(Failure::Denied)?.into(),
                scope: None,
                selectors: BTreeMap::new(),
            };
            let token = self.broker.proxy_token(*id).await?;
            self.broker
                .check(token.expose(), &bound_context(&b).request(*id, &request))
                .await?;
            scope.update(serde_json::to_vec(&b).map_err(|_| Failure::Denied)?);
        }
        Ok((a, format!("{:x}", scope.finalize())))
    }
    async fn resolve(&self, headers: &HeaderMap, r: &Request) -> Result<Vec<Header>, Failure> {
        let (a, scope) = self.admission(headers, &r.context).await?;
        if r.scope.as_ref().is_some_and(|expected| expected != &scope) {
            return Err(Failure::Denied);
        }
        let host = Host::parse(&r.host).map_err(|_| Failure::Denied)?;
        if !a.profile.allow_hosts.contains(&host)
            || a.profile.tls_exempt_hosts.contains(&host)
            || r.port == 0
            || !crate::policy::canonical_path(&r.path)
            || crate::policy::Method::parse(&r.method).is_none()
        {
            return Err(Failure::Denied);
        }
        if r.selectors.len() > 3 {
            return Err(Failure::Denied);
        }
        // A reference is only a selector. Node authentication and fresh account,
        // computer, command, and source authority are still required at use.
        let mut selected = Vec::new();
        for (header, id) in &r.selectors {
            if !a.references.contains(id) {
                return Err(Failure::Denied);
            }
            let b = self.broker.store.get(*id).await?.ok_or(Failure::Denied)?;
            if b.profile != a.profile || !same_context(&b, &r.context) {
                return Err(Failure::Denied);
            }
            let source = self.broker.sources.get(&b.source).ok_or(Failure::Denied)?;
            let rule = source.rules.get(&b.rule).ok_or(Failure::Denied)?;
            if &rule.header != header
                || !rule
                    .destination
                    .matches(&r.host, r.port, &r.path, &r.method)
            {
                return Err(Failure::Denied);
            }
            selected.push(b);
        }
        let mut result = BTreeMap::new();
        for b in selected {
            let token = self.broker.proxy_token(b.id).await?;
            let (name, value) = self
                .broker
                .resolve(token.expose(), &bound_context(&b).request(b.id, r))
                .await?;
            let value = std::str::from_utf8(value.bytes())
                .map_err(|_| Failure::Invalid)?
                .to_owned();
            if result.insert(name, Zeroizing::new(value)).is_some() {
                return Err(Failure::Denied);
            }
        }
        // No header is sent if authority changed while any provider was read.
        let (after, after_scope) = self.admission(headers, &r.context).await?;
        if a != after || scope != after_scope {
            return Err(Failure::Denied);
        }
        Ok(result
            .into_iter()
            .map(|(name, mut value)| Header {
                name,
                value: std::mem::take(&mut *value),
            })
            .collect())
    }
    pub fn router(self: Arc<Self>) -> Router {
        Router::new()
            .route("/v1/resolve", post(resolve))
            .route("/v1/check", post(check))
            .layer(DefaultBodyLimit::max(16384))
            .with_state(self)
    }
}
fn bound_context(b: &Binding) -> Context {
    Context {
        native_resource: b.native_resource.clone(),
        node: b.node.clone(),
        execution_generation: b.execution_generation,
        native_policy_ref: b.native_policy_ref.clone(),
    }
}
fn same_context(b: &Binding, c: &Context) -> bool {
    b.native_resource == c.native_resource
        && b.node == c.node
        && (c.execution_generation == 0 || b.execution_generation == c.execution_generation)
        && b.native_policy_ref == c.native_policy_ref
}
fn error(f: Failure) -> Response {
    let status = if f == Failure::Unavailable {
        StatusCode::SERVICE_UNAVAILABLE
    } else {
        StatusCode::FORBIDDEN
    };
    (
        status,
        [(header::CACHE_CONTROL, "no-store")],
        Json(serde_json::json!({"error":"credential_unavailable"})),
    )
        .into_response()
}
async fn resolve(
    State(service): State<Arc<Service>>,
    headers: HeaderMap,
    Json(request): Json<Request>,
) -> Response {
    match service.resolve(&headers, &request).await {
        Ok(headers) => (
            StatusCode::OK,
            [(header::CACHE_CONTROL, "no-store")],
            Json(headers),
        )
            .into_response(),
        Err(f) => error(f),
    }
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct CheckRequest {
    context: Context,
    #[serde(default)]
    scope: Option<String>,
}
async fn check(
    State(service): State<Arc<Service>>,
    headers: HeaderMap,
    Json(request): Json<CheckRequest>,
) -> Response {
    match service.admission(&headers, &request.context).await {
        Ok((_, scope))
            if request
                .scope
                .as_ref()
                .is_none_or(|expected| expected == &scope) =>
        {
            (
                StatusCode::OK,
                [(header::CACHE_CONTROL, "no-store")],
                Json(serde_json::json!({"scope":scope})),
            )
                .into_response()
        }
        Ok(_) => error(Failure::Denied),
        Err(f) => error(f),
    }
}

/// Used by the trusted per-sandbox gateway process on the host.
/// Every HTTP request makes a fresh broker call; redirects are never followed.
pub struct Client {
    http: reqwest::Client,
    origin: String,
    token: Secret,
    context: Context,
    root_certificate: Option<reqwest::Certificate>,
    socket: Option<std::path::PathBuf>,
}
impl std::fmt::Debug for Client {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GatewayClient")
            .field("context", &self.context)
            .finish_non_exhaustive()
    }
}
impl Client {
    #[cfg(test)]
    pub(crate) fn local(origin: &str, token: Secret, context: Context) -> Result<Self, Failure> {
        let url = url::Url::parse(origin).map_err(|_| Failure::Invalid)?;
        if url.scheme() != "http"
            || url.host_str() != Some("127.0.0.1")
            || url.path() != "/"
            || url.query().is_some()
            || url.fragment().is_some()
            || !url.username().is_empty()
            || url.password().is_some()
        {
            return Err(Failure::Invalid);
        }
        let mut client = Self::new("https://broker.example.com", token, context)?;
        client.origin = origin.trim_end_matches('/').into();
        Ok(client)
    }
    pub fn new(origin: &str, token: Secret, context: Context) -> Result<Self, Failure> {
        context.validate()?;
        let url = url::Url::parse(origin).map_err(|_| Failure::Invalid)?;
        if url.scheme() != "https"
            || url.host_str().is_none()
            || !url.username().is_empty()
            || url.password().is_some()
            || url.query().is_some()
            || url.fragment().is_some()
            || url.path() != "/"
            || token.bytes().len() < 32
        {
            return Err(Failure::Invalid);
        }
        let http = reqwest::Client::builder()
            .no_proxy()
            .redirect(reqwest::redirect::Policy::none())
            .timeout(Duration::from_secs(5))
            .build()
            .map_err(|_| Failure::Unavailable)?;
        Ok(Self {
            http,
            origin: origin.trim_end_matches('/').into(),
            token,
            context,
            root_certificate: None,
            socket: None,
        })
    }
    fn rebuild_http(&mut self) -> Result<(), Failure> {
        let mut builder = reqwest::Client::builder()
            .no_proxy()
            .redirect(reqwest::redirect::Policy::none())
            .timeout(Duration::from_secs(5));
        if let Some(certificate) = &self.root_certificate {
            builder = builder.add_root_certificate(certificate.clone());
        }
        #[cfg(unix)]
        if let Some(socket) = &self.socket {
            builder = builder.unix_socket(socket.clone());
        }
        self.http = builder.build().map_err(|_| Failure::Unavailable)?;
        Ok(())
    }
    /// Keep TLS and hostname checks while reaching the fixed broker through a
    /// host-network relay. Guest network policy never allows management traffic.
    #[cfg(unix)]
    pub fn with_host_socket(mut self, path: std::path::PathBuf) -> Result<Self, Failure> {
        use std::os::unix::fs::{FileTypeExt, MetadataExt};
        let metadata = std::fs::symlink_metadata(&path).map_err(|_| Failure::Unavailable)?;
        if !metadata.file_type().is_socket()
            || metadata.uid() != unsafe { libc::geteuid() }
            || metadata.mode() & 0o077 != 0
        {
            return Err(Failure::Invalid);
        }
        self.socket = Some(path);
        self.rebuild_http()?;
        Ok(self)
    }
    /// Trust one explicitly supplied private broker CA. Host configuration owns
    /// this trust; it is never accepted from a guest or a public API request.
    pub fn with_root_certificate(mut self, pem: &[u8]) -> Result<Self, Failure> {
        if pem.is_empty() || pem.len() > 65536 {
            return Err(Failure::Invalid);
        }
        let certificate = reqwest::Certificate::from_pem(pem).map_err(|_| Failure::Invalid)?;
        self.root_certificate = Some(certificate);
        self.rebuild_http()?;
        Ok(self)
    }
    /// Reads only a host-mounted, private token file. The token is not an env value.
    #[cfg(unix)]
    pub fn from_host_environment(context: Context) -> Result<Option<Self>, Failure> {
        use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
        let origin = match std::env::var("OA_SPACE_CREDENTIAL_BROKER") {
            Ok(s) => s,
            Err(std::env::VarError::NotPresent) => return Ok(None),
            Err(_) => return Err(Failure::Invalid),
        };
        let path =
            std::env::var_os("OA_SPACE_CREDENTIAL_NODE_TOKEN_FILE").ok_or(Failure::Invalid)?;
        // Do not accept a link substituted for the host file.
        let f = std::fs::OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_NOFOLLOW)
            .open(path)
            .map_err(|_| Failure::Unavailable)?;
        let metadata = f.metadata().map_err(|_| Failure::Unavailable)?;
        if !metadata.is_file()
            || metadata.permissions().mode() & 0o077 != 0
            || metadata.len() > 8192
        {
            return Err(Failure::Invalid);
        }
        let mut bytes = Zeroizing::new(Vec::new());
        use std::io::Read as _;
        f.take(8193)
            .read_to_end(&mut bytes)
            .map_err(|_| Failure::Unavailable)?;
        if bytes.len() > 8192 {
            return Err(Failure::Invalid);
        }
        if bytes.last() == Some(&b'\n') {
            bytes.pop();
        }
        let socket =
            std::env::var_os("OA_SPACE_CREDENTIAL_BROKER_SOCKET").ok_or(Failure::Invalid)?;
        Ok(Some(
            Self::new(&origin, Secret::new(std::mem::take(&mut *bytes))?, context)?
                .with_host_socket(socket.into())?,
        ))
    }
    fn context(&self, policy: &str) -> Context {
        let mut c = self.context.clone();
        c.native_policy_ref = policy.into();
        c
    }
    async fn send<T: Serialize + ?Sized>(
        &self,
        path: &str,
        body: &T,
    ) -> Result<reqwest::Response, Failure> {
        let token = std::str::from_utf8(self.token.bytes()).map_err(|_| Failure::Invalid)?;
        self.http
            .post(format!("{}{path}", self.origin))
            .bearer_auth(token)
            .timeout(if path == "/v1/check" {
                Duration::from_secs(5)
            } else {
                Duration::from_secs(30)
            })
            .json(body)
            .send()
            .await
            .map_err(|_| Failure::Unavailable)
    }
    pub async fn check(&self, policy: &str) -> Result<(), Failure> {
        self.check_scope(policy, None).await.map(|_| ())
    }
    pub async fn check_scope(&self, policy: &str, scope: Option<&str>) -> Result<String, Failure> {
        let mut response = self
            .send(
                "/v1/check",
                &CheckRequest {
                    context: self.context(policy),
                    scope: scope.map(str::to_owned),
                },
            )
            .await?;
        if response.status() != 200 {
            return Err(Failure::Denied);
        }
        let mut bytes = Vec::new();
        while let Some(chunk) = response.chunk().await.map_err(|_| Failure::Unavailable)? {
            if bytes.len().saturating_add(chunk.len()) > 1024 {
                return Err(Failure::Unavailable);
            }
            bytes.extend_from_slice(&chunk);
        }
        let answer: serde_json::Value =
            serde_json::from_slice(&bytes).map_err(|_| Failure::Unavailable)?;
        answer["scope"]
            .as_str()
            .filter(|s| crate::digest(s))
            .map(str::to_owned)
            .ok_or(Failure::Unavailable)
    }
    pub async fn headers(
        &self,
        policy: &str,
        host: &str,
        port: u16,
        path: &str,
        method: &str,
        selectors: &BTreeMap<String, Uuid>,
    ) -> Result<Vec<Header>, Failure> {
        self.headers_scoped(policy, host, port, path, method, selectors, None)
            .await
    }
    pub async fn headers_scoped(
        &self,
        policy: &str,
        host: &str,
        port: u16,
        path: &str,
        method: &str,
        selectors: &BTreeMap<String, Uuid>,
        scope: Option<&str>,
    ) -> Result<Vec<Header>, Failure> {
        let request = Request {
            context: self.context(policy),
            host: host.into(),
            port,
            path: path.into(),
            method: method.into(),
            scope: scope.map(str::to_owned),
            selectors: selectors.clone(),
        };
        let mut response = self.send("/v1/resolve", &request).await?;
        if response.status() != 200 {
            return Err(Failure::Denied);
        }
        let mut bytes = Zeroizing::new(Vec::new());
        while let Some(chunk) = response.chunk().await.map_err(|_| Failure::Unavailable)? {
            if bytes.len().saturating_add(chunk.len()) > 300000 {
                return Err(Failure::Unavailable);
            }
            bytes.extend_from_slice(&chunk);
        }
        let headers: Vec<Header> =
            serde_json::from_slice(&bytes).map_err(|_| Failure::Unavailable)?;
        if headers.len() > 16 {
            return Err(Failure::Unavailable);
        }
        for h in &headers {
            if !matches!(
                h.name.as_str(),
                "authorization" | "x-api-key" | "x-goog-api-key"
            ) || h.value.is_empty()
                || h.value.len() > crate::MAX_SECRET_BYTES
                || !h.value.bytes().all(|b| (b' '..=b'~').contains(&b))
            {
                return Err(Failure::Unavailable);
            }
        }
        Ok(headers)
    }
}

/// Extract only opaque markers; raw client credentials are never sent to the
/// broker. Git carries its marker as the password in HTTP Basic authentication.
pub fn selectors(headers: &http::HeaderMap) -> Result<BTreeMap<String, Uuid>, Failure> {
    use base64::Engine as _;
    let mut result = BTreeMap::new();
    for name in ["authorization", "x-api-key", "x-goog-api-key"] {
        let values = headers.get_all(name);
        if values.iter().count() > 1 {
            return Err(Failure::Denied);
        }
        let Some(value) = values.iter().next() else {
            continue;
        };
        let value = value.to_str().map_err(|_| Failure::Denied)?;
        if value.len() > 4096 {
            return Err(Failure::Denied);
        }
        let mut decoded = Zeroizing::new(Vec::new());
        let marker = if name != "authorization" {
            value
        } else {
            let Some((scheme, content)) = value.split_once(' ') else {
                continue;
            };
            if scheme.eq_ignore_ascii_case("bearer") || scheme.eq_ignore_ascii_case("token") {
                content
            } else if scheme.eq_ignore_ascii_case("basic") {
                *decoded = base64::engine::general_purpose::STANDARD
                    .decode(content)
                    .map_err(|_| Failure::Denied)?;
                let text = std::str::from_utf8(&decoded).map_err(|_| Failure::Denied)?;
                let Some((_, password)) = text.split_once(':') else {
                    continue;
                };
                password
            } else {
                continue;
            }
        };
        if let Some(id) = marker.strip_prefix("oa-ref:") {
            let id = Uuid::parse_str(id).map_err(|_| Failure::Denied)?;
            if id.is_nil() {
                return Err(Failure::Denied);
            }
            result.insert(name.into(), id);
        }
    }
    Ok(result)
}
