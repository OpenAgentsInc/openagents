//! The remote script's result lines.
//!
//! Each result line is `oa-ssh` followed by `key=value` fields. Other lines,
//! such as a login banner from the remote shell's startup files, are
//! diagnostics and are ignored. The keys are a closed set: an unknown key or
//! a repeated one is a protocol error rather than something to guess at.

use std::collections::BTreeMap;

use crate::error::Error;

const PREFIX: &str = "oa-ssh ";

const KEYS: &[&str] = &[
    "os",
    "arch",
    "unsupported_os",
    "unsupported_arch",
    "lock",
    "install",
    "sha",
    "need",
    "host",
    "ownership",
    "pid",
    "port",
    "invitation",
    "error",
];

/// The fields one script run reported.
#[derive(Debug, Default)]
pub(crate) struct Report {
    fields: BTreeMap<&'static str, String>,
    /// How many times the run reclaimed a lock from a dead owner.
    pub(crate) reclaimed: u32,
}

impl Report {
    pub(crate) fn get(&self, key: &str) -> Option<&str> {
        self.fields.get(key).map(String::as_str)
    }

    pub(crate) fn require(&self, key: &str) -> Result<&str, Error> {
        self.get(key)
            .ok_or_else(|| Error::Protocol(format!("the result has no {key} field")))
    }

    /// The remote error, mapped to the error a caller can act on.
    pub(crate) fn failure(&self) -> Option<Error> {
        let code = self.get("error")?;
        let platform = || {
            (
                self.get("os")
                    .or(self.get("unsupported_os"))
                    .unwrap_or("unknown")
                    .to_string(),
                self.get("arch")
                    .or(self.get("unsupported_arch"))
                    .unwrap_or("unknown")
                    .to_string(),
            )
        };
        Some(match code {
            "unsupported-platform" => {
                let (os, arch) = platform();
                Error::Unsupported { os, arch }
            }
            "no-artifact" => {
                let (os, arch) = platform();
                Error::NoArtifact { os, arch }
            }
            "checksum-mismatch" => Error::ChecksumMismatch,
            "bad-archive" => Error::BadArchive,
            "binary-rejected" => Error::BinaryRejected,
            "busy" => Error::Busy,
            "host-did-not-start" => Error::HostDidNotStart,
            "not-installed" => Error::NotInstalled,
            "invite-failed" | "invite-malformed" => Error::Invitation(code.to_string()),
            "missing-sha256-tool" => Error::MissingTool("a SHA-256 tool".into()),
            "missing-tar" => Error::MissingTool("tar".into()),
            "missing-gzip" => Error::MissingTool("gzip".into()),
            other => Error::Remote(other.to_string()),
        })
    }
}

/// Reads the result lines from a script run's standard output.
pub(crate) fn parse(stdout: &[u8]) -> Result<Report, Error> {
    let text = String::from_utf8_lossy(stdout);
    let mut report = Report::default();
    for line in text.lines() {
        let Some(rest) = line.strip_prefix(PREFIX) else {
            continue;
        };
        for field in rest.split(' ').filter(|field| !field.is_empty()) {
            let (key, value) = field
                .split_once('=')
                .ok_or_else(|| Error::Protocol(format!("a field has no value: {field}")))?;
            let key = KEYS
                .iter()
                .find(|known| **known == key)
                .ok_or_else(|| Error::Protocol(format!("unknown result field {key}")))?;
            if *key == "lock" {
                if value != "reclaimed" {
                    return Err(Error::Protocol(format!("unknown lock result {value}")));
                }
                report.reclaimed += 1;
                continue;
            }
            if report.fields.insert(key, value.to_string()).is_some() {
                return Err(Error::Protocol(format!("repeated result field {key}")));
            }
        }
    }
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn banners_are_ignored_and_fields_are_closed() {
        let report = parse(
            b"Welcome to the box\noa-ssh os=linux arch=x86_64\noa-ssh lock=reclaimed\noa-ssh host=started ownership=managed pid=12 port=47001\n",
        )
        .unwrap();
        assert_eq!(report.get("os"), Some("linux"));
        assert_eq!(report.get("port"), Some("47001"));
        assert_eq!(report.reclaimed, 1);
        assert!(parse(b"oa-ssh surprise=1\n").is_err());
        assert!(parse(b"oa-ssh os=linux\noa-ssh os=macos\n").is_err());
    }

    #[test]
    fn remote_errors_map_to_named_failures() {
        let report =
            parse(b"oa-ssh unsupported_os=Plan9 unsupported_arch=mips\noa-ssh error=unsupported-platform\n")
                .unwrap();
        match report.failure() {
            Some(Error::Unsupported { os, arch }) => {
                assert_eq!(os, "Plan9");
                assert_eq!(arch, "mips");
            }
            other => panic!("unexpected {other:?}"),
        }
        assert!(matches!(
            parse(b"oa-ssh error=busy\n").unwrap().failure(),
            Some(Error::Busy)
        ));
    }
}
