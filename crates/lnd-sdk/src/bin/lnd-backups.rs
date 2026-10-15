//! Export complete channel backups to a private tmpfs file for the cloud uploader.
use lnd_sdk::{
    Client, Error,
    backups::{Backup, BackupSink},
    macaroon::{Macaroon, Role},
    transport::Config,
};
use serde::Deserialize;
use std::path::{Path, PathBuf};
#[derive(Deserialize)]
struct Settings {
    endpoint: String,
    certificate_sha256: String,
    macaroon: PathBuf,
    output: PathBuf,
}
fn atomic_write(path: &Path, bytes: &[u8]) -> Result<(), Box<dyn std::error::Error>> {
    let temporary = path.with_extension(format!("{}.tmp", std::process::id()));
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o640);
    }
    let mut file = options.open(&temporary)?;
    use std::io::Write;
    file.write_all(bytes)?;
    file.sync_all()?;
    std::fs::rename(temporary, path)?;
    Ok(())
}

struct FileSink(PathBuf);
#[async_trait::async_trait]
impl BackupSink for FileSink {
    async fn save(&mut self, backup: Backup) -> lnd_sdk::Result<()> {
        atomic_write(&self.0, backup.encrypted_bytes()).map_err(|_| Error::Checkpoint)
    }
}
async fn run() -> Result<(), Box<dyn std::error::Error>> {
    let path = std::env::args_os()
        .nth(1)
        .ok_or("a settings file is required")?;
    let settings: Settings = serde_json::from_slice(&std::fs::read(path)?)?;
    let pin = hex::decode(&settings.certificate_sha256)?
        .try_into()
        .map_err(|_| Error::CertificatePin)?;
    let client = Client::connect(
        &Config::new(settings.endpoint, pin),
        Macaroon::from_file(Role::ReadOnly, 203, &settings.macaroon)?,
    )
    .await?;
    client
        .channel_backups(&mut FileSink(settings.output))
        .await?;
    Ok(())
}
#[tokio::main]
async fn main() {
    if run().await.is_err() {
        eprintln!("Could not save channel backups. Check the node and backup settings.");
        std::process::exit(1);
    }
}
