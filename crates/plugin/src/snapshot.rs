//! Virtual snapshot entries a snapshot-read guest may list and read.
//!
//! Names are logical labels. A label may have `/`-separated segments, such
//! as a workspace-relative path, but no segment is empty, `.`, or `..`, so
//! a label never names anything outside the snapshot. A symlink, a parent
//! segment, or a handle from another invocation isn't a path the guest may
//! follow.

use std::collections::BTreeMap;

use serde_json::{Value, json};

/// One snapshot entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Entry {
    /// File bytes and the observation version they belong to.
    File {
        /// Retained bytes.
        bytes: Vec<u8>,
        /// Observation version.
        version: String,
        /// Whether the capture includes every byte of the source.
        complete: bool,
    },
    /// A directory of child names.
    Directory {
        /// Child names in listing order.
        children: Vec<String>,
    },
    /// A symlink. Resolution refuses it.
    Symlink {
        /// Recorded target. It is not followed.
        target: String,
    },
}

/// Entries addressed by logical name.
#[derive(Debug, Clone, Default)]
pub struct Snapshot {
    entries: BTreeMap<String, Entry>,
}

impl Snapshot {
    /// Insert `entry` under `name`.
    ///
    /// # Errors
    ///
    /// Returns a reason `name` isn't a logical label: empty, absolute, or
    /// with an empty, `.`, or `..` segment, a backslash, or a NUL byte.
    pub fn insert(&mut self, name: &str, entry: Entry) -> Result<(), &'static str> {
        if !logical_name(name) {
            return Err("snapshot name");
        }
        self.entries.insert(name.to_string(), entry);
        Ok(())
    }

    /// The entry named `name`.
    #[must_use]
    pub fn get(&self, name: &str) -> Option<&Entry> {
        self.entries.get(name)
    }

    /// Metadata for `name`.
    ///
    /// # Errors
    ///
    /// Returns a reason the name is absent or is a symlink.
    pub fn metadata(&self, name: &str) -> Result<Value, &'static str> {
        match self.entries.get(name) {
            Some(Entry::File {
                bytes,
                version,
                complete: _,
            }) => Ok(json!({"type": "file", "size": bytes.len(), "version": version})),
            Some(Entry::Directory { children }) => Ok(json!({
                "type": "directory",
                "size": children.len(),
                "version": "dir"
            })),
            Some(Entry::Symlink { .. }) => Err("symlink"),
            None => Err("missing entry"),
        }
    }

    /// List `name` when it is a directory.
    ///
    /// # Errors
    ///
    /// Returns a reason the name is not a directory, or the cursor is stale.
    pub fn list(
        &self,
        name: &str,
        cursor: Option<&str>,
        max_entries: usize,
    ) -> Result<Value, &'static str> {
        let Entry::Directory { children } = self.entries.get(name).ok_or("missing entry")? else {
            return Err("not a directory");
        };
        let start = match cursor {
            None => 0,
            Some(cursor) => cursor.parse::<usize>().map_err(|_| "cursor")?,
        };
        if start > children.len() {
            return Err("cursor");
        }
        let end = (start + max_entries).min(children.len());
        let entries: Vec<Value> = children[start..end]
            .iter()
            .map(|child| {
                let (kind, version) = match self.entries.get(child.as_str()) {
                    Some(Entry::Directory { .. }) => ("directory", "dir"),
                    Some(Entry::Symlink { .. }) => ("symlink", ""),
                    Some(Entry::File { version, .. }) => ("file", version.as_str()),
                    None => ("file", ""),
                };
                // The host replaces `handle` with a token scoped to the
                // invocation before the guest sees it.
                json!({"handle": child, "name": child, "type": kind, "version": version})
            })
            .collect();
        let next = if end < children.len() {
            Value::String(end.to_string())
        } else {
            Value::Null
        };
        Ok(json!({
            "entries": entries,
            "next_cursor": next,
            "complete": end == children.len()
        }))
    }

    /// Read at most `max_bytes` from a file at `offset`.
    ///
    /// # Errors
    ///
    /// Returns a reason the entry is missing, a symlink, or the wrong version.
    pub fn read(
        &self,
        name: &str,
        offset: usize,
        max_bytes: usize,
        version: Option<&str>,
    ) -> Result<Value, &'static str> {
        let Entry::File {
            bytes,
            version: have,
            complete: _,
        } = self.entries.get(name).ok_or("missing entry")?
        else {
            return Err("not a file");
        };
        if version.is_some_and(|want| want != have) {
            return Err("stale version");
        }
        if offset > bytes.len() {
            return Err("offset");
        }
        let end = (offset + max_bytes).min(bytes.len());
        Ok(json!({
            "offset": offset,
            "bytes_base64": encode_base64(&bytes[offset..end]),
            "eof": end == bytes.len(),
            "version": have
        }))
    }
}

/// Bytes of a file the caller wants to treat as a complete derivative.
///
/// # Errors
///
/// Returns a reason a partial capture was claimed as complete.
pub fn derivative(
    bytes: &[u8],
    complete: bool,
    claim_complete: bool,
) -> Result<&[u8], &'static str> {
    if claim_complete && !complete {
        return Err("partial capture");
    }
    Ok(bytes)
}

/// Decode a base64 string produced by [`encode_base64`].
///
/// # Errors
///
/// Returns an error when the text is not padded base64.
pub fn decode_base64(text: &str) -> Result<Vec<u8>, &'static str> {
    fn value(byte: u8) -> Result<u8, &'static str> {
        match byte {
            b'A'..=b'Z' => Ok(byte - b'A'),
            b'a'..=b'z' => Ok(byte - b'a' + 26),
            b'0'..=b'9' => Ok(byte - b'0' + 52),
            b'+' => Ok(62),
            b'/' => Ok(63),
            _ => Err("base64"),
        }
    }
    let bytes = text.as_bytes();
    if bytes.is_empty() || !bytes.len().is_multiple_of(4) {
        return Err("base64");
    }
    let mut out = Vec::with_capacity(bytes.len() / 4 * 3);
    for chunk in bytes.chunks(4) {
        let pad = usize::from(chunk[2] == b'=') + usize::from(chunk[3] == b'=');
        let first = value(chunk[0])?;
        let second = value(chunk[1])?;
        let third = if chunk[2] == b'=' {
            0
        } else {
            value(chunk[2])?
        };
        let fourth = if chunk[3] == b'=' {
            0
        } else {
            value(chunk[3])?
        };
        let packed = (u32::from(first) << 18)
            | (u32::from(second) << 12)
            | (u32::from(third) << 6)
            | u32::from(fourth);
        out.push((packed >> 16) as u8);
        if pad < 2 {
            out.push((packed >> 8) as u8);
        }
        if pad < 1 {
            out.push(packed as u8);
        }
    }
    Ok(out)
}

fn logical_name(name: &str) -> bool {
    !name.contains('\\')
        && !name.contains('\0')
        && name
            .split('/')
            .all(|segment| !segment.is_empty() && segment != "." && segment != "..")
}

/// Encode `bytes` as base64.
#[must_use]
pub fn encode_base64(bytes: &[u8]) -> String {
    const TABLE: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::new();
    let mut index = 0;
    while index + 3 <= bytes.len() {
        let n = u32::from(bytes[index]) << 16
            | u32::from(bytes[index + 1]) << 8
            | u32::from(bytes[index + 2]);
        out.push(TABLE[((n >> 18) & 63) as usize] as char);
        out.push(TABLE[((n >> 12) & 63) as usize] as char);
        out.push(TABLE[((n >> 6) & 63) as usize] as char);
        out.push(TABLE[(n & 63) as usize] as char);
        index += 3;
    }
    if index < bytes.len() {
        let rest = &bytes[index..];
        let n = if rest.len() == 1 {
            u32::from(rest[0]) << 16
        } else {
            u32::from(rest[0]) << 16 | u32::from(rest[1]) << 8
        };
        out.push(TABLE[((n >> 18) & 63) as usize] as char);
        out.push(TABLE[((n >> 12) & 63) as usize] as char);
        if rest.len() == 1 {
            out.push('=');
        } else {
            out.push(TABLE[((n >> 6) & 63) as usize] as char);
        }
        out.push('=');
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn traversal_symlink_and_partial_capture_are_refused() {
        let mut snapshot = Snapshot::default();
        assert!(
            snapshot
                .insert(
                    "../secret",
                    Entry::File {
                        bytes: vec![1],
                        version: "1".into(),
                        complete: true,
                    }
                )
                .is_err()
        );
        for refused in [
            "",
            "/etc/passwd",
            "src/../secret",
            "src//lib.rs",
            "./note",
            "a\\b",
        ] {
            let entry = Entry::Directory {
                children: Vec::new(),
            };
            assert!(snapshot.insert(refused, entry).is_err(), "{refused:?}");
        }
        let nested = Entry::Directory {
            children: Vec::new(),
        };
        assert!(snapshot.insert("src/lib.rs", nested).is_ok());
        snapshot
            .insert(
                "note",
                Entry::File {
                    bytes: b"hello".to_vec(),
                    version: "v1".into(),
                    complete: false,
                },
            )
            .unwrap();
        snapshot
            .insert(
                "link",
                Entry::Symlink {
                    target: "/etc/passwd".into(),
                },
            )
            .unwrap();
        assert_eq!(snapshot.metadata("link").unwrap_err(), "symlink");
        assert_eq!(snapshot.read("link", 0, 4, None).unwrap_err(), "not a file");
        assert!(snapshot.read("note", 0, 4, Some("other")).is_err());
        let file = snapshot.get("note").unwrap();
        let Entry::File {
            bytes, complete, ..
        } = file
        else {
            panic!("note is a file");
        };
        assert_eq!(
            derivative(bytes, *complete, true).unwrap_err(),
            "partial capture"
        );
        assert_eq!(derivative(bytes, *complete, false).unwrap(), b"hello");
    }
}
