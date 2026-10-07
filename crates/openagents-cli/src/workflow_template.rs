//! Explicit local meeting-template commands. Domain rules live in Coder.

use std::fs::File;
use std::io::Read;
use std::os::fd::{AsRawFd, FromRawFd};
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::PermissionsExt;
use std::path::{Component, Path};

use coder::workflow_template::{self as template, Approval, Protected, Report, Snapshot};
use serde_json::{Value, json};

const USAGE: &str = "openagents plugin template prepare --package DIR --source FILE --task ID --customer ID --owner ID --recipient ID --permission-epoch N
openagents plugin template run --package DIR --source FILE --snapshot FILE --approve DIGEST --recipient ID --permission-epoch N
openagents plugin template check --source FILE --snapshot FILE --report FILE --protected FILE --protected-sha256 DIGEST";

pub(crate) fn run(words: &[String]) -> Result<Value, String> {
    let args = crate::Args::parse(words, &[])?;
    let [command] = args.positional() else {
        return Err(USAGE.into());
    };
    let allowed: &[&str] = match command.as_str() {
        "prepare" => &[
            "package",
            "source",
            "task",
            "customer",
            "owner",
            "recipient",
            "permission-epoch",
        ],
        "run" => &[
            "package",
            "source",
            "snapshot",
            "approve",
            "recipient",
            "permission-epoch",
        ],
        "check" => &[
            "source",
            "snapshot",
            "report",
            "protected",
            "protected-sha256",
        ],
        _ => return Err(USAGE.into()),
    };
    if args
        .option_names()
        .iter()
        .any(|name| !allowed.contains(name))
    {
        return Err(USAGE.into());
    }
    let required = |name| {
        args.option(name)
            .filter(|s| !s.is_empty())
            .ok_or_else(|| USAGE.to_owned())
    };
    let source = read_file(
        Path::new(required("source")?),
        template::MAX_SOURCE_BYTES,
        true,
    )?;
    if command == "prepare" || command == "run" {
        let dir = Path::new(required("package")?);
        template::release(
            &read_file(&dir.join("package.json"), template::MAX_RECORD_BYTES, false)?,
            &read_file(
                &dir.join("programs/meeting-followup.json"),
                template::MAX_PROGRAM_BYTES,
                false,
            )?,
        )?;
    }
    let epoch = || {
        required("permission-epoch")?
            .parse::<u64>()
            .map_err(|_| "Permission epoch must be a positive integer.".to_owned())
    };
    let snapshot: Snapshot = if command == "prepare" {
        Snapshot {
            schema: "openagents.workflow-template.snapshot.v1".into(),
            task: required("task")?.into(),
            customer: required("customer")?.into(),
            human_owner: required("owner")?.into(),
            recipient: required("recipient")?.into(),
            permission_epoch: epoch()?,
            source_sha256: template::sha256(&source),
            package_digest: coder::package::digest(template::PACKAGE),
            program_digest: coder::package::digest(template::PROGRAM),
        }
    } else {
        serde_json::from_slice(&read_file(
            Path::new(required("snapshot")?),
            template::MAX_RECORD_BYTES,
            true,
        )?)
        .map_err(|_| "Invalid private template snapshot JSON.".to_owned())?
    };
    snapshot.bind_source(&source)?;
    match command.as_str() {
        "prepare" => Ok(
            json!({"snapshot":snapshot,"snapshot_digest":snapshot.digest(),
            "text":"Prepared exact input/release references. Review this snapshot before approving its digest; no workflow ran."}),
        ),
        "run" => {
            let approval = Approval {
                snapshot_digest: required("approve")?.into(),
                recipient: required("recipient")?.into(),
                permission_epoch: epoch()?,
            };
            let report = crate::runtime().block_on(template::run(&snapshot, &approval, &source))?;
            Ok(
                json!({"succeeded":report.with_template.finished && report.with_template.value.is_some() && report.without_file_access.finished && report.without_file_access.value.is_some(), "report":report,"text":"Computed local template and no-file-access comparison attempts. Save the report privately and run the separate protected check; output is untrusted data and delivery remains manual."}),
            )
        }
        "check" => {
            let report: Report = serde_json::from_slice(&read_file(
                Path::new(required("report")?),
                template::MAX_RECORD_BYTES,
                true,
            )?)
            .map_err(|_| "Invalid private template report JSON.".to_owned())?;
            let expected_bytes = read_file(
                Path::new(required("protected")?),
                template::MAX_RECORD_BYTES,
                true,
            )?;
            if template::sha256(&expected_bytes) != required("protected-sha256")? {
                return Err(
                    "Protected check bytes do not match the separately frozen digest.".into(),
                );
            }
            let expected: Protected = serde_json::from_slice(&expected_bytes)
                .map_err(|_| "Invalid protected check JSON.".to_owned())?;
            let check = template::check(&snapshot, &report, &source, &expected)?;
            Ok(
                json!({"check":check,"text":"Checked exact source citations and protected expectations. Customer acceptance, publication, and independent attestation remain separate."}),
            )
        }
        _ => unreachable!(),
    }
}

/// Open each component with no-follow semantics. Recheck the opened file,
/// rather than trusting metadata observed before an open or following a FIFO.
fn read_file(path: &Path, max: usize, private: bool) -> Result<Vec<u8>, String> {
    let path = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()
            .map_err(|e| e.to_string())?
            .join(path)
    };
    let parts: Vec<_> = path
        .components()
        .filter_map(|c| match c {
            Component::Normal(s) => Some(Ok(s)),
            Component::RootDir => None,
            _ => Some(Err(
                "Template paths must have no parent or current-directory segments.".to_owned(),
            )),
        })
        .collect::<Result<_, _>>()?;
    if parts.is_empty() {
        return Err("A template file path is required.".into());
    }
    let mut parent = File::open("/").map_err(|e| e.to_string())?;
    for (index, part) in parts.iter().enumerate() {
        let last = index + 1 == parts.len();
        if last
            && private
            && parent
                .metadata()
                .map_err(|e| e.to_string())?
                .permissions()
                .mode()
                & 0o077
                != 0
        {
            return Err("Private template files require a parent directory with mode 0700.".into());
        }
        let name = std::ffi::CString::new(part.as_bytes())
            .map_err(|_| "Invalid template file path.".to_owned())?;
        let flags = libc::O_RDONLY
            | libc::O_CLOEXEC
            | libc::O_NOFOLLOW
            | libc::O_NONBLOCK
            | if last { 0 } else { libc::O_DIRECTORY };
        // SAFETY: parent owns a live descriptor; name is a terminated string.
        let fd = unsafe { libc::openat(parent.as_raw_fd(), name.as_ptr(), flags) };
        if fd < 0 {
            return Err(format!(
                "Cannot open template file without following links: {}",
                std::io::Error::last_os_error()
            ));
        }
        // SAFETY: openat returned a new descriptor, now owned exactly once.
        parent = unsafe { File::from_raw_fd(fd) };
    }
    let meta = parent.metadata().map_err(|e| e.to_string())?;
    if !meta.is_file()
        || meta.len() > max as u64
        || (private && meta.permissions().mode() & 0o077 != 0)
    {
        return Err(
            "Template input must be a bounded regular file; private records require mode 0600."
                .into(),
        );
    }
    let mut bytes = Vec::new();
    parent
        .take((max + 1) as u64)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() > max {
        return Err("Template file grew beyond its byte bound.".into());
    }
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn template_reads_refuse_links_fifos_wide_permissions_and_unknown_options() {
        use std::os::unix::fs::symlink;
        let dir = tempfile::tempdir().unwrap();
        std::fs::set_permissions(dir.path(), std::fs::Permissions::from_mode(0o700)).unwrap();
        let root = dir.path().canonicalize().unwrap();
        let file = root.join("source");
        std::fs::write(&file, "synthetic").unwrap();
        assert!(read_file(&file, 100, true).is_err());
        std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o600)).unwrap();
        assert_eq!(read_file(&file, 100, true).unwrap(), b"synthetic");
        assert!(read_file(&file, 1, true).is_err());
        symlink(&file, root.join("link")).unwrap();
        symlink(&root, root.join("parent-link")).unwrap();
        assert!(read_file(&root.join("link"), 100, true).is_err());
        assert!(read_file(&root.join("parent-link/source"), 100, true).is_err());
        let fifo = std::ffi::CString::new(root.join("fifo").as_os_str().as_bytes()).unwrap();
        // SAFETY: the path is terminated and names a scratch fixture.
        assert_eq!(unsafe { libc::mkfifo(fifo.as_ptr(), 0o600) }, 0);
        assert!(read_file(&root.join("fifo"), 100, true).is_err());
        let words = ["prepare", "--unknown", "x"].map(str::to_string);
        assert!(run(&words).is_err());
    }
}
