//! Isolated Docker regtest lifecycle. This module is absent from normal builds.
use crate::{Error, Result};
use serde_json::Value;
use std::{
    path::{Path, PathBuf},
    process::Command,
    time::{Duration, Instant},
};

/// Fresh regtest seed only. Never accepts a production seed.
pub struct TestSeed(zeroize::Zeroizing<Vec<String>>);
impl std::fmt::Debug for TestSeed {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("TestSeed([redacted])")
    }
}
pub struct Regtest {
    root: PathBuf,
    project: String,
}
impl Regtest {
    pub fn start(root: &Path) -> Result<Self> {
        Self::start_mode(root, false).map(|r| r.0)
    }
    pub fn recoverable(root: &Path) -> Result<(Self, TestSeed)> {
        let (nodes, seed) = Self::start_mode(root, true)?;
        Ok((nodes, seed.ok_or(Error::Invalid("test seed"))?))
    }
    fn start_mode(root: &Path, recoverable: bool) -> Result<(Self, Option<TestSeed>)> {
        if !root.is_absolute() {
            return Err(Error::Invalid("absolute regtest directory"));
        }
        std::fs::create_dir_all(root).map_err(|_| Error::Unavailable)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(root, std::fs::Permissions::from_mode(0o700))
                .map_err(|_| Error::Unavailable)?;
        }
        let project = format!(
            "oa-lnd-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        );
        let result = Self {
            root: root.into(),
            project,
        };
        let mut compose = include_str!("../../../deploy/lnd-sdk/compose.yaml").to_owned();
        if recoverable {
            compose = compose.replacen("\"--noseedbackup\", ", "", 1);
        }
        std::fs::write(root.join("compose.yaml"), compose).map_err(|_| Error::Unavailable)?;
        result.command(&["up", "-d", "bitcoin"])?;
        result.wait(|| result.bitcoin(&["getblockchaininfo"]).map(|_| ()))?;
        result.bitcoin(&["createwallet", "miner"])?;
        let address = result
            .bitcoin(&["getnewaddress"])?
            .as_str()
            .ok_or(Error::Unavailable)?
            .to_owned();
        result.bitcoin(&["generatetoaddress", "101", &address])?;
        result.command(&["up", "-d", "alice", "bob", "tower", "testnet"])?;
        let seed = if recoverable {
            Some(result.initialize(None, None)?)
        } else {
            None
        };
        for node in ["alice", "bob", "tower"] {
            result.wait(|| result.lncli(node, &["getinfo"]).map(|_| ()))?;
        }
        result.wait(|| result.testnet(&["getinfo"]).map(|_| ()))?;
        result.command(&["up", "-d", "tapd"])?;
        result.wait(|| {
            result
                .command(&[
                    "exec",
                    "-T",
                    "tapd",
                    "tapcli",
                    "--tapddir=/tap",
                    "--network=regtest",
                    "getinfo",
                ])
                .map(|_| ())
        })?;
        Ok((result, seed))
    }
    fn initialize(&self, seed: Option<&TestSeed>, backup: Option<&[u8]>) -> Result<TestSeed> {
        use crate::{
            macaroon::{Macaroon, Role},
            proto::lnrpc,
            transport,
        };
        self.wait(|| {
            self.command(&[
                "exec",
                "-T",
                "alice",
                "lncli",
                "--lnddir=/data",
                "--network=regtest",
                "state",
            ])
            .map(|_| ())
        })?;
        let config = self.tls_config("alice")?;
        tokio::runtime::Runtime::new()
            .map_err(|_| Error::Unavailable)?
            .block_on(async {
                let channel =
                    transport::connect(&config, Macaroon::new(Role::Ops, 0, vec![0])?).await?;
                let mut rpc = lnrpc::wallet_unlocker_client::WalletUnlockerClient::new(channel);
                let words = match seed {
                    Some(seed) => seed.0.to_vec(),
                    None => {
                        let mut generated =
                            transport::once(rpc.gen_seed(lnrpc::GenSeedRequest::default())).await?;
                        use zeroize::Zeroize;
                        generated.enciphered_seed.zeroize();
                        generated.cipher_seed_mnemonic
                    }
                };
                let saved = TestSeed(zeroize::Zeroizing::new(words.clone()));
                transport::once(rpc.init_wallet(lnrpc::InitWalletRequest {
                    wallet_password: b"regtest-wallet-only".to_vec(),
                    cipher_seed_mnemonic: words,
                    recovery_window: if seed.is_some() { 100 } else { 0 },
                    channel_backups: backup.map(|b| lnrpc::ChanBackupSnapshot {
                        multi_chan_backup: Some(lnrpc::MultiChanBackup {
                            chan_points: vec![],
                            multi_chan_backup: b.to_vec(),
                        }),
                        ..Default::default()
                    }),
                    ..Default::default()
                }))
                .await?;
                Ok(saved)
            })
    }
    /// Discard only this fixture's databases after stopping the sole writer.
    /// TLS files are kept so tests can verify the same pinned endpoint.
    pub fn restore(&self, seed: &TestSeed, backup: &[u8]) -> Result<()> {
        self.command(&["stop", "tapd", "alice"])?;
        self.command(&[
            "run",
            "--rm",
            "--no-deps",
            "--entrypoint",
            "sh",
            "alice",
            "-c",
            "rm -rf /data/data",
        ])?;
        self.command(&["up", "-d", "alice"])?;
        self.initialize(Some(seed), Some(backup))?;
        self.wait(|| self.lncli("alice", &["getinfo"]).map(|_| ()))
    }
    pub fn command(&self, args: &[&str]) -> Result<Vec<u8>> {
        let mut command = Command::new("docker");
        command
            .args(["compose", "--project-name", &self.project, "--file"])
            .arg(self.root.join("compose.yaml"))
            .args(args);
        let output = std::thread::spawn(move || {
            let runtime = tokio::runtime::Runtime::new().map_err(|_| Error::Unavailable)?;
            Ok::<_, Error>(
                runtime.block_on(
                    supervise::Job::from_command(command)
                        .bounded(supervise::Limits {
                            stream_max: 1024 * 1024,
                            ..supervise::Limits::within(Duration::from_secs(45))
                        })
                        .run(),
                ),
            )
        })
        .join()
        .map_err(|_| Error::Unavailable)??;
        if !output.ending.success() || output.truncated() {
            // Docker diagnostics may contain credentials. Keep them in the private
            // scratch directory rather than returning them in an error message.
            let _ = std::fs::write(self.root.join("docker-error.log"), &output.stderr.text);
            return Err(Error::Unavailable);
        }
        Ok(output.stdout.text.into_bytes())
    }
    pub fn bitcoin(&self, args: &[&str]) -> Result<Value> {
        let mut cmd = vec![
            "exec",
            "-T",
            "bitcoin",
            "bitcoin-cli",
            "-regtest",
            "-rpcuser=regtest",
            "-rpcpassword=regtest-only",
        ];
        cmd.extend_from_slice(args);
        let out = self.command(&cmd)?;
        Ok(serde_json::from_slice(&out)
            .unwrap_or_else(|_| Value::String(String::from_utf8_lossy(&out).trim().into())))
    }
    pub fn lncli(&self, node: &str, args: &[&str]) -> Result<Value> {
        if !["alice", "bob", "tower"].contains(&node) {
            return Err(Error::Invalid("regtest node"));
        }
        let mut cmd = vec![
            "exec",
            "-T",
            node,
            "lncli",
            "--lnddir=/data",
            "--network=regtest",
        ];
        cmd.extend_from_slice(args);
        serde_json::from_slice(&self.command(&cmd)?).map_err(|_| Error::Invalid("lncli response"))
    }
    pub fn testnet(&self, args: &[&str]) -> Result<Value> {
        let mut cmd = vec![
            "exec",
            "-T",
            "testnet",
            "lncli",
            "--lnddir=/data",
            "--network=testnet",
        ];
        cmd.extend_from_slice(args);
        serde_json::from_slice(&self.command(&cmd)?).map_err(|_| Error::Invalid("lncli response"))
    }
    pub fn wait(&self, mut check: impl FnMut() -> Result<()>) -> Result<()> {
        let deadline = Instant::now() + Duration::from_secs(90);
        loop {
            if check().is_ok() {
                return Ok(());
            }
            if Instant::now() >= deadline {
                return Err(Error::Timeout);
            }
            std::thread::sleep(Duration::from_millis(250));
        }
    }
    pub fn mine(&self, blocks: u32) -> Result<()> {
        let address = self
            .bitcoin(&["getnewaddress"])?
            .as_str()
            .ok_or(Error::Unavailable)?
            .to_owned();
        self.bitcoin(&["generatetoaddress", &blocks.to_string(), &address])?;
        Ok(())
    }
    pub fn tls_config(&self, node: &str) -> Result<crate::transport::Config> {
        use sha2::{Digest, Sha256};
        let published = String::from_utf8(self.command(&["port", node, "10009"])?)
            .map_err(|_| Error::Unavailable)?;
        let cert = self.command(&["exec", "-T", node, "cat", "/data/tls.cert"])?;
        let der = rustls_pemfile::certs(&mut cert.as_slice())
            .next()
            .ok_or(Error::CertificatePin)?
            .map_err(|_| Error::CertificatePin)?;
        Ok(crate::transport::Config::new(
            format!("https://{}", published.trim()),
            Sha256::digest(der).into(),
        ))
    }
    pub fn config(
        &self,
        node: &str,
    ) -> Result<(crate::transport::Config, crate::macaroon::Macaroon)> {
        let config = self.tls_config(node)?;
        let bytes = self.command(&[
            "exec",
            "-T",
            node,
            "od",
            "-An",
            "-v",
            "-tx1",
            "/data/data/chain/bitcoin/regtest/admin.macaroon",
        ])?;
        Ok((
            config,
            crate::macaroon::Macaroon::new(
                crate::macaroon::Role::Ops,
                0,
                hex::decode(
                    bytes
                        .into_iter()
                        .filter(|b| !b.is_ascii_whitespace())
                        .collect::<Vec<_>>(),
                )
                .map_err(|_| Error::Invalid("regtest credential"))?,
            )?,
        ))
    }
}
impl Drop for Regtest {
    fn drop(&mut self) {
        let _ = self.command(&["down", "--volumes", "--remove-orphans"]);
    }
}
