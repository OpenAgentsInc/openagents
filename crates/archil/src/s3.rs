//! S3-compatible object API: result types, XML parsing and request bodies.

use chrono::{DateTime, Utc};
use quick_xml::Reader;
use quick_xml::events::Event;

use crate::error::S3Error;

// ---------------------------------------------------------------- types

/// One object in a listing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct S3Object {
    /// Object key (path on the disk).
    pub key: String,
    /// Size in bytes.
    pub size: u64,
    pub etag: Option<String>,
    pub last_modified: Option<DateTime<Utc>>,
}

/// Result of a put or append.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PutObjectResult {
    /// Entity tag the server assigned (quoted, per S3).
    pub etag: Option<String>,
}

/// Object metadata from a HEAD request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObjectMetadata {
    pub size: u64,
    pub etag: Option<String>,
    pub content_type: Option<String>,
    pub last_modified: Option<DateTime<Utc>>,
}

/// Options for [`Disk::list_objects`](crate::Disk::list_objects).
#[derive(Debug, Clone, Default)]
pub struct ListObjectsOptions {
    /// List the whole subtree flat. When false (the default) only the
    /// immediate level is returned, split into objects and common prefixes.
    pub recursive: bool,
    /// Return the first page only and let the caller page manually.
    pub single_page: bool,
    /// Stop after this many objects (when auto-paginating).
    pub limit: Option<usize>,
    /// Start from a prior `next_continuation_token`.
    pub continuation_token: Option<String>,
    /// Return keys lexicographically after this one.
    pub start_after: Option<String>,
}

/// A listing (or one page of it).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ListObjectsResult {
    pub objects: Vec<S3Object>,
    /// Directory-like prefixes rolled up by the delimiter.
    pub common_prefixes: Vec<String>,
    pub is_truncated: bool,
    pub next_continuation_token: Option<String>,
    pub key_count: usize,
    pub prefix: Option<String>,
}

/// A part to list in [`DiskMultipart::complete`](crate::DiskMultipart::complete).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UploadPart {
    /// 1-based part number (1..=10000).
    pub part_number: u32,
    pub etag: String,
}

/// Handle to an in-progress multipart upload.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MultipartUpload {
    pub upload_id: String,
    pub key: String,
    /// The bucket (disk id).
    pub bucket: String,
}

/// The assembled object after completing a multipart upload.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CompletedMultipartUpload {
    pub etag: Option<String>,
    pub location: Option<String>,
    pub bucket: Option<String>,
    pub key: Option<String>,
}

/// One part in a parts listing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PartInfo {
    pub part_number: u32,
    pub etag: Option<String>,
    pub size: u64,
    pub last_modified: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Default)]
pub struct ListPartsOptions {
    /// Cap parts returned in one page (server clamps to 1000).
    pub max_parts: Option<u32>,
    pub part_number_marker: Option<u32>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PartListing {
    pub bucket: Option<String>,
    pub key: Option<String>,
    pub upload_id: Option<String>,
    pub parts: Vec<PartInfo>,
    pub is_truncated: bool,
    pub part_number_marker: u32,
    pub next_part_number_marker: Option<u32>,
    pub max_parts: u32,
}

/// One in-progress upload in a listing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MultipartUploadSummary {
    pub key: String,
    pub upload_id: String,
    pub initiated: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Default)]
pub struct ListMultipartUploadsOptions {
    pub prefix: Option<String>,
    pub delimiter: Option<String>,
    pub key_marker: Option<String>,
    pub upload_id_marker: Option<String>,
    pub max_uploads: Option<u32>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MultipartUploadListing {
    pub bucket: Option<String>,
    pub uploads: Vec<MultipartUploadSummary>,
    pub common_prefixes: Vec<String>,
    pub is_truncated: bool,
    pub key_marker: Option<String>,
    pub upload_id_marker: Option<String>,
    pub next_key_marker: Option<String>,
    pub next_upload_id_marker: Option<String>,
    pub prefix: Option<String>,
    pub delimiter: Option<String>,
    pub max_uploads: Option<u32>,
}

#[derive(Debug, Clone, Default)]
pub struct DeleteObjectsOptions {
    /// Omit the per-key success list from the response.
    pub quiet: bool,
}

/// A per-key failure within a batch delete.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeleteObjectsError {
    pub key: String,
    pub code: Option<String>,
    pub message: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DeleteObjectsResult {
    /// Keys confirmed deleted (empty in quiet mode).
    pub deleted: Vec<String>,
    pub errors: Vec<DeleteObjectsError>,
}

/// POSIX attributes for newly created objects, sent as `x-archil-*` headers.
/// Missing parent directories inherit `uid`/`gid` with mode `0755`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct PosixCreateAttrs {
    /// Mode bits (`0o644`), sent in octal.
    pub mode: Option<u32>,
    pub uid: Option<u32>,
    pub gid: Option<u32>,
}

impl PosixCreateAttrs {
    pub(crate) fn headers(&self) -> Vec<(String, String)> {
        let mut h = Vec::new();
        if let Some(m) = self.mode {
            h.push(("x-archil-mode".to_string(), format!("{m:o}")));
        }
        if let Some(u) = self.uid {
            h.push(("x-archil-uid".to_string(), u.to_string()));
        }
        if let Some(g) = self.gid {
            h.push(("x-archil-gid".to_string(), g.to_string()));
        }
        h
    }
}

/// Options for [`Disk::put_object`](crate::Disk::put_object).
#[derive(Debug, Clone, Default)]
pub struct PutObjectOptions {
    /// Defaults to `application/octet-stream`.
    pub content_type: Option<String>,
    /// Bodies larger than this take the multipart path. Defaults to the part
    /// size; `Some(u64::MAX)` forces a single PutObject.
    pub multipart_threshold: Option<u64>,
    /// Bytes per part (minimum 5 MiB, default 16 MiB).
    pub part_size: Option<u64>,
    /// Parts uploaded in parallel (default 4).
    pub concurrency: Option<usize>,
    pub attrs: PosixCreateAttrs,
}

impl PutObjectOptions {
    pub fn content_type(ct: impl Into<String>) -> Self {
        Self {
            content_type: Some(ct.into()),
            ..Default::default()
        }
    }
}

impl From<&str> for PutObjectOptions {
    fn from(ct: &str) -> Self {
        Self::content_type(ct)
    }
}

/// Options for [`Disk::append_object`](crate::Disk::append_object).
#[derive(Debug, Clone, Default)]
pub struct AppendObjectOptions {
    /// Applied only when the object is newly created.
    pub content_type: Option<String>,
    pub attrs: PosixCreateAttrs,
}

impl From<&str> for AppendObjectOptions {
    fn from(ct: &str) -> Self {
        Self {
            content_type: Some(ct.to_string()),
            ..Default::default()
        }
    }
}

/// S3's per-request cap on DeleteObjects keys.
pub(crate) const MAX_DELETE_OBJECTS_PER_REQUEST: usize = 1000;
/// Minimum size of every multipart part but the last (5 MiB).
pub(crate) const MIN_PART_SIZE: u64 = 5 * 1024 * 1024;
/// Default multipart part size (16 MiB).
pub(crate) const DEFAULT_PART_SIZE: u64 = 16 * 1024 * 1024;
pub(crate) const DEFAULT_UPLOAD_CONCURRENCY: usize = 4;
/// Server cap on parts per multipart upload.
pub(crate) const MAX_PARTS_PER_UPLOAD: u64 = 10_000;

/// Choose the part size for a `total_bytes` multipart upload: the requested
/// size unless that would exceed the server's 10,000-part cap, in which case
/// it grows (rounded up to a whole MiB) so the body fits.
pub fn effective_upload_part_size(total_bytes: u64, requested_part_size: u64) -> u64 {
    if total_bytes.div_ceil(requested_part_size) <= MAX_PARTS_PER_UPLOAD {
        return requested_part_size;
    }
    const MIB: u64 = 1024 * 1024;
    total_bytes.div_ceil(MAX_PARTS_PER_UPLOAD).div_ceil(MIB) * MIB
}

// ---------------------------------------------------------------- XML

/// A parsed XML element: text content and children, in document order.
#[derive(Debug, Default, Clone)]
pub(crate) struct Node {
    pub name: String,
    pub text: String,
    pub children: Vec<Node>,
}

impl Node {
    pub fn child(&self, name: &str) -> Option<&Node> {
        self.children.iter().find(|c| c.name == name)
    }
    pub fn all<'a>(&'a self, name: &'a str) -> impl Iterator<Item = &'a Node> {
        self.children.iter().filter(move |c| c.name == name)
    }
    /// Trimmed text of a child, if the child exists.
    pub fn text_of(&self, name: &str) -> Option<String> {
        self.child(name).map(|c| c.text.trim().to_string())
    }
    pub fn bool_of(&self, name: &str) -> bool {
        self.text_of(name).as_deref() == Some("true")
    }
    pub fn num_of<T: std::str::FromStr>(&self, name: &str) -> Option<T> {
        self.text_of(name).and_then(|t| t.parse().ok())
    }
}

/// Parse an XML document into a tree. Blank or malformed input yields an
/// empty root, like the TypeScript SDK's tolerant parser.
pub(crate) fn parse_xml(xml: &str) -> Node {
    let mut reader = Reader::from_str(xml);
    let mut stack: Vec<Node> = vec![Node::default()];
    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) => stack.push(Node {
                name: String::from_utf8_lossy(e.local_name().as_ref()).into_owned(),
                ..Default::default()
            }),
            Ok(Event::Empty(e)) => {
                let node = Node {
                    name: String::from_utf8_lossy(e.local_name().as_ref()).into_owned(),
                    ..Default::default()
                };
                if let Some(top) = stack.last_mut() {
                    top.children.push(node);
                }
            }
            Ok(Event::End(_)) => {
                if stack.len() > 1 {
                    let node = stack.pop().expect("non-empty");
                    stack.last_mut().expect("root").children.push(node);
                }
            }
            Ok(Event::Text(t)) => {
                if let (Some(top), Ok(text)) = (stack.last_mut(), t.decode()) {
                    top.text.push_str(&text);
                }
            }
            Ok(Event::CData(t)) => {
                if let Some(top) = stack.last_mut() {
                    top.text.push_str(&String::from_utf8_lossy(&t.into_inner()));
                }
            }
            Ok(Event::GeneralRef(r)) => {
                let resolved = match r.resolve_char_ref() {
                    Ok(Some(c)) => Some(c.to_string()),
                    _ => match r.decode().as_deref() {
                        Ok("amp") => Some("&".to_string()),
                        Ok("lt") => Some("<".to_string()),
                        Ok("gt") => Some(">".to_string()),
                        Ok("quot") => Some("\"".to_string()),
                        Ok("apos") => Some("'".to_string()),
                        _ => None,
                    },
                };
                if let (Some(top), Some(s)) = (stack.last_mut(), resolved) {
                    top.text.push_str(&s);
                }
            }
            Ok(Event::Eof) | Err(_) => break,
            Ok(_) => {}
        }
    }
    // Unwind any unclosed elements.
    while stack.len() > 1 {
        let node = stack.pop().expect("non-empty");
        stack.last_mut().expect("root").children.push(node);
    }
    let mut root = stack.pop().unwrap_or_default();
    // The synthetic root's only child is the document element.
    if root.children.len() == 1 {
        root.children.remove(0)
    } else {
        root
    }
}

/// Build an [`S3Error`] from a failed S3 response, parsing the XML body.
pub(crate) fn parse_s3_error(
    operation: &str,
    status: u16,
    status_text: &str,
    body: &str,
) -> S3Error {
    let root = parse_xml(body);
    let (code, message, request_id) = if root.name == "Error" {
        (
            root.text_of("Code"),
            root.text_of("Message"),
            root.text_of("RequestId"),
        )
    } else {
        (None, None, None)
    };
    build_s3_error(
        operation,
        status,
        status_text,
        code,
        message,
        request_id,
        body.to_string(),
    )
}

pub(crate) fn build_s3_error(
    operation: &str,
    status: u16,
    status_text: &str,
    code: Option<String>,
    message: Option<String>,
    request_id: Option<String>,
    raw: String,
) -> S3Error {
    let detail = message
        .clone()
        .or_else(|| Some(status_text.to_string()))
        .unwrap_or_default();
    let code_part = code.as_deref().map(|c| format!(" {c}")).unwrap_or_default();
    let tail = if detail.is_empty() {
        String::new()
    } else {
        format!(" — {detail}")
    };
    S3Error {
        message: format!("S3 {operation} failed: {status}{code_part}{tail}"),
        status,
        code,
        request_id,
        raw,
    }
}

fn parse_date(s: Option<String>) -> Option<DateTime<Utc>> {
    let s = s?;
    DateTime::parse_from_rfc3339(&s)
        .ok()
        .map(|d| d.with_timezone(&Utc))
}

pub(crate) fn parse_list_objects_result(xml: &str) -> ListObjectsResult {
    let root = parse_xml(xml);
    let root = if root.name == "ListBucketResult" {
        root
    } else {
        Node::default()
    };
    let objects: Vec<S3Object> = root
        .all("Contents")
        .map(|c| S3Object {
            key: c.text_of("Key").unwrap_or_default(),
            size: c.num_of("Size").unwrap_or(0),
            etag: c.text_of("ETag"),
            last_modified: parse_date(c.text_of("LastModified")),
        })
        .collect();
    let common_prefixes = root
        .all("CommonPrefixes")
        .filter_map(|c| c.text_of("Prefix"))
        .collect();
    ListObjectsResult {
        key_count: root.num_of("KeyCount").unwrap_or(objects.len()),
        objects,
        common_prefixes,
        is_truncated: root.bool_of("IsTruncated"),
        next_continuation_token: root.text_of("NextContinuationToken"),
        prefix: root.text_of("Prefix"),
    }
}

pub(crate) fn parse_delete_objects_result(xml: &str) -> DeleteObjectsResult {
    let root = parse_xml(xml);
    let root = if root.name == "DeleteResult" {
        root
    } else {
        Node::default()
    };
    DeleteObjectsResult {
        deleted: root
            .all("Deleted")
            .filter_map(|d| d.text_of("Key"))
            .collect(),
        errors: root
            .all("Error")
            .map(|e| DeleteObjectsError {
                key: e.text_of("Key").unwrap_or_default(),
                code: e.text_of("Code"),
                message: e.text_of("Message"),
            })
            .collect(),
    }
}

pub(crate) fn parse_list_parts_result(xml: &str) -> PartListing {
    let root = parse_xml(xml);
    let root = if root.name == "ListPartsResult" {
        root
    } else {
        Node::default()
    };
    let parts: Vec<PartInfo> = root
        .all("Part")
        .map(|p| PartInfo {
            part_number: p.num_of("PartNumber").unwrap_or(0),
            etag: p.text_of("ETag"),
            size: p.num_of("Size").unwrap_or(0),
            last_modified: parse_date(p.text_of("LastModified")),
        })
        .collect();
    PartListing {
        bucket: root.text_of("Bucket"),
        key: root.text_of("Key"),
        upload_id: root.text_of("UploadId"),
        is_truncated: root.bool_of("IsTruncated"),
        part_number_marker: root.num_of("PartNumberMarker").unwrap_or(0),
        next_part_number_marker: root.num_of("NextPartNumberMarker"),
        max_parts: root.num_of("MaxParts").unwrap_or(parts.len() as u32),
        parts,
    }
}

pub(crate) fn parse_list_multipart_uploads_result(xml: &str) -> MultipartUploadListing {
    let root = parse_xml(xml);
    let root = if root.name == "ListMultipartUploadsResult" {
        root
    } else {
        Node::default()
    };
    MultipartUploadListing {
        bucket: root.text_of("Bucket"),
        uploads: root
            .all("Upload")
            .map(|u| MultipartUploadSummary {
                key: u.text_of("Key").unwrap_or_default(),
                upload_id: u.text_of("UploadId").unwrap_or_default(),
                initiated: parse_date(u.text_of("Initiated")),
            })
            .collect(),
        common_prefixes: root
            .all("CommonPrefixes")
            .filter_map(|c| c.text_of("Prefix"))
            .collect(),
        is_truncated: root.bool_of("IsTruncated"),
        key_marker: root.text_of("KeyMarker"),
        upload_id_marker: root.text_of("UploadIdMarker"),
        next_key_marker: root.text_of("NextKeyMarker"),
        next_upload_id_marker: root.text_of("NextUploadIdMarker"),
        prefix: root.text_of("Prefix"),
        delimiter: root.text_of("Delimiter"),
        max_uploads: root.num_of("MaxUploads"),
    }
}

// ---------------------------------------------------------------- request bodies

fn escape_xml(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

pub(crate) fn build_delete_objects_xml(keys: &[String], quiet: bool) -> String {
    let objects: String = keys
        .iter()
        .map(|k| format!("<Object><Key>{}</Key></Object>", escape_xml(k)))
        .collect();
    let quiet = if quiet { "<Quiet>true</Quiet>" } else { "" };
    format!("<?xml version=\"1.0\" encoding=\"UTF-8\"?><Delete>{objects}{quiet}</Delete>")
}

pub(crate) fn build_complete_multipart_upload_xml(parts: &[UploadPart]) -> String {
    let body: String = parts
        .iter()
        .map(|p| {
            format!(
                "<Part><PartNumber>{}</PartNumber><ETag>{}</ETag></Part>",
                p.part_number,
                escape_xml(&p.etag)
            )
        })
        .collect();
    format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?><CompleteMultipartUpload>{body}</CompleteMultipartUpload>"
    )
}
