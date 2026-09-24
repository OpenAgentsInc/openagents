//! Decision configuration shared by conversation and program callers.
//!
//! A profile names where a judgment goes; the judgment itself does not
//! change with the door. [`crate::profiles`] resolves the active one —
//! a flag, the environment, or the default, in that order — and this
//! module turns it into the one client every decision call site asks
//! through: the turn's routing, the shell judge, program selection, and
//! each `decide` step's question set. Profiles select the existing Rust
//! SDK transport; they do not configure generation or executors, and
//! they add no inference implementation of their own.
//!
//! Nothing here opens a socket, and nothing here substitutes one
//! setting for another: a missing setting, a malformed value, and a
//! profile no System One client can carry all reach the caller as the
//! refusal the resolver named, never a quieter door.

use crate::profiles::{Profile, Profiles};

/// The variables whose presence means a decision door was configured:
/// the profile's own settings and the `TYPESAFE_*` set the hosted
/// profile still reads. The shared `OPENAGENTS_*` variables and the
/// relay door's `CODER_*` pair are not among them — another tool on the
/// machine may own those, and their presence alone configures nothing
/// here.
const CONFIGURED: &[&str] = &[
    crate::profiles::PROFILE_VAR,
    crate::profiles::URL_VAR,
    crate::profiles::MODEL_VAR,
    crate::profiles::KEY_VAR,
    jev::env::API_KEY,
    jev::env::BASE_URL,
    jev::env::DEFAULT_MODEL,
];

/// Resolve the active profile into the door the call sites share,
/// without hiding invalid configuration.
///
/// An environment that names no decision variable at all answers
/// `None` — ordinary chat without a classifier, the same absence it has
/// always meant. One that names any resolves through
/// [`Profiles::resolve_env`] and builds through [`Profile::client`],
/// and either stage's refusal is the caller's error rather than a
/// fallback.
///
/// # Errors
///
/// Returns the [`Refusal`] text for a configuration that resolved
/// badly: a required setting no source supplied, a value the profile
/// cannot use, a name it does not know, a setting it does not take, or
/// a profile no System One client carries.
pub fn from_env() -> Result<Option<jev::Client>, String> {
    profile_from_env()?
        .map(|profile| profile.client().map_err(|refusal| refusal.to_string()))
        .transpose()
}

/// Resolve the active profile without building its door — the same
/// environment check [`from_env`] runs, answered as the profile itself.
/// A call site that binds behavior to the profile's identity — which
/// disclosures stay on this machine, which reach the network — reads
/// the resolved profile rather than re-deriving locality from a URL.
///
/// # Errors
///
/// Returns the [`Refusal`] text for a configuration that resolved
/// badly, exactly as [`from_env`] does.
pub fn profile_from_env() -> Result<Option<Profile>, String> {
    // A variable holding bytes that are not Unicode counts as set: it
    // is malformed configuration, and resolution is what says so.
    let configured = CONFIGURED.iter().any(|name| match std::env::var(name) {
        Ok(value) => !value.trim().is_empty(),
        Err(std::env::VarError::NotPresent) => false,
        Err(std::env::VarError::NotUnicode(_)) => true,
    });
    if !configured {
        // The Jev key Coder One, the Gym, and the harness read. Without
        // this, a machine whose key lives only in `~/.openagents/jev.json`
        // ran every Coder Terminal turn unrouted.
        let Some(key) = jev_file_key() else {
            return Ok(None);
        };
        return Profiles::new()
            .resolve(|name| {
                if name == jev::env::API_KEY {
                    Some(key.clone())
                } else {
                    std::env::var(name).ok()
                }
            })
            .map(Some)
            .map_err(|refusal| refusal.to_string());
    }
    Profiles::new()
        .resolve_env()
        .map(Some)
        .map_err(|refusal| refusal.to_string())
}

/// The TypeSafe key in `~/.openagents/jev.json` (`{"api_key": "…"}`), when
/// the file exists and holds one.
fn jev_file_key() -> Option<String> {
    let home = std::env::var_os("HOME")?;
    let path = std::path::Path::new(&home).join(".openagents/jev.json");
    let text = std::fs::read_to_string(path).ok()?;
    let value: serde_json::Value = serde_json::from_str(&text).ok()?;
    value
        .get("api_key")
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|key| !key.is_empty())
        .map(str::to_string)
}

/// `read` decides what the environment says, so a test fixes it.
/// Nothing configured is no door rather than an error, and a configured
/// one resolves and builds through the same path the process
/// environment takes.
#[cfg(test)]
fn resolve(read: impl Fn(&str) -> Option<String>) -> Result<Option<jev::Client>, String> {
    let configured = CONFIGURED
        .iter()
        .any(|name| read(name).is_some_and(|value| !value.trim().is_empty()));
    if !configured {
        return Ok(None);
    }
    door(Profiles::new().resolve(read))
}

/// The resolved profile's door — or its refusal, either way as the
/// string the call sites carry.
#[cfg(test)]
fn door(
    resolved: Result<Profile, crate::profiles::Refusal>,
) -> Result<Option<jev::Client>, String> {
    resolved
        .and_then(|profile| profile.client())
        .map(Some)
        .map_err(|refusal| refusal.to_string())
}

#[cfg(test)]
mod tests {
    use super::resolve;

    /// A valid worker public key for the relay profile: the secp256k1
    /// generator's x coordinate.
    const WORKER: &str = "79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798";

    fn profile(values: &[(&str, &str)]) -> Result<Option<jev::Client>, String> {
        resolve(|name| {
            values
                .iter()
                .find(|(key, _)| *key == name)
                .map(|(_, value)| (*value).into())
        })
    }

    /// An environment that names no decision variable leaves ordinary
    /// chat without a classifier; one that names a malformed or
    /// incomplete one is an error, not an absence.
    #[test]
    fn absent_configuration_is_distinct_from_invalid_configuration() {
        assert!(profile(&[]).unwrap().is_none());
        // A stray decision setting engages resolution, and the hosted
        // profile names the credential it still needs.
        assert!(profile(&[("CODER_DECISION_URL", "http://127.0.0.1:1")]).is_err());
        // A legacy endpoint without its key is an error, as before.
        assert!(profile(&[("TYPESAFE_BASE_URL", "http://127.0.0.1:1")]).is_err());
        // A name the resolver does not know is refused, not defaulted.
        assert!(profile(&[("CODER_DECISION_PROFILE", "hosted-ish")]).is_err());
    }

    /// Every profile kind that speaks the System One HTTP door builds
    /// the one client the call sites share; the relay kind resolves
    /// cleanly and refuses the HTTP door it does not use.
    #[test]
    fn each_profile_kind_builds_its_door() {
        for (values, url) in [
            (
                &[
                    ("CODER_DECISION_PROFILE", "hosted_http"),
                    ("CODER_DECISION_KEY", "oak_test.secret"),
                ][..],
                jev::defaults::BASE_URL,
            ),
            (
                &[
                    ("CODER_DECISION_PROFILE", "direct_local"),
                    ("CODER_DECISION_URL", "http://127.0.0.1:1"),
                    ("CODER_DECISION_MODEL", "local-kev"),
                ][..],
                "http://127.0.0.1:1",
            ),
            (
                &[
                    ("CODER_DECISION_PROFILE", "own_provider"),
                    ("CODER_DECISION_URL", "https://decisions.example.com"),
                    ("CODER_DECISION_MODEL", "shared-kev"),
                    ("CODER_DECISION_KEY", "oak_test.secret"),
                ][..],
                "https://decisions.example.com",
            ),
        ] {
            let client = profile(values)
                .unwrap_or_else(|error| panic!("{values:?} refused: {error}"))
                .expect("a configured profile builds a door");
            assert_eq!(client.base_url(), url, "{values:?}");
        }
        match profile(&[
            ("CODER_DECISION_PROFILE", "relay"),
            ("CODER_DECISION_WORKER", WORKER),
        ]) {
            Err(error) => assert!(error.contains("relay"), "{error}"),
            Ok(client) => panic!("a relay profile built an HTTP door: {client:?}"),
        }
    }

    /// A local endpoint never invents a provider credential: a
    /// `direct_local` and a loopback `own_provider` build with no key
    /// variable anywhere in the environment.
    #[test]
    fn a_local_profile_builds_without_any_key_env() {
        for values in [
            &[
                ("CODER_DECISION_PROFILE", "direct_local"),
                ("CODER_DECISION_URL", "http://127.0.0.1:1"),
                ("CODER_DECISION_MODEL", "local-kev"),
            ][..],
            &[
                ("CODER_DECISION_PROFILE", "own_provider"),
                ("CODER_DECISION_URL", "http://127.0.0.1:1"),
                ("CODER_DECISION_MODEL", "local-kev"),
            ][..],
        ] {
            let client = profile(values)
                .unwrap_or_else(|error| panic!("{values:?} refused: {error}"))
                .expect("a local endpoint builds a door");
            assert_eq!(client.base_url(), "http://127.0.0.1:1");
            assert_eq!(client.default_model(), "local-kev");
        }
    }

    /// A keyless `own_provider` beyond loopback cannot be built — and
    /// the resolver does not lend it the hosted credential sitting in
    /// the same environment to make one anyway.
    #[test]
    fn a_keyless_remote_door_refuses_rather_than_borrowing_a_credential() {
        let error = profile(&[
            ("CODER_DECISION_PROFILE", "own_provider"),
            ("CODER_DECISION_URL", "https://decisions.example.com"),
            ("CODER_DECISION_MODEL", "shared-kev"),
            ("TYPESAFE_API_KEY", "ts-secret"),
        ])
        .expect_err("a keyless remote door is refused");
        assert!(error.contains("credential"), "{error}");
    }

    /// Malformed configuration names what was wrong rather than
    /// vanishing into a default or an absent door.
    #[test]
    fn malformed_configuration_errors_loudly() {
        for values in [
            // A remote address under the local profile.
            &[
                ("CODER_DECISION_PROFILE", "direct_local"),
                ("CODER_DECISION_URL", "https://example.invalid"),
                ("CODER_DECISION_MODEL", "local-kev"),
            ][..],
            // A credential the local profile does not take.
            &[
                ("CODER_DECISION_PROFILE", "direct_local"),
                ("CODER_DECISION_URL", "http://127.0.0.1:1"),
                ("CODER_DECISION_MODEL", "local-kev"),
                ("CODER_DECISION_KEY", "oak_test.secret"),
            ][..],
            // A legacy endpoint that is not a URL at all.
            &[
                ("TYPESAFE_API_KEY", "ts-secret"),
                ("TYPESAFE_BASE_URL", "not a url"),
            ][..],
        ] {
            assert!(profile(values).is_err(), "{values:?}");
        }
    }
}
