//! Lists a tailnet's devices through Tailscale's control server.
//!
//! The app registers as its own tailnet node with `ts_control` and asks for
//! one netmap. It never joins the data plane. A node that is not yet
//! authorized gets a sign-in URL; after the user signs in, registering again
//! with the same node key succeeds.

use std::path::{Path, PathBuf};
use std::time::Duration;
use tokio::io::AsyncReadExt;
use ts_control::{MapRequestBuilder, RegistrationError};
use ts_control_serde::MapResponse;
use ts_keys::{NodeState, PersistState};
use url::Url;

/// The name this app registers under in the tailnet.
const HOSTNAME: &str = "openagents-ios";
/// The largest netmap frame the app reads.
const MAX_FRAME_BYTES: usize = 16 * 1024 * 1024;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Device {
    pub name: String,
    pub os: String,
    pub address: String,
    pub online: Option<bool>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Tailnet {
    pub name: Option<String>,
    pub this_device: Option<String>,
    pub devices: Vec<Device>,
}

pub enum Outcome {
    Devices(Tailnet),
    SignIn(Url),
}

pub struct Client {
    keys: NodeState,
    config: ts_control::Config,
}

impl Client {
    /// Load this app's node keys from `dir`, or create and save new ones.
    pub fn open(dir: &Path) -> Result<Self, String> {
        let path: PathBuf = dir.join("tailscale-node.json");
        let state: PersistState = match std::fs::read(&path) {
            Ok(bytes) => serde_json::from_slice(&bytes)
                .map_err(|_| "Couldn't read this phone's saved Tailscale sign-in.".to_string())?,
            Err(_) => {
                let state = PersistState::default();
                std::fs::create_dir_all(dir).map_err(|e| format!("Could not save keys: {e}"))?;
                let bytes = serde_json::to_vec(&state).map_err(|e| e.to_string())?;
                write_private(&path, &bytes).map_err(|e| format!("Could not save keys: {e}"))?;
                state
            }
        };
        Ok(Self {
            keys: NodeState::from(state),
            config: ts_control::Config {
                server_url: ts_control::DEFAULT_CONTROL_SERVER.clone(),
                hostname: Some(HOSTNAME.into()),
                client_name: Some("OpenAgents".into()),
                tags: vec![],
                ephemeral: false,
            },
        })
    }

    /// Register, then read the netmap. `followup` long-polls until the user
    /// finishes signing in at that URL.
    pub async fn devices(&self, followup: Option<Url>, limit: Duration) -> Result<Outcome, String> {
        tokio::time::timeout(limit, self.fetch(followup))
            .await
            .map_err(|_| "Tailscale did not answer in time.".to_string())?
    }

    async fn fetch(&self, followup: Option<Url>) -> Result<Outcome, String> {
        let control = &self.config.server_url;
        let conn = ts_control::connect(control, &self.keys.machine_keys)
            .await
            .map_err(|e| format!("Could not reach Tailscale: {e}"))?;
        match ts_control::register(&self.config, control, None, followup, &self.keys, &conn).await {
            Ok(()) => {}
            Err(RegistrationError::MachineNotAuthorized(Some(url))) => {
                return Ok(Outcome::SignIn(url));
            }
            Err(RegistrationError::MachineNotAuthorized(None)) => {
                return Err("Tailscale did not authorize this device.".into());
            }
            Err(e) => return Err(format!("Tailscale registration failed: {e}")),
        }
        // Every map request carries the hostname; without it control
        // renames the node to a default such as `node-1`.
        let request = MapRequestBuilder::new(&self.keys)
            .hostname(HOSTNAME)
            .stream(false)
            .omit_peers(false)
            .build();
        let map_url = control.join("machine/map").map_err(|e| e.to_string())?;
        let mut reader = ts_control::client::send_map_request(request, &map_url, &conn)
            .await
            .map_err(|e| format!("Could not read the tailnet: {e}"))?;
        loop {
            let len = reader
                .read_u32_le()
                .await
                .map_err(|_| "Couldn't read your tailnet. Try again.".to_string())?
                as usize;
            if len > MAX_FRAME_BYTES {
                return Err("Your tailnet is too large to show here.".into());
            }
            let mut frame = vec![0; len];
            reader
                .read_exact(&mut frame)
                .await
                .map_err(|_| "Couldn't read your tailnet. Try again.".to_string())?;
            let map: MapResponse = serde_json::from_slice(&frame)
                .map_err(|_| "Couldn't read your tailnet. Try again.".to_string())?;
            if let Some(peers) = &map.peers {
                return Ok(Outcome::Devices(tailnet(&map, peers)));
            }
        }
    }
}

fn tailnet(map: &MapResponse, peers: &[ts_control_serde::Node]) -> Tailnet {
    let mut devices: Vec<Device> = peers
        .iter()
        .map(|peer| Device {
            name: short_name(peer.name, peer.host_info.hostname),
            os: os_label(peer.host_info.os),
            address: peer.addresses.0.addr().to_string(),
            online: peer.online,
        })
        .collect();
    devices.sort_by_key(|device| device.name.to_lowercase());
    Tailnet {
        name: (!map.domain.is_empty()).then(|| map.domain.to_owned()),
        this_device: map
            .node
            .as_ref()
            .map(|node| short_name(node.name, node.host_info.hostname)),
        devices,
    }
}

/// The MagicDNS label, such as `laptop` for `laptop.tail1234.ts.net.`.
fn short_name(name: &str, hostname: Option<&str>) -> String {
    match name.split('.').next().filter(|label| !label.is_empty()) {
        Some(label) => label.to_owned(),
        None => hostname.unwrap_or("Unnamed device").to_owned(),
    }
}

pub fn os_label(os: &str) -> String {
    match os.to_ascii_lowercase().as_str() {
        "" => "Unknown".into(),
        "ios" => "iOS".into(),
        "ipados" => "iPadOS".into(),
        "macos" | "darwin" => "macOS".into(),
        "tvos" => "tvOS".into(),
        "linux" => "Linux".into(),
        "windows" => "Windows".into(),
        "android" => "Android".into(),
        "freebsd" => "FreeBSD".into(),
        "openbsd" => "OpenBSD".into(),
        _ => os.to_owned(),
    }
}

fn write_private(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    use std::io::Write;
    use std::os::unix::fs::OpenOptionsExt;
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(path)?;
    file.write_all(bytes)
}
