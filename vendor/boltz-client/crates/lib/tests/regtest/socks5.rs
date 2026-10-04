#![allow(dead_code)]
//! A real SOCKS5 proxy container, so the proxy tests exercise the actual
//! handshake rather than a stub.
//!
//! Joined to the regtest stack's own docker network, which is what lets a test
//! address the backend by container name: a name the host cannot resolve, so a
//! request that succeeds can only have gone through the proxy.

use std::net::TcpStream;
use std::process::Command;
use std::time::{Duration, Instant};

use anyhow::{Context, Result, bail};
use platform_utils::ProxyConfig;

/// The backend's container name on the stack network, and the port nginx
/// serves inside it.
pub const BACKEND_HOST: &str = "boltz-backend-nginx";
pub const BACKEND_PORT: u16 = 9001;

const IMAGE: &str = "serjs/go-socks5-proxy";
const SOCKS_PORT: u16 = 1080;

fn docker(args: &[&str]) -> Result<String> {
    let output = Command::new("docker")
        .args(args)
        .output()
        .context("Failed to run docker. Is it installed and running?")?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        bail!("docker {args:?} failed: {stderr}");
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

/// The API URL the *proxy* resolves: the backend by container name.
pub fn backend_url() -> String {
    format!("http://{BACKEND_HOST}:{BACKEND_PORT}")
}

/// A running SOCKS5 proxy. The container is removed on drop, so keep it alive
/// for as long as the client under test needs it.
pub struct Socks5Proxy {
    container_id: String,
    config: ProxyConfig,
}

impl Socks5Proxy {
    /// Starts a proxy with no authentication.
    pub fn start() -> Result<Self> {
        Self::start_inner(None)
    }

    /// Starts a proxy that requires the given credentials.
    pub fn start_with_credentials(user: &str, password: &str) -> Result<Self> {
        Self::start_inner(Some((user.to_string(), password.to_string())))
    }

    fn start_inner(credentials: Option<(String, String)>) -> Result<Self> {
        let (username, password) = match credentials {
            Some((user, password)) => (Some(user), Some(password)),
            None => (None, None),
        };
        let network = stack_network()?;

        let mut args = vec![
            "run".to_string(),
            "-d".to_string(),
            "--network".to_string(),
            network,
            // Port 0 lets the host pick, so concurrent runs don't collide.
            "-p".to_string(),
            format!("0:{SOCKS_PORT}"),
        ];
        match (&username, &password) {
            (Some(user), Some(password)) => {
                args.extend(["-e".to_string(), format!("PROXY_USER={user}")]);
                args.extend(["-e".to_string(), format!("PROXY_PASSWORD={password}")]);
            }
            // The image refuses to start unless authentication is either
            // configured or explicitly waived.
            _ => args.extend(["-e".to_string(), "REQUIRE_AUTH=false".to_string()]),
        }
        args.push(IMAGE.to_string());

        let refs: Vec<&str> = args.iter().map(String::as_str).collect();
        let container_id = docker(&refs)?;

        let proxy = Self {
            config: ProxyConfig {
                host: "127.0.0.1".to_string(),
                port: published_port(&container_id)?,
                username,
                password,
            },
            container_id,
        };
        proxy.wait_until_listening()?;
        Ok(proxy)
    }

    /// The config to put on `BoltzConfig::proxy`.
    pub fn config(&self) -> ProxyConfig {
        self.config.clone()
    }

    /// The container publishes on the host only once its listener is up, and
    /// `docker run -d` returns before that.
    fn wait_until_listening(&self) -> Result<()> {
        const TIMEOUT: Duration = Duration::from_secs(30);
        let started = Instant::now();
        let addr = format!("127.0.0.1:{}", self.config.port);
        while started.elapsed() < TIMEOUT {
            if TcpStream::connect(&addr).is_ok() {
                return Ok(());
            }
            std::thread::sleep(Duration::from_millis(200));
        }
        bail!("SOCKS5 proxy did not start listening on {addr} within 30s")
    }
}

impl Drop for Socks5Proxy {
    fn drop(&mut self) {
        let _ = docker(&["rm", "-f", &self.container_id]);
    }
}

/// The docker network the regtest stack is on, read off a running container so
/// the test does not have to know the compose project name.
fn stack_network() -> Result<String> {
    let network = docker(&[
        "inspect",
        "-f",
        "{{range $k, $v := .NetworkSettings.Networks}}{{$k}}{{end}}",
        BACKEND_HOST,
    ])
    .context("Could not inspect the Boltz backend. Is the regtest stack running?")?;
    if network.is_empty() {
        bail!("{BACKEND_HOST} is on no docker network");
    }
    Ok(network)
}

/// The host port docker mapped to the container's SOCKS port.
fn published_port(container_id: &str) -> Result<u16> {
    let mapping = docker(&["port", container_id, &SOCKS_PORT.to_string()])?;
    // e.g. "0.0.0.0:54321\n[::]:54321" — either line carries the port.
    mapping
        .lines()
        .next()
        .and_then(|line| line.rsplit_once(':'))
        .and_then(|(_, port)| port.trim().parse().ok())
        .ok_or_else(|| anyhow::anyhow!("Could not parse published port from '{mapping}'"))
}
