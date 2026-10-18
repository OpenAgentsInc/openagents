//! A single disk: control-plane operations plus the S3-compatible object API.

use std::collections::HashSet;
use std::ops::Deref;
use std::sync::Arc;

use bytes::Bytes;
use futures_util::StreamExt;
use futures_util::stream;
use reqwest::Method;
use serde::{Deserialize, Serialize};

use crate::error::{Error, Result, S3Error};
use crate::http::{Http, Req, S3Req, S3Resp, enc};
use crate::retry::RetryMode;
use crate::s3::*;
use crate::types::*;

/// Options for [`Disk::create_branch`].
#[derive(Debug, Clone, Default)]
pub struct CreateBranchOptions {
    /// 1-100 letters, digits, hyphens or underscores; unique across the disk.
    pub name: String,
    /// Name of a committed checkpoint on the source.
    pub from_checkpoint: String,
    /// Branch the checkpoint was taken on; omit to branch from the root disk.
    pub from_branch: Option<String>,
    /// UUID identifying this create. Pass the same key when you retry a create
    /// yourself so a retry of one that succeeded returns that branch instead
    /// of a 409. Defaults to a fresh key per call; the SDK's own retries
    /// always reuse it.
    pub idempotency_key: Option<String>,
}

/// Options for [`Disk::grep`].
#[derive(Debug, Clone)]
pub struct GrepOptions {
    /// Directory to search, relative to the disk root (`""` or `"/"` = root).
    pub directory: String,
    /// Extended regular expression (`grep -E`).
    pub pattern: String,
    /// Walk subdirectories breadth-first. Default false.
    pub recursive: bool,
    /// Wall-clock deadline in seconds. Default 30.
    pub max_duration_seconds: u64,
    /// Maximum parallel grep workers. Default 50.
    pub concurrency: u64,
    /// Stop after this many matches. Default 1000.
    pub max_results: u64,
}

impl GrepOptions {
    pub fn new(directory: impl Into<String>, pattern: impl Into<String>) -> Self {
        Self {
            directory: directory.into(),
            pattern: pattern.into(),
            recursive: false,
            max_duration_seconds: 30,
            concurrency: 50,
            max_results: 1000,
        }
    }
    pub fn recursive(mut self, v: bool) -> Self {
        self.recursive = v;
        self
    }
    pub fn max_duration_seconds(mut self, v: u64) -> Self {
        self.max_duration_seconds = v;
        self
    }
    pub fn concurrency(mut self, v: u64) -> Self {
        self.concurrency = v;
        self
    }
    pub fn max_results(mut self, v: u64) -> Self {
        self.max_results = v;
        self
    }
}

/// Options for [`Disk::share`].
#[derive(Debug, Clone, Default)]
pub struct ShareUrlOptions {
    /// Lifetime in seconds, up to 604800 (7 days). Default 86400.
    pub expires_in: Option<u64>,
}

/// A signed, time-limited download URL.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ShareUrlResult {
    pub url: String,
    pub expires_in: u64,
}

/// Which kind of disk user to remove.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UserType {
    Token,
    AwsSts,
}

impl UserType {
    fn as_str(self) -> &'static str {
        match self {
            Self::Token => "token",
            Self::AwsSts => "awssts",
        }
    }
}

/// A disk. Derefs to its [`DiskResponse`] fields (`disk.id`, `disk.name`, ...).
#[derive(Debug, Clone)]
pub struct Disk {
    data: DiskResponse,
    http: Arc<Http>,
}

impl Deref for Disk {
    type Target = DiskResponse;
    fn deref(&self) -> &DiskResponse {
        &self.data
    }
}

impl Disk {
    pub(crate) fn new(data: DiskResponse, http: Arc<Http>) -> Self {
        Self { data, http }
    }

    /// The disk's wire representation.
    pub fn to_response(&self) -> DiskResponse {
        self.data.clone()
    }

    fn path(&self, suffix: &str) -> String {
        format!("/api/disks/{}{suffix}", enc(&self.data.id))
    }

    // ------------------------------------------------------------ users

    /// Authorize a user to mount this disk.
    pub async fn add_user(&self, user: DiskUser) -> Result<AuthorizedUser> {
        let req = Req::new(Method::POST, self.path("/users"), RetryMode::Connect).json(user);
        self.http.call(req).await
    }

    /// Remove an authorized user by the identifier from the creation or list response.
    pub async fn remove_user(&self, user_type: UserType, identifier: &str) -> Result<()> {
        let req = Req::new(
            Method::DELETE,
            self.path(&format!("/users/{}", user_type.as_str())),
            RetryMode::Connect,
        )
        .query("identifier", identifier);
        self.http.call_empty(req).await
    }

    /// Create a new mount token for this disk. The secret is returned once.
    pub async fn create_token(&self, nickname: &str) -> Result<AuthorizedUser> {
        let user = self
            .add_user(DiskUser::Token(TokenUser {
                nickname: nickname.to_string(),
                ..Default::default()
            }))
            .await?;
        if user.token.is_none() || user.identifier.is_none() {
            return Err(Error::Decode(
                "Server did not return a generated token".into(),
            ));
        }
        Ok(user)
    }

    /// Remove a token user by identifier.
    pub async fn remove_token_user(&self, identifier: &str) -> Result<()> {
        self.remove_user(UserType::Token, identifier).await
    }

    // ------------------------------------------------------------ delegations

    /// List delegations currently held on this disk.
    pub async fn list_delegations(&self) -> Result<Vec<Delegation>> {
        #[derive(Deserialize)]
        struct Data {
            #[serde(default)]
            delegations: Vec<Delegation>,
        }
        let data: Data = self.http.call(Req::get(self.path("/delegations"))).await?;
        Ok(data.delegations)
    }

    /// Forcibly revoke the delegation identified by `(client_id, inode_id)`.
    pub async fn revoke_delegation(&self, delegation: &Delegation) -> Result<()> {
        let req = Req::new(
            Method::POST,
            self.path("/revoke-delegation"),
            RetryMode::Connect,
        )
        .json(serde_json::json!({
            "clientId": delegation.client_id,
            "inodeId": delegation.inode_id,
        }));
        self.http.call_empty(req).await
    }

    // ------------------------------------------------------------ branches

    /// Create a writable branch from a committed checkpoint.
    ///
    /// Fails with 409 if the name exists or the checkpoint is not yet
    /// committed, and 404 if the checkpoint or `from_branch` does not exist.
    pub async fn create_branch(&self, options: CreateBranchOptions) -> Result<Branch> {
        // One key for every attempt, so a retry after a timeout returns the
        // branch the first attempt created instead of a 409.
        let key = options
            .idempotency_key
            .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
        let mut body = serde_json::json!({
            "branch_name": options.name,
            "from_checkpoint_name": options.from_checkpoint,
        });
        if let Some(b) = options.from_branch {
            body["from_branch"] = b.into();
        }
        let req = Req::new(Method::POST, self.path("/branches"), RetryMode::Transient)
            .header("Idempotency-Key", key)
            .json(body);
        self.http.call(req).await
    }

    /// List every branch of this disk, including branches of branches.
    pub async fn list_branches(&self) -> Result<Vec<Branch>> {
        let branches: Option<Vec<Branch>> =
            self.http.call(Req::get(self.path("/branches"))).await?;
        Ok(branches.unwrap_or_default())
    }

    pub async fn get_branch(&self, name: &str) -> Result<Branch> {
        self.http
            .call(Req::get(self.path(&format!("/branches/{}", enc(name)))))
            .await
    }

    // ------------------------------------------------------------ allowed IPs

    pub async fn get_allowed_ips(&self) -> Result<Vec<String>> {
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct Data {
            #[serde(default)]
            allowed_ips: Vec<String>,
        }
        let d: Data = self.http.call(Req::get(self.path("/allowed-ips"))).await?;
        Ok(d.allowed_ips)
    }

    /// Replace the IP allowlist; an empty list removes all restrictions.
    pub async fn set_allowed_ips(&self, allowed_ips: &[String]) -> Result<Vec<String>> {
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct Data {
            #[serde(default)]
            allowed_ips: Vec<String>,
        }
        let req = Req::new(Method::PUT, self.path("/allowed-ips"), RetryMode::Transient)
            .json(serde_json::json!({ "allowedIps": allowed_ips }));
        let d: Data = self.http.call(req).await?;
        Ok(d.allowed_ips)
    }

    pub async fn add_allowed_ip(&self, ip: &str) -> Result<Vec<String>> {
        let mut current = self.get_allowed_ips().await?;
        if current.iter().any(|i| i == ip) {
            return Ok(current);
        }
        current.push(ip.to_string());
        self.set_allowed_ips(&current).await
    }

    pub async fn remove_allowed_ip(&self, ip: &str) -> Result<Vec<String>> {
        let current: Vec<String> = self
            .get_allowed_ips()
            .await?
            .into_iter()
            .filter(|i| i != ip)
            .collect();
        self.set_allowed_ips(&current).await
    }

    // ------------------------------------------------------------ lifecycle

    /// Delete the disk and all associated resources.
    pub async fn delete(&self) -> Result<()> {
        let req = Req::new(Method::DELETE, self.path(""), RetryMode::Connect);
        self.http.call_empty(req).await
    }

    /// Run a command in a container with this disk mounted; blocks until it completes.
    pub async fn exec(&self, command: &str) -> Result<ExecResult> {
        exec_disk(&self.http, &self.data.id, command).await
    }

    /// Constant-time parallel grep across files on this disk.
    ///
    /// `stopped_reason` says whether the search completed or short-circuited;
    /// when stopping early the matches are a sample, not the first N.
    pub async fn grep(&self, opts: GrepOptions) -> Result<GrepResult> {
        let body = GrepDiskRequest {
            directory: opts.directory,
            pattern: opts.pattern,
            recursive: opts.recursive,
            max_duration_seconds: opts.max_duration_seconds,
            concurrency: opts.concurrency,
            max_results: opts.max_results,
        };
        self.http
            .call(Req::new(Method::POST, self.path("/grep"), RetryMode::Transient).json(body))
            .await
    }

    /// Create a signed, time-limited URL anyone can use to download one file.
    pub async fn share(&self, key: &str, opts: ShareUrlOptions) -> Result<ShareUrlResult> {
        let mut body = serde_json::json!({ "key": key });
        if let Some(e) = opts.expires_in {
            body["expiresIn"] = e.into();
        }
        self.http
            .call(Req::new(Method::POST, self.path("/share"), RetryMode::Connect).json(body))
            .await
    }

    // ------------------------------------------------------------ objects

    fn s3req(&self, method: Method, key: &str) -> S3Req {
        S3Req::new(method, self.data.id.clone(), key)
    }

    /// Read an object's full contents. Fails with an [`S3Error`] (404,
    /// `NoSuchKey`) if absent.
    pub async fn get_object(&self, key: &str) -> Result<Bytes> {
        let resp = self.http.s3(self.s3req(Method::GET, key)).await?;
        check(&resp, "GetObject")?;
        Ok(resp.body)
    }

    /// Fetch metadata without downloading; `None` if the object does not exist.
    pub async fn head_object(&self, key: &str) -> Result<Option<ObjectMetadata>> {
        let resp = self.http.s3(self.s3req(Method::HEAD, key)).await?;
        if resp.status == 404 {
            return Ok(None);
        }
        check(&resp, "HeadObject")?;
        let header = |name: &str| {
            resp.headers
                .get(name)
                .and_then(|v| v.to_str().ok())
                .map(str::to_string)
        };
        Ok(Some(ObjectMetadata {
            size: header("content-length")
                .and_then(|v| v.parse().ok())
                .unwrap_or(0),
            etag: header("etag"),
            content_type: header("content-type"),
            last_modified: header("last-modified").and_then(|v| {
                chrono::DateTime::parse_from_rfc2822(&v)
                    .ok()
                    .map(|d| d.with_timezone(&chrono::Utc))
            }),
        }))
    }

    /// Whether an object exists (a HEAD that maps 404 to `false`).
    pub async fn object_exists(&self, key: &str) -> Result<bool> {
        Ok(self.head_object(key).await?.is_some())
    }

    /// Write an object with default options.
    pub async fn put_object(&self, key: &str, body: impl Into<Bytes>) -> Result<PutObjectResult> {
        self.put_object_with(key, body, PutObjectOptions::default())
            .await
    }

    /// Write an object of any size via the S3-compatible API.
    ///
    /// Bodies at or below the multipart threshold (default: the part size,
    /// 16 MiB) go through a single PutObject; larger bodies are split into
    /// parts, uploaded with bounded concurrency (default 4) and assembled. A
    /// failed part aborts the upload so nothing is left half-staged.
    ///
    /// `attrs` set the POSIX mode/owner of the new file. A key ending in `/`
    /// creates a directory marker.
    pub async fn put_object_with(
        &self,
        key: &str,
        body: impl Into<Bytes>,
        options: impl Into<PutObjectOptions>,
    ) -> Result<PutObjectResult> {
        let opts: PutObjectOptions = options.into();
        let body: Bytes = body.into();
        let content_type = opts
            .content_type
            .clone()
            .unwrap_or_else(|| "application/octet-stream".into());
        let part_size = opts
            .part_size
            .unwrap_or(DEFAULT_PART_SIZE)
            .max(MIN_PART_SIZE);
        let threshold = opts.multipart_threshold.unwrap_or(part_size);

        if (body.len() as u64) <= threshold {
            let req = self
                .s3req(Method::PUT, key)
                .body(body)
                .content_type(Some(content_type))
                .headers(opts.attrs.headers());
            let resp = self.http.s3(req).await?;
            check(&resp, "PutObject")?;
            return Ok(PutObjectResult {
                etag: header_str(&resp, "etag"),
            });
        }
        self.put_multipart(
            key,
            body,
            &content_type,
            part_size,
            opts.concurrency
                .unwrap_or(DEFAULT_UPLOAD_CONCURRENCY)
                .max(1),
            opts.attrs,
        )
        .await
    }

    async fn put_multipart(
        &self,
        key: &str,
        bytes: Bytes,
        content_type: &str,
        part_size: u64,
        concurrency: usize,
        attrs: PosixCreateAttrs,
    ) -> Result<PutObjectResult> {
        // Grow the part size if the body would need more than the server's
        // 10,000-part cap.
        let part_size = effective_upload_part_size(bytes.len() as u64, part_size) as usize;
        let mp = self.multipart();
        let upload = mp.create_with(key, Some(content_type), attrs).await?;
        let result: Result<CompletedMultipartUpload> = async {
            let count = bytes.len().div_ceil(part_size);
            let mut parts: Vec<Option<UploadPart>> = vec![None; count];
            let mut uploads = stream::iter((0..count).map(|index| {
                let start = index * part_size;
                let slice = bytes.slice(start..(start + part_size).min(bytes.len()));
                let mp = &mp;
                let upload_id = upload.upload_id.as_str();
                async move {
                    let part = mp
                        .upload_part(key, upload_id, index as u32 + 1, slice)
                        .await?;
                    Ok::<_, Error>((index, part))
                }
            }))
            .buffer_unordered(concurrency.min(count));
            while let Some(done) = uploads.next().await {
                let (index, part) = done?;
                parts[index] = Some(part);
            }
            drop(uploads);
            let parts: Vec<UploadPart> = parts.into_iter().flatten().collect();
            mp.complete(key, &upload.upload_id, &parts).await
        }
        .await;
        match result {
            Ok(done) => Ok(PutObjectResult { etag: done.etag }),
            Err(err) => {
                // Don't let a cleanup failure mask the original error.
                let _ = mp.abort(key, &upload.upload_id).await;
                Err(err)
            }
        }
    }

    /// Append up to 1 MiB to an object (creating it if absent) via the
    /// `?append=true` PutObject extension.
    ///
    /// Unlike most operations this is never auto-retried: append is not
    /// idempotent, so replaying a succeeded-but-unacknowledged append would
    /// duplicate the bytes.
    pub async fn append_object(
        &self,
        key: &str,
        body: impl Into<Bytes>,
    ) -> Result<PutObjectResult> {
        self.append_object_with(key, body, AppendObjectOptions::default())
            .await
    }

    pub async fn append_object_with(
        &self,
        key: &str,
        body: impl Into<Bytes>,
        options: impl Into<AppendObjectOptions>,
    ) -> Result<PutObjectResult> {
        let opts: AppendObjectOptions = options.into();
        let req = self
            .s3req(Method::PUT, key)
            .body(body)
            .content_type(Some(
                opts.content_type
                    .unwrap_or_else(|| "application/octet-stream".into()),
            ))
            .query("append", "true")
            .no_retry()
            .headers(opts.attrs.headers());
        let resp = self.http.s3(req).await?;
        check(&resp, "AppendObject")?;
        Ok(PutObjectResult {
            etag: header_str(&resp, "etag"),
        })
    }

    /// Delete an object. Idempotent: a missing key is not an error.
    pub async fn delete_object(&self, key: &str) -> Result<()> {
        let resp = self.http.s3(self.s3req(Method::DELETE, key)).await?;
        if !resp.ok && resp.status != 404 {
            return Err(s3_error("DeleteObject", &resp).into());
        }
        Ok(())
    }

    /// List objects (ListObjectsV2). By default follows continuation tokens
    /// until exhausted; use `limit` to cap the total, `single_page` for one
    /// request, or [`list_objects_pages`](Self::list_objects_pages) to stream.
    pub async fn list_objects(
        &self,
        prefix: Option<&str>,
        opts: ListObjectsOptions,
    ) -> Result<ListObjectsResult> {
        if opts.single_page {
            return self.list_objects_page(prefix, &opts).await;
        }
        let mut objects = Vec::new();
        let mut common_prefixes = Vec::new();
        let mut seen = HashSet::new();
        let mut echoed_prefix = None;
        let mut truncated = false;
        let mut pages = self.list_objects_pages(prefix, opts.clone());
        'outer: while let Some(page) = pages.next_page().await? {
            echoed_prefix = page.prefix.clone();
            for cp in page.common_prefixes {
                if seen.insert(cp.clone()) {
                    common_prefixes.push(cp);
                }
            }
            for obj in page.objects {
                if opts.limit.is_some_and(|l| objects.len() >= l) {
                    truncated = true; // the cap cut the listing short
                    break 'outer;
                }
                objects.push(obj);
            }
        }
        Ok(ListObjectsResult {
            key_count: objects.len(),
            objects,
            common_prefixes,
            is_truncated: truncated,
            next_continuation_token: None,
            prefix: echoed_prefix,
        })
    }

    /// Lazily walk ListObjectsV2 pages, following continuation tokens.
    pub fn list_objects_pages(
        &self,
        prefix: Option<&str>,
        opts: ListObjectsOptions,
    ) -> ListObjectsPages {
        ListObjectsPages {
            disk: self.clone(),
            prefix: prefix.map(str::to_string),
            token: opts.continuation_token.clone(),
            opts,
            seen: HashSet::new(),
            done: false,
        }
    }

    async fn list_objects_page(
        &self,
        prefix: Option<&str>,
        opts: &ListObjectsOptions,
    ) -> Result<ListObjectsResult> {
        let mut req = self.s3req(Method::GET, "").query("list-type", 2);
        if let Some(p) = prefix {
            req = req.query("prefix", p);
        }
        // Non-recursive lists a single level via the "/" delimiter.
        if !opts.recursive {
            req = req.query("delimiter", "/");
        }
        if let Some(t) = &opts.continuation_token {
            req = req.query("continuation-token", t);
        }
        if let Some(s) = &opts.start_after {
            req = req.query("start-after", s);
        }
        let resp = self.http.s3(req).await?;
        check(&resp, "ListObjectsV2")?;
        Ok(parse_list_objects_result(&text(&resp.body)))
    }

    /// Delete many objects. Failures are reported per key rather than
    /// returned as errors. Inputs over 1000 keys are split into batches.
    pub async fn delete_objects(
        &self,
        keys: &[String],
        opts: DeleteObjectsOptions,
    ) -> Result<DeleteObjectsResult> {
        let mut out = DeleteObjectsResult::default();
        for batch in keys.chunks(MAX_DELETE_OBJECTS_PER_REQUEST) {
            let req = self
                .s3req(Method::POST, "")
                .query("delete", "")
                .body(build_delete_objects_xml(batch, opts.quiet))
                .content_type(Some("application/xml"));
            let resp = self.http.s3(req).await?;
            check(&resp, "DeleteObjects")?;
            let parsed = parse_delete_objects_result(&text(&resp.body));
            out.deleted.extend(parsed.deleted);
            out.errors.extend(parsed.errors);
        }
        Ok(out)
    }

    /// The manual multipart-upload API. Most callers want
    /// [`put_object`](Self::put_object), which runs the lifecycle for them.
    pub fn multipart(&self) -> DiskMultipart {
        DiskMultipart { disk: self.clone() }
    }
}

/// Lazy ListObjectsV2 pager returned by [`Disk::list_objects_pages`].
#[derive(Debug)]
pub struct ListObjectsPages {
    disk: Disk,
    prefix: Option<String>,
    token: Option<String>,
    opts: ListObjectsOptions,
    seen: HashSet<String>,
    done: bool,
}

impl ListObjectsPages {
    /// The next page, or `None` once the listing is exhausted. Stops if the
    /// server repeats a token (no forward progress).
    pub async fn next_page(&mut self) -> Result<Option<ListObjectsResult>> {
        if self.done {
            return Ok(None);
        }
        let mut opts = self.opts.clone();
        opts.continuation_token = self.token.clone();
        let page = self
            .disk
            .list_objects_page(self.prefix.as_deref(), &opts)
            .await?;
        let next = if page.is_truncated {
            page.next_continuation_token.clone()
        } else {
            None
        };
        match next {
            Some(t) if self.seen.insert(t.clone()) => self.token = Some(t),
            _ => self.done = true,
        }
        Ok(Some(page))
    }
}

/// Manual multipart-upload lifecycle, from [`Disk::multipart`].
#[derive(Debug, Clone)]
pub struct DiskMultipart {
    disk: Disk,
}

impl DiskMultipart {
    /// Start a multipart upload.
    pub async fn create(&self, key: &str) -> Result<MultipartUpload> {
        self.create_with(key, None, PosixCreateAttrs::default())
            .await
    }

    /// Start a multipart upload with a content type and POSIX attributes.
    pub async fn create_with(
        &self,
        key: &str,
        content_type: Option<&str>,
        attrs: PosixCreateAttrs,
    ) -> Result<MultipartUpload> {
        let req = self
            .disk
            .s3req(Method::POST, key)
            .query("uploads", "")
            .content_type(content_type)
            .headers(attrs.headers());
        let resp = self.disk.http.s3(req).await?;
        check(&resp, "CreateMultipartUpload")?;
        let body = text(&resp.body);
        let root = parse_xml(&body);
        let root = if root.name == "InitiateMultipartUploadResult" {
            root
        } else {
            Node::default()
        };
        let Some(upload_id) = root.text_of("UploadId").filter(|s| !s.is_empty()) else {
            return Err(build_s3_error(
                "CreateMultipartUpload",
                resp.status,
                "",
                None,
                Some("response did not contain an UploadId".into()),
                None,
                body,
            )
            .into());
        };
        Ok(MultipartUpload {
            upload_id,
            key: root.text_of("Key").unwrap_or_else(|| key.to_string()),
            bucket: root
                .text_of("Bucket")
                .unwrap_or_else(|| self.disk.data.id.clone()),
        })
    }

    /// Upload one part and return its etag. Every part but the last must be at least 5 MiB.
    pub async fn upload_part(
        &self,
        key: &str,
        upload_id: &str,
        part_number: u32,
        body: impl Into<Bytes>,
    ) -> Result<UploadPart> {
        let req = self
            .disk
            .s3req(Method::PUT, key)
            .query("uploadId", upload_id)
            .query("partNumber", part_number)
            .body(body);
        let resp = self.disk.http.s3(req).await?;
        check(&resp, "UploadPart")?;
        Ok(UploadPart {
            part_number,
            etag: header_str(&resp, "etag").unwrap_or_default(),
        })
    }

    /// Assemble the parts (sorted by part number) into one object.
    ///
    /// Never auto-retried: a retry after a succeeded-but-unacknowledged
    /// completion would return a spurious `NoSuchUpload`.
    pub async fn complete(
        &self,
        key: &str,
        upload_id: &str,
        parts: &[UploadPart],
    ) -> Result<CompletedMultipartUpload> {
        let mut sorted = parts.to_vec();
        sorted.sort_by_key(|p| p.part_number);
        let req = self
            .disk
            .s3req(Method::POST, key)
            .query("uploadId", upload_id)
            .body(build_complete_multipart_upload_xml(&sorted))
            .content_type(Some("application/xml"))
            .no_retry();
        let resp = self.disk.http.s3(req).await?;
        check(&resp, "CompleteMultipartUpload")?;
        let root = parse_xml(&text(&resp.body));
        let root = if root.name == "CompleteMultipartUploadResult" {
            root
        } else {
            Node::default()
        };
        Ok(CompletedMultipartUpload {
            etag: root.text_of("ETag"),
            location: root.text_of("Location"),
            bucket: root.text_of("Bucket"),
            key: root.text_of("Key"),
        })
    }

    /// Discard every staged part. A missing upload (404) is not an error.
    pub async fn abort(&self, key: &str, upload_id: &str) -> Result<()> {
        let resp = self
            .disk
            .http
            .s3(self
                .disk
                .s3req(Method::DELETE, key)
                .query("uploadId", upload_id))
            .await?;
        if !resp.ok && resp.status != 404 {
            return Err(s3_error("AbortMultipartUpload", &resp).into());
        }
        Ok(())
    }

    /// One page of the parts uploaded so far.
    pub async fn list_parts(
        &self,
        key: &str,
        upload_id: &str,
        opts: ListPartsOptions,
    ) -> Result<PartListing> {
        let mut req = self
            .disk
            .s3req(Method::GET, key)
            .query("uploadId", upload_id);
        if let Some(m) = opts.max_parts {
            req = req.query("max-parts", m);
        }
        if let Some(m) = opts.part_number_marker {
            req = req.query("part-number-marker", m);
        }
        let resp = self.disk.http.s3(req).await?;
        check(&resp, "ListParts")?;
        Ok(parse_list_parts_result(&text(&resp.body)))
    }

    /// One page of in-progress uploads on the disk.
    pub async fn list_uploads(
        &self,
        opts: ListMultipartUploadsOptions,
    ) -> Result<MultipartUploadListing> {
        let mut req = self.disk.s3req(Method::GET, "").query("uploads", "");
        if let Some(v) = &opts.prefix {
            req = req.query("prefix", v);
        }
        if let Some(v) = &opts.delimiter {
            req = req.query("delimiter", v);
        }
        if let Some(v) = &opts.key_marker {
            req = req.query("key-marker", v);
        }
        if let Some(v) = &opts.upload_id_marker {
            req = req.query("upload-id-marker", v);
        }
        if let Some(v) = opts.max_uploads {
            req = req.query("max-uploads", v);
        }
        let resp = self.disk.http.s3(req).await?;
        check(&resp, "ListMultipartUploads")?;
        Ok(parse_list_multipart_uploads_result(&text(&resp.body)))
    }
}

pub(crate) async fn exec_disk(http: &Http, id: &str, command: &str) -> Result<ExecResult> {
    let req = Req::new(
        Method::POST,
        format!("/api/disks/{}/exec", enc(id)),
        RetryMode::Connect,
    )
    .json(serde_json::json!({ "command": command }));
    http.call(req).await
}

fn text(body: &Bytes) -> String {
    String::from_utf8_lossy(body).into_owned()
}

fn header_str(resp: &S3Resp, name: &str) -> Option<String> {
    resp.headers
        .get(name)
        .and_then(|v| v.to_str().ok())
        .map(str::to_string)
}

fn s3_error(operation: &str, resp: &S3Resp) -> S3Error {
    crate::s3::parse_s3_error(operation, resp.status, &resp.status_text, &text(&resp.body))
}

fn check(resp: &S3Resp, operation: &str) -> Result<()> {
    if resp.ok {
        Ok(())
    } else {
        Err(s3_error(operation, resp).into())
    }
}
