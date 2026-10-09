//! Sign a command-line app in to an openagents.com account (docs/auth,
//! "Apps and the command line").
//!
//! The device-code flow (RFC 8628) against the website:
//!
//! 1. [`start`] asks `POST {origin}/device/code` for a short code and
//!    the page to enter it on (`{origin}/device`).
//! 2. The person opens that page (signed in), checks the code, and
//!    approves. [`open_browser`] opens it when this computer has a
//!    browser.
//! 3. [`wait`] polls `POST {origin}/device/token` at the server's
//!    interval (slower on `slow_down`) until the account's token arrives,
//!    or the code is denied or expires.
//! 4. [`Saved::store`] keeps the token in a 0600 file; [`sign_out`] ends
//!    it on the server and [`Saved::forget`] removes the file.
//!
//! The token is a session on the person's account, revocable from
//! Settings on the website. It never appears in `Debug` output, and
//! nothing here logs it.

use std::path::{Path, PathBuf};
use std::time::Duration;

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

/// The production website.
pub const DEFAULT_ORIGIN: &str = "https://openagents.com";

/// The environment variable that points sign-in at another site, such as
/// the local fixture (`http://127.0.0.1:4301`).
pub const ORIGIN_ENV: &str = "OPENAGENTS_ORIGIN";

/// The file the token lives in, under the app's own folder.
pub const FILE: &str = "account.json";

/// The site to sign in to: `OPENAGENTS_ORIGIN`, or openagents.com.
#[must_use]
pub fn origin_from(get_env: impl Fn(&str) -> Option<String>) -> String {
    get_env(ORIGIN_ENV)
        .map(|value| value.trim().trim_end_matches('/').to_string())
        .filter(|value| {
            value.starts_with("https://")
                || value.starts_with("http://127.0.0.1:")
                || value.starts_with("http://localhost:")
        })
        .unwrap_or_else(|| DEFAULT_ORIGIN.to_string())
}

/// Whether `pair` looks like a pair code: 16 to 64 letters and digits.
#[must_use]
pub fn valid_pair(pair: &str) -> bool {
    (16..=64).contains(&pair.len()) && pair.bytes().all(|b| b.is_ascii_alphanumeric())
}

/// This computer's name, as the approval page shows it.
#[must_use]
pub fn computer_name() -> String {
    let from = |program: &str, args: &[&str]| {
        std::process::Command::new(program)
            .args(args)
            .output()
            .ok()
            .filter(|out| out.status.success())
            .and_then(|out| String::from_utf8(out.stdout).ok())
            .map(|name| name.trim().to_string())
            .filter(|name| !name.is_empty())
    };
    std::env::var("OPENAGENTS_COMPUTER_NAME")
        .ok()
        .filter(|name| !name.trim().is_empty())
        .or_else(|| {
            if cfg!(target_os = "macos") {
                from("scutil", &["--get", "ComputerName"])
            } else {
                None
            }
        })
        .or_else(|| from("hostname", &[]))
        .map(|name| {
            let name = name.trim_end_matches(".local");
            name.chars().filter(|c| !c.is_control()).take(64).collect()
        })
        .unwrap_or_else(|| "this computer".to_string())
}

/// Why sign-in stopped.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Error {
    /// The site couldn't be reached or answered strangely.
    Unreachable(String),
    /// The person pressed Deny.
    Denied,
    /// The code ran out before anyone approved it.
    Expired,
    /// The site refused the request; its words.
    Refused(String),
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Unreachable(origin) => write!(
                f,
                "Couldn't reach {origin}. Check your connection and try again."
            ),
            Self::Denied => f.write_str("Sign-in was denied on the website."),
            Self::Expired => {
                f.write_str("The code expired before it was approved. Run login again.")
            }
            Self::Refused(message) => f.write_str(message),
        }
    }
}

impl std::error::Error for Error {}

/// A started sign-in: what to show the person.
#[derive(Clone, Deserialize)]
pub struct Started {
    device_code: String,
    pub user_code: String,
    pub verification_uri: String,
    #[serde(default)]
    pub verification_uri_complete: Option<String>,
    pub expires_in: u64,
    pub interval: u64,
}

impl std::fmt::Debug for Started {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Started")
            .field("user_code", &self.user_code)
            .field("verification_uri", &self.verification_uri)
            .finish_non_exhaustive()
    }
}

/// The signed-in account, as kept on disk.
#[derive(Clone, Deserialize, Serialize, PartialEq, Eq)]
pub struct Saved {
    /// The site this token belongs to.
    pub origin: String,
    /// The account's id and display name.
    pub account: String,
    pub label: String,
    /// When the token stops working, as Unix seconds.
    pub expires_at: u64,
    token: String,
}

impl std::fmt::Debug for Saved {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Saved")
            .field("origin", &self.origin)
            .field("account", &self.account)
            .field("label", &self.label)
            .field("token", &"[redacted]")
            .finish()
    }
}

impl Saved {
    /// The bearer token, for calls made as this account.
    #[must_use]
    pub fn token(&self) -> &str {
        &self.token
    }

    /// Whether the token is past its deadline at `now`.
    #[must_use]
    pub fn expired(&self, now: u64) -> bool {
        self.expires_at <= now
    }

    /// The saved account in `dir`, if there is one. A file that isn't
    /// ours (wrong shape) reads as none.
    pub fn load(dir: &Path) -> Option<Self> {
        let text = std::fs::read_to_string(dir.join(FILE)).ok()?;
        serde_json::from_str(&text).ok()
    }

    /// Keep this account in `dir/account.json`, readable only by this
    /// user (0600), the folder 0700.
    pub fn store(&self, dir: &Path) -> Result<PathBuf, String> {
        let path = dir.join(FILE);
        let mut bytes = serde_json::to_vec_pretty(self).map_err(|e| e.to_string())?;
        bytes.push(b'\n');
        let written = write_private(&path, &bytes);
        bytes.fill(0);
        written.map(|()| path)
    }

    /// Remove the saved account from `dir`.
    pub fn forget(dir: &Path) -> Result<(), String> {
        match std::fs::remove_file(dir.join(FILE)) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(_) => Err(format!("Couldn't remove {}.", dir.join(FILE).display())),
        }
    }
}

fn write_private(file: &Path, bytes: &[u8]) -> Result<(), String> {
    use std::io::Write;
    let dir = file
        .parent()
        .ok_or_else(|| format!("{} has no folder", file.display()))?;
    std::fs::create_dir_all(dir).map_err(|_| format!("Couldn't make {}.", dir.display()))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o700));
    }
    let tmp = dir.join(format!(".{FILE}.{}.tmp", std::process::id()));
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create(true).truncate(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let fail = || format!("Couldn't write {}.", file.display());
    let mut handle = options.open(&tmp).map_err(|_| fail())?;
    handle.write_all(bytes).map_err(|_| fail())?;
    handle.sync_all().map_err(|_| fail())?;
    drop(handle);
    std::fs::rename(&tmp, file).map_err(|_| {
        let _ = std::fs::remove_file(&tmp);
        fail()
    })
}

fn client() -> Result<reqwest::Client, Error> {
    reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .connect_timeout(Duration::from_secs(10))
        .timeout(Duration::from_secs(20))
        .build()
        .map_err(|_| Error::Unreachable("the network".into()))
}

async fn post(
    http: &reqwest::Client,
    origin: &str,
    path: &str,
    body: Value,
    bearer: Option<&str>,
) -> Result<(u16, Value), Error> {
    let mut request = http.post(format!("{origin}{path}")).json(&body);
    if let Some(token) = bearer {
        request = request.bearer_auth(token);
    }
    let response = request
        .send()
        .await
        .map_err(|_| Error::Unreachable(origin.to_string()))?;
    let status = response.status().as_u16();
    let body = response
        .json::<Value>()
        .await
        .map_err(|_| Error::Unreachable(origin.to_string()))?;
    Ok((status, body))
}

/// Ask `origin` to start a sign-in for `app` on this computer.
pub async fn start(origin: &str, app: &str, computer: &str) -> Result<Started, Error> {
    start_paired(origin, app, computer, None).await
}

/// [`start`] with the pair code from the website's "Connect your
/// terminal" page (`coder login --pair <code>`), so the sign-in shows on
/// that page to approve there.
pub async fn start_paired(
    origin: &str,
    app: &str,
    computer: &str,
    pair: Option<&str>,
) -> Result<Started, Error> {
    let http = client()?;
    let mut body = json!({"app": app, "computer": computer});
    if let Some(pair) = pair.filter(|pair| valid_pair(pair)) {
        body["pair"] = json!(pair);
    }
    let (status, body) = post(&http, origin, "/device/code", body, None).await?;
    if status != 200 {
        return Err(Error::Refused(
            body["error_description"]
                .as_str()
                .unwrap_or("The site didn't start sign-in. Try again.")
                .to_string(),
        ));
    }
    serde_json::from_value(body).map_err(|_| Error::Unreachable(origin.to_string()))
}

/// One poll's answer.
#[derive(Debug)]
pub enum Poll {
    Signed(Saved),
    Pending,
    /// Poll slower: the new interval in seconds.
    SlowDown(u64),
}

/// Poll once.
pub async fn poll(origin: &str, started: &Started) -> Result<Poll, Error> {
    let http = client()?;
    poll_with(&http, origin, started).await
}

async fn poll_with(http: &reqwest::Client, origin: &str, started: &Started) -> Result<Poll, Error> {
    let (status, body) = post(
        http,
        origin,
        "/device/token",
        json!({
            "grant_type": "urn:ietf:params:oauth:grant-type:device_code",
            "device_code": started.device_code,
        }),
        None,
    )
    .await?;
    if status == 200 {
        let token = body["access_token"]
            .as_str()
            .filter(|t| t.starts_with("sess_"))
            .ok_or_else(|| Error::Unreachable(origin.to_string()))?;
        let now = unix_now();
        return Ok(Poll::Signed(Saved {
            origin: origin.to_string(),
            account: body["account"]["id"]
                .as_str()
                .unwrap_or_default()
                .to_string(),
            label: body["account"]["label"]
                .as_str()
                .unwrap_or("your account")
                .to_string(),
            expires_at: now + body["expires_in"].as_u64().unwrap_or(0),
            token: token.to_string(),
        }));
    }
    match body["error"].as_str() {
        Some("authorization_pending") => Ok(Poll::Pending),
        Some("slow_down") => Ok(Poll::SlowDown(
            body["interval"].as_u64().unwrap_or(started.interval + 5),
        )),
        Some("access_denied") => Err(Error::Denied),
        Some("expired_token") => Err(Error::Expired),
        Some("invalid_grant") => Err(Error::Expired),
        _ => Err(Error::Refused(
            body["error_description"]
                .as_str()
                .unwrap_or("Sign-in stopped. Try again.")
                .to_string(),
        )),
    }
}

/// Poll until the person approves or denies, or the code expires.
pub async fn wait(origin: &str, started: &Started) -> Result<Saved, Error> {
    let http = client()?;
    let deadline = tokio::time::Instant::now() + Duration::from_secs(started.expires_in.max(1));
    let mut interval = started.interval.clamp(1, 60);
    loop {
        tokio::time::sleep(Duration::from_secs(interval)).await;
        if tokio::time::Instant::now() >= deadline {
            return Err(Error::Expired);
        }
        match poll_with(&http, origin, started).await {
            Ok(Poll::Signed(saved)) => return Ok(saved),
            Ok(Poll::Pending) => {}
            Ok(Poll::SlowDown(next)) => interval = next.clamp(interval, 60),
            // A dropped connection is not the end of a sign-in.
            Err(Error::Unreachable(_)) => {}
            Err(error) => return Err(error),
        }
    }
}

/// End the saved token on its site. A token the site no longer knows is
/// already signed out.
pub async fn sign_out(saved: &Saved) -> Result<(), Error> {
    let http = client()?;
    let (status, body) = post(
        &http,
        &saved.origin,
        "/device/sign-out",
        json!({}),
        Some(&saved.token),
    )
    .await?;
    if status == 200 {
        Ok(())
    } else {
        Err(Error::Refused(
            body["error_description"]
                .as_str()
                .unwrap_or("The site didn't sign this computer out.")
                .to_string(),
        ))
    }
}

/// Open `url` in this computer's browser, when it has one. Over SSH with
/// no display, there is none to open: answers false.
#[must_use]
pub fn open_browser(url: &str) -> bool {
    if !url.starts_with("https://") && !url.starts_with("http://") {
        return false;
    }
    let remote =
        std::env::var_os("SSH_CONNECTION").is_some() || std::env::var_os("SSH_TTY").is_some();
    let program = if cfg!(target_os = "macos") {
        if remote {
            return false;
        }
        "open"
    } else if cfg!(target_os = "windows") {
        return false;
    } else {
        if std::env::var_os("DISPLAY").is_none() && std::env::var_os("WAYLAND_DISPLAY").is_none() {
            return false;
        }
        "xdg-open"
    };
    std::process::Command::new(program)
        .arg(url)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .is_ok_and(|status| status.success())
}

fn unix_now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

#[cfg(test)]
mod tests;
