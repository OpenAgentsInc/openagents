//! Cloud Storage V4 signed URLs (`GOOG4-RSA-SHA256`) for one `GET`.
//!
//! The signature itself comes from the caller: the broker sends
//! [`Draft::string_to_sign`] to the IAM Credentials API's `signBlob`, which
//! signs with the service account's Google-managed key, and passes the
//! result to [`Draft::finish`]. No key is ever held here.
//! See <https://cloud.google.com/storage/docs/access-control/signing-urls-manually>.

use crate::sha256_hex;

/// Cloud Storage's host for path-style URLs.
pub const HOST: &str = "storage.googleapis.com";
const ALGORITHM: &str = "GOOG4-RSA-SHA256";

/// A URL ready for its signature.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Draft {
    path: String,
    query: String,
    string_to_sign: String,
}

/// RFC 3986 percent-encoding of everything but the unreserved characters,
/// and `/` when `keep_slash`.
fn encode(value: &str, keep_slash: bool) -> String {
    let mut out = String::with_capacity(value.len());
    for b in value.bytes() {
        if b.is_ascii_alphanumeric()
            || matches!(b, b'-' | b'.' | b'_' | b'~')
            || (keep_slash && b == b'/')
        {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

/// The UTC civil date of `unix` seconds, as `YYYYMMDD` and `YYYYMMDDTHHMMSSZ`.
#[must_use]
pub fn timestamps(unix: u64) -> (String, String) {
    let days = (unix / 86_400) as i64;
    let seconds = unix % 86_400;
    // Howard Hinnant's days-to-civil.
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    let date = format!("{year:04}{month:02}{day:02}");
    let time = format!(
        "{date}T{:02}{:02}{:02}Z",
        seconds / 3_600,
        seconds % 3_600 / 60,
        seconds % 60
    );
    (date, time)
}

/// `unix` seconds as an RFC 3339 UTC time, such as `2026-10-06T22:15:30Z`.
#[must_use]
pub fn rfc3339(unix: u64) -> String {
    let (_, t) = timestamps(unix);
    format!(
        "{}-{}-{}T{}:{}:{}Z",
        &t[0..4],
        &t[4..6],
        &t[6..8],
        &t[9..11],
        &t[11..13],
        &t[13..15]
    )
}

impl Draft {
    /// A `GET` of `object` in `bucket` for `email`'s key, valid for
    /// `expires` seconds from `now`.
    #[must_use]
    pub fn new(bucket: &str, object: &str, email: &str, now: u64, expires: u32) -> Self {
        let (date, time) = timestamps(now);
        let scope = format!("{date}/auto/storage/goog4_request");
        let path = format!("/{}/{}", encode(bucket, false), encode(object, true));
        // Sorted by name, as the canonical query requires.
        let query = [
            ("X-Goog-Algorithm", ALGORITHM.to_owned()),
            ("X-Goog-Credential", format!("{email}/{scope}")),
            ("X-Goog-Date", time.clone()),
            ("X-Goog-Expires", expires.to_string()),
            ("X-Goog-SignedHeaders", "host".to_owned()),
        ]
        .iter()
        .map(|(k, v)| format!("{k}={}", encode(v, false)))
        .collect::<Vec<_>>()
        .join("&");
        let canonical = format!("GET\n{path}\n{query}\nhost:{HOST}\n\nhost\nUNSIGNED-PAYLOAD");
        let string_to_sign = format!(
            "{ALGORITHM}\n{time}\n{scope}\n{}",
            sha256_hex(canonical.as_bytes())
        );
        Self {
            path,
            query,
            string_to_sign,
        }
    }

    /// The bytes the service account signs with RSA-SHA256.
    #[must_use]
    pub fn string_to_sign(&self) -> &[u8] {
        self.string_to_sign.as_bytes()
    }

    /// The signed URL, given the raw RSA signature over
    /// [`Self::string_to_sign`].
    #[must_use]
    pub fn finish(&self, signature: &[u8]) -> String {
        let hex: String = signature.iter().map(|b| format!("{b:02x}")).collect();
        format!(
            "https://{HOST}{}?{}&X-Goog-Signature={hex}",
            self.path, self.query
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn timestamps_are_utc_civil_dates() {
        assert_eq!(
            timestamps(0),
            ("19700101".into(), "19700101T000000Z".into())
        );
        // 2026-10-06T22:15:30Z
        assert_eq!(
            timestamps(1_791_324_930),
            ("20261006".into(), "20261006T221530Z".into())
        );
        // A leap day.
        assert_eq!(timestamps(1_709_164_800).0, "20240229");
        assert_eq!(rfc3339(1_791_324_930), "2026-10-06T22:15:30Z");
    }

    #[test]
    fn a_draft_is_canonical_and_its_url_carries_the_signature() {
        let draft = Draft::new(
            "bucket",
            "packs/ab.vtp",
            "broker@project.iam.gserviceaccount.com",
            1_791_324_930,
            300,
        );
        let text = String::from_utf8(draft.string_to_sign().to_vec()).unwrap();
        let lines: Vec<&str> = text.lines().collect();
        assert_eq!(lines[0], "GOOG4-RSA-SHA256");
        assert_eq!(lines[1], "20261006T221530Z");
        assert_eq!(lines[2], "20261006/auto/storage/goog4_request");
        let canonical = "GET\n/bucket/packs/ab.vtp\n\
            X-Goog-Algorithm=GOOG4-RSA-SHA256\
            &X-Goog-Credential=broker%40project.iam.gserviceaccount.com%2F20261006%2Fauto%2Fstorage%2Fgoog4_request\
            &X-Goog-Date=20261006T221530Z&X-Goog-Expires=300&X-Goog-SignedHeaders=host\n\
            host:storage.googleapis.com\n\nhost\nUNSIGNED-PAYLOAD";
        assert_eq!(lines[3], sha256_hex(canonical.as_bytes()));
        let url = draft.finish(&[0xde, 0xad]);
        assert!(
            url.starts_with("https://storage.googleapis.com/bucket/packs/ab.vtp?X-Goog-Algorithm=")
        );
        assert!(url.ends_with("&X-Goog-Signature=dead"));
    }
}
