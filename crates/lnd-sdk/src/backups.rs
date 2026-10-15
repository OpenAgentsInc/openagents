//! Static channel backups and watchtower registration. Never restore channel.db.
use crate::{
    Client, Error, Result,
    proto::{lnrpc, wtclientrpc},
    transport,
};
use std::time::Duration;

/// The payload is already encrypted by LND with a seed-derived key. Apply
/// independent encryption before sending it to off-host storage.
pub struct Backup {
    bytes: Vec<u8>,
    pub channels: usize,
}
impl Backup {
    pub fn encrypted_bytes(&self) -> &[u8] {
        &self.bytes
    }
    pub fn from_snapshot(snapshot: lnrpc::ChanBackupSnapshot) -> Result<Self> {
        let multi = snapshot
            .multi_chan_backup
            .ok_or(Error::Invalid("channel backup"))?;
        if multi.multi_chan_backup.is_empty() || multi.multi_chan_backup.len() > 16 * 1024 * 1024 {
            return Err(Error::Invalid("channel backup size"));
        }
        Ok(Self {
            channels: multi.chan_points.len(),
            bytes: multi.multi_chan_backup,
        })
    }
}
impl std::fmt::Debug for Backup {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Backup")
            .field("channels", &self.channels)
            .finish_non_exhaustive()
    }
}
#[async_trait::async_trait]
pub trait BackupSink: Send {
    /// Return only after saving the complete backup. A failure stops delivery;
    /// restarting always exports a new complete snapshot.
    async fn save(&mut self, backup: Backup) -> Result<()>;
}
impl Client {
    pub async fn channel_backup(&self) -> Result<Backup> {
        self.ensure_current().await?;
        Backup::from_snapshot(
            transport::read(|| {
                let mut rpc = self.rpc();
                async move {
                    rpc.export_all_channel_backups(lnrpc::ChanBackupExportRequest {})
                        .await
                }
            })
            .await?,
        )
    }
    /// Stream changes and export every 30 seconds to cover subscription gaps.
    /// This includes gaps while LND waits to send subscription headers.
    pub async fn channel_backups(&self, sink: &mut impl BackupSink) -> Result<()> {
        loop {
            sink.save(self.channel_backup().await?).await?;
            let mut rpc = self.rpc();
            let subscription = rpc.subscribe_channel_backups(lnrpc::ChannelBackupSubscription {});
            tokio::pin!(subscription);
            let mut poll = tokio::time::interval(Duration::from_secs(30));
            poll.tick().await;
            let response = loop {
                tokio::select! {
                    response = &mut subscription => break response,
                    _ = poll.tick() => sink.save(self.channel_backup().await?).await?,
                }
            };
            let mut stream = match response {
                Ok(r) => r.into_inner(),
                Err(e) => match Error::from(e) {
                    Error::Unavailable | Error::Timeout => {
                        tokio::time::sleep(Duration::from_secs(1)).await;
                        continue;
                    }
                    error => return Err(error),
                },
            };
            loop {
                tokio::select! {
                    next = stream.message() => match next {
                        Ok(Some(snapshot)) => sink.save(Backup::from_snapshot(snapshot)?).await?,
                        Ok(None) => break,
                        Err(e) => match Error::from(e) {
                            Error::Unavailable | Error::Timeout => break,
                            error => return Err(error),
                        }
                    },
                    _ = poll.tick() => sink.save(self.channel_backup().await?).await?,
                }
            }
            tokio::time::sleep(Duration::from_secs(1)).await;
        }
    }
    pub async fn add_watchtower(&self, public_key: [u8; 33], address: String) -> Result<()> {
        if !matches!(public_key[0], 2 | 3) || address.is_empty() || address.len() > 512 {
            return Err(Error::Invalid("watchtower address"));
        }
        self.ensure_current().await?;
        transport::once(
            wtclientrpc::watchtower_client_client::WatchtowerClientClient::new(
                self.channel.clone(),
            )
            .add_tower(wtclientrpc::AddTowerRequest {
                pubkey: public_key.to_vec(),
                address,
            }),
        )
        .await?;
        Ok(())
    }
    pub async fn watchtowers(&self) -> Result<Vec<wtclientrpc::Tower>> {
        transport::read(|| {
            let mut rpc = wtclientrpc::watchtower_client_client::WatchtowerClientClient::new(
                self.channel.clone(),
            );
            async move {
                rpc.list_towers(wtclientrpc::ListTowersRequest {
                    include_sessions: true,
                    exclude_exhausted_sessions: false,
                })
                .await
            }
        })
        .await
        .map(|r| r.towers)
    }
}
