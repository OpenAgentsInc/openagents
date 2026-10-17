//! Private, bounded account records. No default home or login is consulted.
use crate::{Profile, Result, failed};
use fs2::FileExt;
use std::{
    fs::{File, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
};
const MAX_BYTES: u64 = 1024 * 1024;

pub(crate) struct Store {
    directory: PathBuf,
    _lock: File,
}
impl Store {
    pub(crate) fn open(directory: &Path) -> Result<Self> {
        #[cfg(not(unix))]
        {
            let _ = directory;
            return Err(failed(
                "Project sharing is not supported on this device yet.",
            ));
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::{DirBuilderExt, MetadataExt, OpenOptionsExt};
            std::fs::DirBuilder::new()
                .recursive(true)
                .mode(0o700)
                .create(directory)
                .map_err(io)?;
            let metadata = std::fs::symlink_metadata(directory).map_err(io)?;
            if !metadata.is_dir()
                || metadata.uid() != unsafe { libc::geteuid() }
                || metadata.mode() & 0o077 != 0
            {
                return Err(failed("The saved computer folder is not private."));
            }
            let lock = OpenOptions::new()
                .read(true)
                .write(true)
                .create(true)
                .truncate(false)
                .mode(0o600)
                .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
                .open(directory.join("lock"))
                .map_err(io)?;
            private(&lock)?;
            lock.try_lock_exclusive().map_err(io)?;
            Ok(Self {
                directory: directory.into(),
                _lock: lock,
            })
        }
    }
    fn read(&self, name: &str) -> Result<Option<Vec<u8>>> {
        let mut options = OpenOptions::new();
        options.read(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK);
        }
        let file = match options.open(self.directory.join(name)) {
            Ok(file) => file,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(io(error)),
        };
        private(&file)?;
        if file.metadata().map_err(io)?.len() > MAX_BYTES {
            return Err(failed("The saved computer list is too large."));
        }
        let mut bytes = Vec::new();
        file.take(MAX_BYTES + 1)
            .read_to_end(&mut bytes)
            .map_err(io)?;
        if bytes.len() > MAX_BYTES as usize {
            return Err(failed("The saved computer list is too large."));
        }
        Ok(Some(bytes))
    }
    fn write(&self, name: &str, bytes: &[u8]) -> Result<()> {
        if bytes.len() > MAX_BYTES as usize {
            return Err(failed("The saved computer list is too large."));
        }
        let temporary = self
            .directory
            .join(format!("save-{}", coder_access::protocol::random_id()));
        let mut options = OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600).custom_flags(libc::O_NOFOLLOW);
        }
        let mut file = options.open(&temporary).map_err(io)?;
        let result = (|| {
            file.write_all(bytes).map_err(io)?;
            file.sync_all().map_err(io)?;
            std::fs::rename(&temporary, self.directory.join(name)).map_err(io)?;
            File::open(&self.directory)
                .and_then(|file| file.sync_all())
                .map_err(io)
        })();
        if result.is_err() {
            let _ = std::fs::remove_file(&temporary);
        }
        result
    }
    pub(crate) fn profile(&self, account: &str) -> Result<Profile> {
        match self.read("computers.json")? {
            Some(bytes) => {
                let profile: Profile = serde_json::from_slice(&bytes)
                    .map_err(|_| failed("The saved computer list cannot be read."))?;
                if profile.v != "openagents.project-sharing.v1"
                    || profile.account != account
                    || profile.connections.len() > 64
                    || profile.pending.len() > 128
                    || profile.attachments.len() > 128
                    || profile.jobs.len() > 32
                    || profile.contacts.len() > 64
                {
                    return Err(failed(
                        "This computer list belongs to another account or version.",
                    ));
                }
                Ok(profile)
            }
            None => Ok(Profile::new(account)),
        }
    }
    pub(crate) fn save(&self, profile: &Profile) -> Result<()> {
        self.write(
            "computers.json",
            &serde_json::to_vec(profile)
                .map_err(|_| failed("The computer list could not be saved."))?,
        )
    }
    pub(crate) fn key(&self, account: &str) -> Result<secp256k1::SecretKey> {
        use std::str::FromStr;
        #[derive(serde::Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Key {
            v: String,
            account: String,
            secret: String,
        }
        match self.read("device.key")? {
            Some(mut bytes) => {
                let parsed = serde_json::from_slice::<Key>(&bytes);
                bytes.fill(0);
                let key = parsed.map_err(|_| failed("The device key could not be read."))?;
                let text = computer_driver::Secret::new(key.secret);
                if key.v != "openagents.sharing-device.v1" || key.account != account {
                    return Err(failed("This device key belongs to another account."));
                }
                secp256k1::SecretKey::from_str(text.expose())
                    .map_err(|_| failed("The device key could not be read."))
            }
            None => {
                if self.read("computers.json")?.is_some() {
                    return Err(failed(
                        "The saved device key is missing. Restore the app's data from backup.",
                    ));
                }
                let key = secp256k1::SecretKey::new(&mut secp256k1::rand::rng());
                let secret = computer_driver::Secret::new(key.display_secret().to_string());
                let mut bytes=serde_json::to_vec(&serde_json::json!({"v":"openagents.sharing-device.v1","account":account,"secret":secret.expose()})).map_err(|_|failed("The device key could not be saved."))?;
                let result = self.write("device.key", &bytes);
                bytes.fill(0);
                result?;
                Ok(key)
            }
        }
    }
}
fn private(file: &File) -> Result<()> {
    let metadata = file.metadata().map_err(io)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        if !metadata.is_file()
            || metadata.nlink() != 1
            || metadata.uid() != unsafe { libc::geteuid() }
            || metadata.mode() & 0o077 != 0
        {
            return Err(failed("The saved computer file is not private."));
        }
    }
    #[cfg(not(unix))]
    {
        let _ = metadata;
        return Err(failed(
            "Project sharing is not supported on this device yet.",
        ));
    }
    Ok(())
}
fn io(_: std::io::Error) -> crate::Error {
    failed("The saved computer list is unavailable. Try again later.")
}
