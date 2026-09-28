//! Push wakes for the phone, off until the app is configured for them.
//!
//! The native shell obtains the platform token (APNs device token as
//! lowercase hexadecimal, or the FCM registration token) and passes it with
//! `push_token`. Rust registers it with the push gateway, obtains a delivery
//! capability for the relay, and publishes the device's NIP-PL lease, all
//! with the device key. The enrollment state lives in its own encrypted
//! cache directory. `push_disable` revokes the lease and asks the gateway to
//! forget the token.

use coder_computers::cache::Cache;
use push_gateway::client::{Enrollment, SyncOutcome};
use secp256k1::SecretKey;
use serde::Deserialize;

const ENROLLMENT_KEY: &str = "enrollment";

/// Where the phone's wakes come from. Absent means push is off.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PushConfig {
    /// The relay whose PL executor wakes the phone, `wss://`.
    pub relay_url: String,
    /// The push gateway's registration URL, `https://`.
    pub gateway_url: String,
    /// The application profile the relay and gateway serve.
    pub app_profile: String,
}

pub(crate) struct Push {
    config: PushConfig,
    cache: Cache,
    pub status: String,
}

impl Push {
    /// Check the configuration and open the push cache. Test launches may
    /// use loopback `ws://` and `http://` URLs.
    pub fn open(
        config: PushConfig,
        cache_dir: &std::path::Path,
        secret: &SecretKey,
        loopback_test: bool,
    ) -> Result<Self, String> {
        let secure =
            config.relay_url.starts_with("wss://") && config.gateway_url.starts_with("https://");
        let loopback = |url: &str| {
            [
                "ws://127.0.0.1",
                "ws://localhost",
                "http://127.0.0.1",
                "http://localhost",
            ]
            .iter()
            .any(|prefix| url.starts_with(prefix))
        };
        if !secure
            && !(loopback_test && loopback(&config.relay_url) && loopback(&config.gateway_url))
        {
            return Err("Push needs a wss:// relay and an https:// gateway.".into());
        }
        if config.app_profile.is_empty() || config.app_profile.len() > 512 {
            return Err("Push needs an app profile.".into());
        }
        let cache = Cache::open(&cache_dir.join("push"), secret)?;
        let status = match cache.read::<Enrollment>(ENROLLMENT_KEY) {
            Ok(Some(enrollment)) if enrollment.lease.is_some() => "Wakes on".into(),
            _ => "Wakes off".into(),
        };
        Ok(Self {
            config,
            cache,
            status,
        })
    }

    fn enrollment(&self, public_key: &str) -> Result<Enrollment, String> {
        match self.cache.read::<Enrollment>(ENROLLMENT_KEY)? {
            Some(enrollment)
                if enrollment.relay_url == self.config.relay_url
                    && enrollment.gateway_url == self.config.gateway_url
                    && enrollment.app_profile == self.config.app_profile =>
            {
                Ok(enrollment)
            }
            _ => Ok(Enrollment::new(
                &self.config.relay_url,
                &self.config.gateway_url,
                &self.config.app_profile,
                public_key,
            )),
        }
    }

    /// Register or refresh the lease for `token`. Call it at every launch
    /// and whenever the platform reports a new token; it renews only when
    /// due.
    pub fn token(
        &mut self,
        runtime: &tokio::runtime::Runtime,
        secret: &SecretKey,
        public_key: &str,
        token: &str,
        now: u64,
    ) -> Result<(), String> {
        let mut enrollment = self.enrollment(public_key)?;
        let cache = &self.cache;
        let mut save = |state: &Enrollment| cache.write(ENROLLMENT_KEY, state);
        let result = runtime.block_on(enrollment.sync(secret, token, now, &mut save));
        match result {
            Ok(SyncOutcome::Published { .. } | SyncOutcome::Current) => {
                self.status = "Wakes on".into();
                Ok(())
            }
            Err(error) => {
                self.status = "Wakes unavailable".into();
                Err(error.to_string())
            }
        }
    }

    /// Revoke the lease and forget the token at the gateway.
    pub fn disable(
        &mut self,
        runtime: &tokio::runtime::Runtime,
        secret: &SecretKey,
        public_key: &str,
        now: u64,
    ) -> Result<(), String> {
        let mut enrollment = self.enrollment(public_key)?;
        let cache = &self.cache;
        let mut save = |state: &Enrollment| cache.write(ENROLLMENT_KEY, state);
        runtime
            .block_on(enrollment.revoke(secret, now, &mut save))
            .map_err(|error| error.to_string())?;
        self.status = "Wakes off".into();
        Ok(())
    }
}
