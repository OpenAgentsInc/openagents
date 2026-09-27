//! Gateway configuration from the environment.
//!
//! Credentials come only from files named by `*_FILE` variables, read once
//! at startup. On Unix, a credential file that its group or other users can
//! read refuses startup. No credential appears in a log line or in `Debug`
//! output.

use std::{
    fmt,
    net::SocketAddr,
    path::{Path, PathBuf},
};

/// Production APNs endpoint.
pub const APNS_PRODUCTION: &str = "https://api.push.apple.com";
/// Development (sandbox) APNs endpoint.
pub const APNS_DEVELOPMENT: &str = "https://api.sandbox.push.apple.com";
/// FCM HTTP v1 endpoint.
pub const FCM_PRODUCTION: &str = "https://fcm.googleapis.com";

/// Secret text that never prints.
#[derive(Clone)]
pub struct Secret(String);

impl Secret {
    /// Wrap secret text.
    #[must_use]
    pub fn new(value: String) -> Self {
        Self(value)
    }

    /// The secret text.
    #[must_use]
    pub fn expose(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for Secret {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("<withheld>")
    }
}

/// The APNs sender's settings.
#[derive(Debug, Clone)]
pub struct ApnsConfig {
    /// The `app_profile` value devices register under.
    pub app_profile: String,
    /// Contents of the `.p8` signing key.
    pub key_pem: Secret,
    /// The key's 10-character key ID.
    pub key_id: String,
    /// The Apple Developer team ID.
    pub team_id: String,
    /// The app's bundle ID, sent as `apns-topic`.
    pub topic: String,
    /// `https://api.push.apple.com`, the sandbox, or a loopback test server.
    pub base_url: String,
}

/// The FCM sender's settings.
#[derive(Debug, Clone)]
pub struct FcmConfig {
    /// The `app_profile` value devices register under.
    pub app_profile: String,
    /// Contents of the service account key file.
    pub service_account_json: Secret,
    /// Overrides the key file's `project_id`.
    pub project_id: Option<String>,
    /// `https://fcm.googleapis.com` or a loopback test server.
    pub base_url: String,
}

/// Bounds on what devices and relays may ask for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Limits {
    /// Longest installation lifetime, in seconds.
    pub max_installation_seconds: u64,
    /// Longest delivery capability lifetime, in seconds.
    pub max_grant_seconds: u64,
    /// Live installations per owner key.
    pub max_installations_per_owner: usize,
    /// Deliveries per installation per hour.
    pub wakes_per_hour: u32,
    /// Ceiling on `apns-expiration` and the FCM TTL, in seconds from now.
    pub max_wake_expiration_seconds: u64,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            max_installation_seconds: 90 * 86_400,
            max_grant_seconds: 31 * 86_400,
            max_installations_per_owner: 16,
            wakes_per_hour: 120,
            max_wake_expiration_seconds: 3_600,
        }
    }
}

/// Everything the gateway needs to start.
#[derive(Debug, Clone)]
pub struct Config {
    /// Private listener for relay deliveries. Keep it on loopback or a
    /// private link.
    pub delivery_addr: SocketAddr,
    /// Listener for device registration, behind a TLS proxy.
    pub registration_addr: SocketAddr,
    /// Directory for the state file.
    pub state_dir: PathBuf,
    /// Key that seals native tokens at rest.
    pub state_key: [u8; 32],
    /// Relay signing keys allowed to deliver and to receive delegations.
    pub relay_pubkeys: Vec<String>,
    /// The APNs sender, if configured.
    pub apns: Option<ApnsConfig>,
    /// The FCM sender, if configured.
    pub fcm: Option<FcmConfig>,
    /// Bounds.
    pub limits: Limits,
}

impl Config {
    /// Read the configuration from `PUSH_GATEWAY_*` variables.
    ///
    /// # Errors
    ///
    /// Returns the first missing, malformed, or unsafe setting.
    pub fn from_env() -> Result<Self, String> {
        Self::from_lookup(|name| std::env::var(name).ok().filter(|value| !value.is_empty()))
    }

    /// Read the configuration through `lookup`.
    ///
    /// # Errors
    ///
    /// Returns the first missing, malformed, or unsafe setting.
    pub fn from_lookup(lookup: impl Fn(&str) -> Option<String>) -> Result<Self, String> {
        let required = |name: &str| lookup(name).ok_or_else(|| format!("{name} is required"));
        let address = |name: &str, default: &str| -> Result<SocketAddr, String> {
            lookup(name)
                .unwrap_or_else(|| default.to_owned())
                .parse()
                .map_err(|_| format!("{name} must be an address such as 127.0.0.1:8090"))
        };
        let number = |name: &str, default: u64| -> Result<u64, String> {
            lookup(name).map_or(Ok(default), |value| {
                value
                    .parse::<u64>()
                    .ok()
                    .filter(|value| *value > 0)
                    .ok_or_else(|| format!("{name} must be a positive integer"))
            })
        };

        let key_text = read_secret_file(
            "PUSH_GATEWAY_STATE_KEY_FILE",
            &required("PUSH_GATEWAY_STATE_KEY_FILE")?,
        )?;
        let state_key = parse_key(key_text.expose().trim())
            .ok_or("PUSH_GATEWAY_STATE_KEY_FILE must hold 64 hexadecimal characters")?;

        let relay_pubkeys = required("PUSH_GATEWAY_RELAY_PUBKEYS")?
            .split(',')
            .map(|key| key.trim().to_owned())
            .collect::<Vec<_>>();
        if relay_pubkeys.is_empty()
            || relay_pubkeys.len() > 16
            || !relay_pubkeys
                .iter()
                .all(|key| crate::wire::valid_pubkey(key))
        {
            return Err(
                "PUSH_GATEWAY_RELAY_PUBKEYS must list 1 to 16 relay signing keys as 64 lowercase hexadecimal characters"
                    .to_owned(),
            );
        }

        let apns = match lookup("PUSH_GATEWAY_APNS_APP_PROFILE") {
            None => {
                refuse_orphans(
                    &lookup,
                    "PUSH_GATEWAY_APNS_APP_PROFILE",
                    &[
                        "PUSH_GATEWAY_APNS_KEY_FILE",
                        "PUSH_GATEWAY_APNS_KEY_ID",
                        "PUSH_GATEWAY_APNS_TEAM_ID",
                        "PUSH_GATEWAY_APNS_TOPIC",
                    ],
                )?;
                None
            }
            Some(app_profile) => {
                let base_url = match (
                    lookup("PUSH_GATEWAY_APNS_URL"),
                    lookup("PUSH_GATEWAY_APNS_ENVIRONMENT").as_deref(),
                ) {
                    (Some(url), _) => url,
                    (None, None | Some("production")) => APNS_PRODUCTION.to_owned(),
                    (None, Some("development")) => APNS_DEVELOPMENT.to_owned(),
                    (None, Some(_)) => {
                        return Err(
                            "PUSH_GATEWAY_APNS_ENVIRONMENT must be production or development"
                                .to_owned(),
                        );
                    }
                };
                Some(ApnsConfig {
                    app_profile: profile(app_profile)?,
                    key_pem: read_secret_file(
                        "PUSH_GATEWAY_APNS_KEY_FILE",
                        &required("PUSH_GATEWAY_APNS_KEY_FILE")?,
                    )?,
                    key_id: identifier(
                        "PUSH_GATEWAY_APNS_KEY_ID",
                        required("PUSH_GATEWAY_APNS_KEY_ID")?,
                    )?,
                    team_id: identifier(
                        "PUSH_GATEWAY_APNS_TEAM_ID",
                        required("PUSH_GATEWAY_APNS_TEAM_ID")?,
                    )?,
                    topic: identifier(
                        "PUSH_GATEWAY_APNS_TOPIC",
                        required("PUSH_GATEWAY_APNS_TOPIC")?,
                    )?,
                    base_url,
                })
            }
        };

        let fcm = match lookup("PUSH_GATEWAY_FCM_APP_PROFILE") {
            None => {
                refuse_orphans(
                    &lookup,
                    "PUSH_GATEWAY_FCM_APP_PROFILE",
                    &[
                        "PUSH_GATEWAY_FCM_SERVICE_ACCOUNT_FILE",
                        "PUSH_GATEWAY_FCM_PROJECT_ID",
                    ],
                )?;
                None
            }
            Some(app_profile) => Some(FcmConfig {
                app_profile: profile(app_profile)?,
                service_account_json: read_secret_file(
                    "PUSH_GATEWAY_FCM_SERVICE_ACCOUNT_FILE",
                    &required("PUSH_GATEWAY_FCM_SERVICE_ACCOUNT_FILE")?,
                )?,
                project_id: lookup("PUSH_GATEWAY_FCM_PROJECT_ID"),
                base_url: lookup("PUSH_GATEWAY_FCM_URL")
                    .unwrap_or_else(|| FCM_PRODUCTION.to_owned()),
            }),
        };
        if apns.is_none() && fcm.is_none() {
            return Err(
                "configure PUSH_GATEWAY_APNS_APP_PROFILE, PUSH_GATEWAY_FCM_APP_PROFILE, or both"
                    .to_owned(),
            );
        }
        if let (Some(apns), Some(fcm)) = (&apns, &fcm)
            && apns.app_profile == fcm.app_profile
        {
            return Err("the APNs and FCM app profiles must differ".to_owned());
        }

        let limits = Limits {
            max_installation_seconds: number("PUSH_GATEWAY_MAX_INSTALLATION_SECONDS", 90 * 86_400)?,
            max_grant_seconds: number("PUSH_GATEWAY_MAX_GRANT_SECONDS", 31 * 86_400)?,
            max_installations_per_owner: 16,
            wakes_per_hour: u32::try_from(number("PUSH_GATEWAY_WAKES_PER_HOUR", 120)?)
                .map_err(|_| "PUSH_GATEWAY_WAKES_PER_HOUR is too large")?,
            max_wake_expiration_seconds: 3_600,
        };

        Ok(Self {
            delivery_addr: address("PUSH_GATEWAY_DELIVERY_ADDR", "127.0.0.1:8090")?,
            registration_addr: address("PUSH_GATEWAY_REGISTRATION_ADDR", "127.0.0.1:8091")?,
            state_dir: PathBuf::from(required("PUSH_GATEWAY_STATE_DIR")?),
            state_key,
            relay_pubkeys,
            apns,
            fcm,
            limits,
        })
    }
}

fn refuse_orphans(
    lookup: &impl Fn(&str) -> Option<String>,
    switch: &str,
    names: &[&str],
) -> Result<(), String> {
    match names.iter().find(|name| lookup(name).is_some()) {
        Some(name) => Err(format!("{name} requires {switch}")),
        None => Ok(()),
    }
}

fn profile(value: String) -> Result<String, String> {
    if value.is_empty() || value.len() > 512 || value.chars().any(char::is_control) {
        return Err("an app profile must be 1 to 512 bytes without control characters".to_owned());
    }
    Ok(value)
}

fn identifier(name: &str, value: String) -> Result<String, String> {
    if value.is_empty()
        || value.len() > 256
        || !value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'-' | b'_'))
    {
        return Err(format!(
            "{name} must be letters, digits, dots, hyphens, or underscores"
        ));
    }
    Ok(value)
}

fn parse_key(text: &str) -> Option<[u8; 32]> {
    if text.len() != 64 {
        return None;
    }
    let mut key = [0_u8; 32];
    for (index, byte) in key.iter_mut().enumerate() {
        *byte = u8::from_str_radix(text.get(index * 2..index * 2 + 2)?, 16).ok()?;
    }
    Some(key)
}

/// Read a credential file. On Unix, refuse one that its group or other
/// users can read.
///
/// # Errors
///
/// Returns a reason naming the variable, never the contents.
pub fn read_secret_file(name: &str, path: &str) -> Result<Secret, String> {
    let path = Path::new(path);
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        let metadata = std::fs::metadata(path)
            .map_err(|_| format!("{name} names a file that cannot be read"))?;
        if metadata.permissions().mode() & 0o077 != 0 {
            return Err(format!(
                "{name} names a file that its group or other users can read; set its mode to 0600 or 0400"
            ));
        }
    }
    let text = std::fs::read_to_string(path)
        .map_err(|_| format!("{name} names a file that cannot be read"))?;
    if text.len() > 65_536 {
        return Err(format!("{name} names a file larger than 64 KiB"));
    }
    Ok(Secret::new(text))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn write_private(dir: &Path, name: &str, contents: &str) -> String {
        let path = dir.join(name);
        std::fs::write(&path, contents).unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt as _;
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).unwrap();
        }
        path.to_string_lossy().into_owned()
    }

    #[test]
    fn configuration_needs_a_transport_and_private_credential_files() {
        let dir = tempfile::tempdir().unwrap();
        let key = write_private(dir.path(), "state.key", &"ab".repeat(32));
        let apns_key = write_private(dir.path(), "apns.p8", "placeholder");
        let mut env = HashMap::from([
            ("PUSH_GATEWAY_STATE_KEY_FILE", key.clone()),
            (
                "PUSH_GATEWAY_STATE_DIR",
                dir.path().to_string_lossy().into_owned(),
            ),
            ("PUSH_GATEWAY_RELAY_PUBKEYS", "a".repeat(64)),
        ]);
        let read = |env: &HashMap<&str, String>| {
            let env = env.clone();
            Config::from_lookup(move |name| env.get(name).cloned())
        };
        assert!(read(&env).unwrap_err().contains("APP_PROFILE"));
        env.insert("PUSH_GATEWAY_APNS_KEY_FILE", apns_key.clone());
        assert!(
            read(&env)
                .unwrap_err()
                .contains("requires PUSH_GATEWAY_APNS_APP_PROFILE")
        );
        env.insert(
            "PUSH_GATEWAY_APNS_APP_PROFILE",
            "com.example.app/ios".into(),
        );
        env.insert("PUSH_GATEWAY_APNS_KEY_ID", "ABC123DEFG".into());
        env.insert("PUSH_GATEWAY_APNS_TEAM_ID", "TEAM123456".into());
        env.insert("PUSH_GATEWAY_APNS_TOPIC", "com.example.app".into());
        let config = read(&env).unwrap();
        let apns = config.apns.as_ref().unwrap();
        assert_eq!(apns.base_url, APNS_PRODUCTION);
        assert!(!format!("{config:?}").contains("placeholder"));
        env.insert("PUSH_GATEWAY_APNS_ENVIRONMENT", "development".into());
        assert_eq!(read(&env).unwrap().apns.unwrap().base_url, APNS_DEVELOPMENT);
        env.insert("PUSH_GATEWAY_RELAY_PUBKEYS", "A".repeat(64));
        assert!(read(&env).is_err());
        env.insert("PUSH_GATEWAY_RELAY_PUBKEYS", "a".repeat(64));
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt as _;
            std::fs::set_permissions(&key, std::fs::Permissions::from_mode(0o644)).unwrap();
            assert!(read(&env).unwrap_err().contains("0600"));
        }
    }
}
