//! Bounded file transfers; a stable request ID resumes the same upload.
use super::*;
use sha2::{Digest, Sha256};
use std::io::{IsTerminal, Read, Seek, SeekFrom, Write};

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn sha_file(file: &mut std::fs::File) -> Result<String, String> {
    file.seek(SeekFrom::Start(0))
        .map_err(|_| "The file couldn't be read")?;
    let mut digest = Sha256::new();
    let mut bytes = vec![0; 1024 * 1024];
    loop {
        let count = file
            .read(&mut bytes)
            .map_err(|_| "The file couldn't be read")?;
        if count == 0 {
            break;
        }
        digest.update(&bytes[..count]);
    }
    file.seek(SeekFrom::Start(0))
        .map_err(|_| "The file couldn't be read")?;
    Ok(hex(&digest.finalize()))
}
impl Projects<'_> {
    pub(super) fn upload_large(&self, path: &Path, project: &str) -> Result<Value, String> {
        let mut file =
            std::fs::File::open(path).map_err(|error| format!("{}: {error}", path.display()))?;
        let metadata = file.metadata().map_err(|_| "The file couldn't be read")?;
        if !metadata.is_file() {
            return Err("Choose an ordinary file to upload".into());
        }
        let size = metadata.len();
        let sha256 = sha_file(&mut file)?;
        let name = path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned();
        let mut identity = Sha256::new();
        for field in [project, name.as_str(), &size.to_string(), sha256.as_str()] {
            identity.update((field.len() as u64).to_be_bytes());
            identity.update(field.as_bytes());
        }
        let digest = identity.finalize();
        let request = hex(&digest);
        let site = self.0;
        let mut status=self.send(site.http.post(site.url("/v1/files/uploads")).json(&json!({"request_id":request,"project":project,"filename":name,"size_bytes":size,"sha256":sha256})))?;
        let upload = status["id"]
            .as_str()
            .ok_or("The website didn't start a file upload")?
            .to_owned();
        safe(&upload)?;
        let prefix = format!("/v1/files/uploads/{upload}");
        let part_bytes = status["part_bytes"]
            .as_u64()
            .filter(|n| *n > 0 && *n <= 8 * 1024 * 1024)
            .ok_or("The website sent an unsupported file part size")?;
        let path_suffix = format!("?project={}", encode(project));
        if status["size_bytes"].as_u64() != Some(size) || status["sha256"].as_str() != Some(&sha256)
        {
            return Err("The website opened a different file upload".into());
        }
        let result = (|| {
            if status["complete"] != true {
                let mut offset = status["uploaded_bytes"]
                    .as_u64()
                    .ok_or("The website didn't report upload progress")?;
                let mut part = status["next_part"]
                    .as_u64()
                    .ok_or("The website didn't report the next file part")?;
                if offset > size
                    || (offset < size && part.checked_mul(part_bytes) != Some(offset))
                    || (offset == size && part != size.div_ceil(part_bytes))
                {
                    return Err("The website sent an invalid upload position".into());
                }
                file.seek(SeekFrom::Start(offset))
                    .map_err(|_| "The file couldn't be read")?;
                if offset > 0 {
                    eprintln!("Resuming {name} at {offset} of {size} bytes");
                }
                let mut bytes = vec![0u8; part_bytes as usize];
                while offset < size {
                    let count = (size - offset).min(part_bytes) as usize;
                    file.read_exact(&mut bytes[..count])
                        .map_err(|_| "The file changed or couldn't be read")?;
                    let digest = hex(&Sha256::digest(&bytes[..count]));
                    let sent = self.send(
                        site.http
                            .put(site.url(&format!("{prefix}/parts/{part}{path_suffix}")))
                            .timeout(TRANSFER)
                            .header("content-type", "application/octet-stream")
                            .header("x-part-sha256", digest)
                            .body(bytes[..count].to_vec()),
                    );
                    status = match sent {
                        Ok(status) => status,
                        Err(error) => {
                            // A lost part reply is recovered by reading the same session.
                            let recovered = self.get(&format!("{prefix}{path_suffix}"))?;
                            if recovered["uploaded_bytes"].as_u64() != Some(offset + count as u64) {
                                return Err(error);
                            }
                            recovered
                        }
                    };
                    offset += count as u64;
                    part += 1;
                    if status["uploaded_bytes"].as_u64() != Some(offset)
                        || status["next_part"].as_u64() != Some(part)
                    {
                        return Err("The website reported an unexpected upload position".into());
                    }
                    if std::io::stderr().is_terminal() {
                        eprint!("\rUploading {name}: {offset} / {size} bytes");
                        let _ = std::io::stderr().flush();
                    }
                }
                if std::io::stderr().is_terminal() {
                    eprintln!();
                }
            }
            self.send(
                site.http
                    .post(site.url(&format!("{prefix}/complete{path_suffix}")))
                    .timeout(Duration::from_secs(3600)),
            )
        })();
        result.map_err(|error| {
            format!("{error} Run the same files add command to resume this upload ({upload}).")
        })
    }
    pub(super) fn download_large(
        &self,
        route: &str,
        out: Option<&Path>,
        name: &str,
        size: u64,
        sha256: Option<&str>,
    ) -> Result<PathBuf, String> {
        let site = self.0;
        let mut response = site
            .http
            .get(site.url(route))
            .bearer_auth(site.saved.token())
            .timeout(Duration::from_secs(3600))
            .send()
            .map_err(|_| "The website couldn't be reached")?;
        if !response.status().is_success() {
            return Site::answer(Ok(response))
                .and_then(|_| Err("The download couldn't be opened".into()));
        }
        if response
            .content_length()
            .is_some_and(|length| length != size)
        {
            return Err("The downloaded file has a different size".into());
        }
        let path = out.map(Path::to_owned).unwrap_or_else(|| {
            PathBuf::from(
                Path::new(name)
                    .file_name()
                    .unwrap_or_else(|| std::ffi::OsStr::new("file")),
            )
        });
        let directory = path
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or(Path::new("."));
        let mut temporary = tempfile::NamedTempFile::new_in(directory)
            .map_err(|_| "The download folder couldn't be opened")?;
        let mut digest = Sha256::new();
        let mut received = 0u64;
        let mut bytes = vec![0u8; 1024 * 1024];
        loop {
            let count = response
                .read(&mut bytes)
                .map_err(|_| "The download was interrupted. Run files get again to retry")?;
            if count == 0 {
                break;
            }
            received = received
                .checked_add(count as u64)
                .ok_or("The download is too large")?;
            if received > size {
                return Err("The downloaded file has a different size".into());
            }
            digest.update(&bytes[..count]);
            temporary
                .write_all(&bytes[..count])
                .map_err(|_| "The download couldn't be saved")?;
        }
        if received != size || sha256.is_some_and(|sha| sha != hex(&digest.finalize())) {
            return Err("The downloaded file failed its size or SHA-256 check".into());
        }
        temporary
            .as_file()
            .sync_all()
            .map_err(|_| "The download couldn't be saved")?;
        if out.is_some() {
            temporary
                .persist(&path)
                .map_err(|_| "The download couldn't replace its output file")?;
        } else {
            temporary.persist_noclobber(&path).map_err(|_| {
                format!(
                    "{} already exists here. Pass --output PATH to save it somewhere else.",
                    path.display()
                )
            })?;
        }
        Ok(path)
    }
}
