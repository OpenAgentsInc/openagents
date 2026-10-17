//! Session notifications on the phone (#11853): register the native push
//! token with the OpenAgents push gateway, keep a delivery grant for the
//! sync server's key current, and hand the sync server only that grant.
//!
//! The platform shell supplies the token and nothing else. Rust keeps a
//! push-only Nostr key and the installation state in `push.json` under the
//! account's data directory (mode `0600`), which sign-out removes with the
//! rest of the account's files. The gateway calls are signed with NIP-98 by
//! that key, and the native token never reaches the sync server.
use async_trait::async_trait;
use openagents_client as zc;
use push_gateway::{
    client::{ClientError as GatewayError, GatewayClient},
    wire::{self, DelegationRequest, InstallationResponse, InstallationRevokeRequest, Transport},
};
use secp256k1::SecretKey;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::path::{Path, PathBuf};

/// How long a gateway installation is asked to live.
pub const INSTALLATION_SECONDS: u64 = 60 * 86_400;
/// How long a delivery grant is asked to live (the gateway allows 31 days).
pub const GRANT_SECONDS: u64 = 30 * 86_400;
const STATE_FILE: &str = "push.json";

/// What the sync server says about push (`/registry/{org}/push-config`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Config {
    /// The gateway's registration base URL.
    pub registration: String,
    /// The sync server's delivery key, which the grant is bound to.
    pub delegate: String,
    /// The app profile for this phone's transport.
    pub profile: String,
    pub transport: Transport,
}

impl Config {
    /// Read the server's answer for a phone on `platform` (`ios`, `android`).
    pub fn parse(value: &Value, platform: &str) -> Option<Self> {
        let transport = if platform == "android" {
            Transport::Fcm
        } else {
            Transport::Apns
        };
        let profile = value["profiles"][transport.as_str()].as_str()?;
        let delegate = value["delegate"]
            .as_str()
            .filter(|d| wire::valid_pubkey(d))?;
        let registration = value["registration"].as_str()?;
        GatewayClient::new(registration).ok()?;
        Some(Self {
            registration: registration.trim_end_matches('/').to_owned(),
            delegate: delegate.to_owned(),
            profile: profile.to_owned(),
            transport,
        })
    }
}

/// The installation the gateway holds for this phone.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Installation {
    pub registration: String,
    pub profile: String,
    pub handle: String,
    pub epoch: u64,
    /// SHA-256 of the native token, to notice a new token without keeping it.
    pub token_digest: String,
    pub expires_at: u64,
}

/// The delivery grant the sync server holds.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Grant {
    pub delegate: String,
    pub epoch: u64,
    pub grant: String,
    pub expires_at: u64,
}

/// Everything this phone remembers about push, saved before each step that
/// uses a new generation so a crash never reuses one.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct State {
    /// The push-only Nostr key, as hexadecimal.
    pub secret: String,
    pub generation: u64,
    pub installation: Option<Installation>,
    pub grant: Option<Grant>,
}

impl State {
    fn fresh() -> Self {
        let secret = SecretKey::new(&mut secp256k1::rand::rng());
        Self {
            secret: secret.display_secret().to_string(),
            generation: 0,
            installation: None,
            grant: None,
        }
    }

    fn key(&self) -> Result<SecretKey, String> {
        let bytes = (0..self.secret.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(self.secret.get(i..i + 2).unwrap_or("zz"), 16))
            .collect::<Result<Vec<_>, _>>()
            .map_err(|_| "the saved push key is damaged".to_owned())?;
        let bytes: [u8; 32] = bytes
            .try_into()
            .map_err(|_| "the saved push key is damaged".to_owned())?;
        SecretKey::from_byte_array(bytes).map_err(|_| "the saved push key is damaged".to_owned())
    }

    /// Load `push.json` from `dir`, or start a new key.
    pub fn load(dir: &Path) -> Self {
        std::fs::read(dir.join(STATE_FILE))
            .ok()
            .and_then(|bytes| serde_json::from_slice::<Self>(&bytes).ok())
            .filter(|state| state.key().is_ok())
            .unwrap_or_else(Self::fresh)
    }

    /// Save to `dir` with owner-only permissions, replacing atomically.
    pub fn save(&self, dir: &Path) -> Result<(), String> {
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
        let temp = dir.join(format!("{STATE_FILE}.tmp"));
        let bytes = serde_json::to_vec(self).map_err(|e| e.to_string())?;
        {
            use std::io::Write as _;
            let mut options = std::fs::OpenOptions::new();
            options.write(true).create(true).truncate(true);
            #[cfg(unix)]
            std::os::unix::fs::OpenOptionsExt::mode(&mut options, 0o600);
            let mut file = options.open(&temp).map_err(|e| e.to_string())?;
            file.write_all(&bytes).map_err(|e| e.to_string())?;
            file.sync_all().map_err(|e| e.to_string())?;
        }
        std::fs::rename(&temp, dir.join(STATE_FILE)).map_err(|e| e.to_string())
    }

    /// Forget everything (sign-out or notifications turned off).
    pub fn remove(dir: &Path) {
        let _ = std::fs::remove_file(dir.join(STATE_FILE));
    }
}

/// The gateway's registration routes, as a trait so tests can stand in.
#[async_trait]
pub trait Registrar: Send + Sync {
    async fn register(
        &self,
        secret: &SecretKey,
        profile: &str,
        token: &str,
        expires_at: u64,
    ) -> Result<InstallationResponse, GatewayError>;
    async fn rotate(
        &self,
        secret: &SecretKey,
        handle: &str,
        epoch: u64,
        token: &str,
    ) -> Result<u64, GatewayError>;
    async fn delegate(
        &self,
        secret: &SecretKey,
        request: &DelegationRequest,
    ) -> Result<String, GatewayError>;
}

#[async_trait]
impl Registrar for GatewayClient {
    async fn register(
        &self,
        secret: &SecretKey,
        profile: &str,
        token: &str,
        expires_at: u64,
    ) -> Result<InstallationResponse, GatewayError> {
        GatewayClient::register(self, secret, profile, token, expires_at).await
    }
    async fn rotate(
        &self,
        secret: &SecretKey,
        handle: &str,
        epoch: u64,
        token: &str,
    ) -> Result<u64, GatewayError> {
        GatewayClient::rotate(self, secret, handle, epoch, token).await
    }
    async fn delegate(
        &self,
        secret: &SecretKey,
        request: &DelegationRequest,
    ) -> Result<String, GatewayError> {
        GatewayClient::delegate(self, secret, request).await
    }
}

/// Make sure the gateway holds `token` and a grant for the sync server's
/// key is current, renewing it in the last third of its life. Returns the
/// grant to hand the sync server. `save` runs after every change.
pub async fn enroll(
    state: &mut State,
    config: &Config,
    token: &str,
    now: u64,
    gateway: &dyn Registrar,
    save: &mut (dyn FnMut(&State) -> Result<(), String> + Send),
) -> Result<Grant, String> {
    if !config.transport.valid_token(token) {
        return Err("the push token is not valid".into());
    }
    let secret = state.key()?;
    let digest = push_gateway::sha256_hex(token.as_bytes());
    let same_place = |held: &Installation| {
        held.registration == config.registration && held.profile == config.profile
    };
    match state.installation.clone() {
        Some(held) if same_place(&held) && held.expires_at > now + GRANT_SECONDS => {
            if held.token_digest != digest {
                let epoch = gateway
                    .rotate(&secret, &held.handle, held.epoch, token)
                    .await
                    .map_err(|e| e.to_string())?;
                state.installation = Some(Installation {
                    epoch,
                    token_digest: digest,
                    ..held
                });
                state.grant = None;
                save(state)?;
            }
        }
        _ => {
            let made = gateway
                .register(&secret, &config.profile, token, now + INSTALLATION_SECONDS)
                .await
                .map_err(|e| e.to_string())?;
            state.installation = Some(Installation {
                registration: config.registration.clone(),
                profile: config.profile.clone(),
                handle: made.installation_handle,
                epoch: made.endpoint_epoch,
                token_digest: digest,
                expires_at: made.expires_at,
            });
            state.grant = None;
            save(state)?;
        }
    }
    let held = state
        .installation
        .clone()
        .ok_or_else(|| "no installation".to_owned())?;
    if let Some(grant) = &state.grant {
        if grant.delegate == config.delegate
            && grant.epoch == held.epoch
            && grant.expires_at > now + GRANT_SECONDS / 3
        {
            return Ok(grant.clone());
        }
    }
    state.generation += 1;
    save(state)?;
    let expires_at = (now + GRANT_SECONDS).min(held.expires_at);
    let grant = gateway
        .delegate(
            &secret,
            &DelegationRequest {
                v: wire::WIRE_VERSION,
                installation_handle: held.handle.clone(),
                endpoint_epoch: held.epoch,
                generation: state.generation,
                relay_pubkey: config.delegate.clone(),
                not_before: now.saturating_sub(60),
                expires_at,
            },
        )
        .await
        .map_err(|e| e.to_string())?;
    let grant = Grant {
        delegate: config.delegate.clone(),
        epoch: held.epoch,
        grant,
        expires_at,
    };
    state.grant = Some(grant.clone());
    save(state)?;
    Ok(grant)
}

fn state_dir(client: &zc::Client) -> PathBuf {
    client.config().data_dir.clone()
}

fn failed(message: String) -> zc::ClientError {
    zc::ClientError::HostError(message)
}

/// Register for session notifications with this phone's native token.
/// Nothing happens when the server has push off.
pub async fn register(
    client: &zc::Client,
    token: &str,
    prefs: zc::PushPrefs,
) -> Result<(), zc::ClientError> {
    let Some(value) = client.push_config().await? else {
        return Ok(());
    };
    let config = Config::parse(&value, &client.config().platform)
        .ok_or_else(|| failed("push setup is not available for this phone".into()))?;
    let dir = state_dir(client);
    let mut state = State::load(&dir);
    let gateway = GatewayClient::new(&config.registration).map_err(|e| failed(e.to_string()))?;
    let grant = enroll(
        &mut state,
        &config,
        token,
        push_gateway::unix_now(),
        &gateway,
        &mut |state| state.save(&dir),
    )
    .await
    .map_err(failed)?;
    client
        .store_push_grant(
            config.transport.as_str(),
            &grant.grant,
            grant.expires_at,
            prefs,
        )
        .await
}

/// Stop session notifications: the sync server forgets the grant, and the
/// gateway forgets the token.
pub async fn unregister(client: &zc::Client) -> Result<(), zc::ClientError> {
    let result = client.remove_push_grant().await;
    let dir = state_dir(client);
    let state = State::load(&dir);
    if let (Some(held), Ok(secret)) = (&state.installation, state.key()) {
        if let Ok(gateway) = GatewayClient::new(&held.registration) {
            let _ = gateway
                .revoke_installation(
                    &secret,
                    &InstallationRevokeRequest {
                        v: wire::WIRE_VERSION,
                        installation_handle: held.handle.clone(),
                        endpoint_epoch: held.epoch,
                    },
                )
                .await;
        }
    }
    State::remove(&dir);
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::sync::Mutex;

    const NOW: u64 = 1_800_000_000;
    const TOKEN: &str = "abcdef0123456789";

    #[derive(Default)]
    struct Fake {
        calls: Mutex<Vec<String>>,
    }
    #[async_trait]
    impl Registrar for Fake {
        async fn register(
            &self,
            _: &SecretKey,
            profile: &str,
            token: &str,
            expires_at: u64,
        ) -> Result<InstallationResponse, GatewayError> {
            self.calls
                .lock()
                .unwrap()
                .push(format!("register {profile} {token}"));
            Ok(InstallationResponse {
                installation_handle: "0f0e0d0c-0b0a-4908-8706-050403020100".into(),
                endpoint_epoch: 1,
                expires_at,
            })
        }
        async fn rotate(
            &self,
            _: &SecretKey,
            _: &str,
            epoch: u64,
            token: &str,
        ) -> Result<u64, GatewayError> {
            self.calls.lock().unwrap().push(format!("rotate {token}"));
            Ok(epoch + 1)
        }
        async fn delegate(
            &self,
            _: &SecretKey,
            request: &DelegationRequest,
        ) -> Result<String, GatewayError> {
            self.calls.lock().unwrap().push(format!(
                "delegate {} {} {}",
                request.relay_pubkey, request.endpoint_epoch, request.generation
            ));
            Ok(format!("pg1_{}", request.generation))
        }
    }

    fn config() -> Config {
        Config::parse(
            &json!({"configured":true,"registration":"https://push.example.com","delegate":"b".repeat(64),
                "profiles":{"apns":"app.ios","fcm":"app.android"}}),
            "ios",
        )
        .unwrap()
    }

    async fn run(state: &mut State, gateway: &Fake, token: &str, now: u64) -> Grant {
        enroll(state, &config(), token, now, gateway, &mut |_| Ok(()))
            .await
            .unwrap()
    }

    #[test]
    fn the_server_answer_picks_this_phones_transport() {
        let value = json!({"configured":true,"registration":"https://push.example.com","delegate":"b".repeat(64),
            "profiles":{"apns":"app.ios","fcm":null}});
        assert_eq!(config().transport, Transport::Apns);
        assert_eq!(config().profile, "app.ios");
        assert!(Config::parse(&value, "android").is_none());
        let mut bad = value.clone();
        bad["delegate"] = json!("not a key");
        assert!(Config::parse(&bad, "ios").is_none());
        bad = value;
        bad["registration"] = json!("ftp://push.example.com");
        assert!(Config::parse(&bad, "ios").is_none());
    }

    #[tokio::test]
    async fn registers_once_then_reuses_the_grant_until_renewal() {
        let gateway = Fake::default();
        let mut state = State::fresh();
        let first = run(&mut state, &gateway, TOKEN, NOW).await;
        assert_eq!(first.grant, "pg1_1");
        assert_eq!(first.delegate, "b".repeat(64));
        assert_eq!(
            *gateway.calls.lock().unwrap(),
            [
                format!("register app.ios {TOKEN}"),
                format!("delegate {} 1 1", "b".repeat(64))
            ]
        );
        // The next launch reuses the grant: no gateway calls.
        let again = run(&mut state, &gateway, TOKEN, NOW + 3_600).await;
        assert_eq!(again, first);
        assert_eq!(gateway.calls.lock().unwrap().len(), 2);
        // In the last third of the grant's life, a higher generation renews it.
        let renewed = run(&mut state, &gateway, TOKEN, NOW + GRANT_SECONDS * 2 / 3 + 1).await;
        assert_eq!(renewed.grant, "pg1_2");
        assert_eq!(gateway.calls.lock().unwrap().len(), 3);
    }

    #[tokio::test]
    async fn a_new_token_rotates_and_needs_a_new_grant() {
        let gateway = Fake::default();
        let mut state = State::fresh();
        run(&mut state, &gateway, TOKEN, NOW).await;
        let rotated = run(&mut state, &gateway, "00ff00ff", NOW + 10).await;
        assert_eq!(rotated.epoch, 2);
        assert_eq!(rotated.grant, "pg1_2");
        let calls = gateway.calls.lock().unwrap().clone();
        assert_eq!(calls[2], "rotate 00ff00ff");
        assert!(calls[3].ends_with(" 2 2"), "{calls:?}");
        // No token text is kept, only its digest.
        let saved = serde_json::to_string(&state).unwrap();
        assert!(!saved.contains("00ff00ff"));
    }

    #[tokio::test]
    async fn a_bad_token_is_refused_before_any_call() {
        let gateway = Fake::default();
        let mut state = State::fresh();
        let result = enroll(&mut state, &config(), "NOT-HEX", NOW, &gateway, &mut |_| {
            Ok(())
        })
        .await;
        assert!(result.is_err());
        assert!(gateway.calls.lock().unwrap().is_empty());
    }

    #[test]
    fn the_saved_state_is_private_and_survives_a_reload() {
        let dir = tempfile::tempdir().unwrap();
        let mut state = State::load(dir.path());
        state.generation = 4;
        state.save(dir.path()).unwrap();
        let loaded = State::load(dir.path());
        assert_eq!(loaded, state);
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = std::fs::metadata(dir.path().join(STATE_FILE))
                .unwrap()
                .permissions()
                .mode();
            assert_eq!(mode & 0o777, 0o600);
        }
        State::remove(dir.path());
        assert_ne!(State::load(dir.path()).secret, state.secret);
    }
}
