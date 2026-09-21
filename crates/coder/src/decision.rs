//! Decision configuration shared by conversation and program callers.
//!
//! Profiles select the existing Rust SDK transport. They do not configure
//! generation or executors, and do not add an inference implementation.

/// Resolve the decision profile without hiding invalid configuration.
/// An absent legacy key leaves ordinary chat without a classifier.
pub fn from_env() -> Result<Option<jev::Client>, String> {
    let mut values = std::collections::BTreeMap::new();
    for name in [
        "CODER_DECISION_PROFILE",
        "CODER_DECISION_URL",
        "CODER_DECISION_MODEL",
        "CODER_DECISION_KEY",
        "TYPESAFE_API_KEY",
        "TYPESAFE_BASE_URL",
        "TYPESAFE_DEFAULT_MODEL",
    ] {
        match std::env::var(name) {
            Ok(value) => {
                values.insert(name, value);
            }
            Err(std::env::VarError::NotPresent) => {}
            Err(std::env::VarError::NotUnicode(_)) => {
                return Err(format!("{name} must be valid Unicode"));
            }
        }
    }
    resolve(|name| values.get(name).cloned())
}

fn resolve(read: impl Fn(&str) -> Option<String>) -> Result<Option<jev::Client>, String> {
    let value = |name| read(name).filter(|value| !value.trim().is_empty());
    let profile = value("CODER_DECISION_PROFILE");
    let endpoint = value("CODER_DECISION_URL");
    let model = value("CODER_DECISION_MODEL");
    let key = value("CODER_DECISION_KEY");
    let configured = endpoint.is_some() || model.is_some() || key.is_some();
    let required = |value: Option<String>, name| {
        value.ok_or_else(|| format!("the decision profile requires {name}"))
    };
    let config = match profile.as_deref() {
        Some("local") => {
            if key.is_some() {
                return Err("the local decision profile does not accept CODER_DECISION_KEY".into());
            }
            jev::Config::local(
                required(endpoint, "CODER_DECISION_URL")?,
                required(model, "CODER_DECISION_MODEL")?,
            )
        }
        Some("http" | "provider") => jev::Config::new()
            .base_url(required(endpoint, "CODER_DECISION_URL")?)
            .default_model(required(model, "CODER_DECISION_MODEL")?)
            .api_key(required(key, "CODER_DECISION_KEY")?),
        Some(_) => return Err("CODER_DECISION_PROFILE must be local, http, or provider; relay profiles are not implemented".into()),
        None if configured => return Err("CODER_DECISION_PROFILE is required with CODER_DECISION_URL, CODER_DECISION_MODEL, or CODER_DECISION_KEY".into()),
        None => {
            let key = value("TYPESAFE_API_KEY");
            let endpoint = value("TYPESAFE_BASE_URL");
            let model = value("TYPESAFE_DEFAULT_MODEL");
            if key.is_none() && endpoint.is_none() && model.is_none() {
                return Ok(None);
            }
            jev::Config::new()
                .api_key(required(key, "TYPESAFE_API_KEY")?)
                .base_url(endpoint.unwrap_or_else(|| jev::defaults::BASE_URL.into()))
                .default_model(model.unwrap_or_else(|| jev::defaults::MODEL.into()))
        }
    };
    jev::Client::new(config)
        .map(Some)
        .map_err(|error| format!("decision configuration is invalid: {error}"))
}

#[cfg(test)]
mod tests {
    use super::resolve;

    fn profile(values: &[(&str, &str)]) -> Result<Option<jev::Client>, String> {
        resolve(|name| {
            values
                .iter()
                .find(|(key, _)| *key == name)
                .map(|(_, value)| (*value).into())
        })
    }

    #[test]
    fn absent_legacy_configuration_is_distinct_from_invalid_configuration() {
        assert!(profile(&[]).unwrap().is_none());
        assert!(profile(&[("TYPESAFE_BASE_URL", "http://127.0.0.1:1")]).is_err());
        assert!(profile(&[("CODER_DECISION_PROFILE", "relay")]).is_err());
        assert!(profile(&[("CODER_DECISION_URL", "http://127.0.0.1:1")]).is_err());
    }

    #[test]
    fn explicit_local_profile_needs_no_provider_key_and_refuses_remote_routes() {
        for (url, valid) in [
            ("http://127.0.0.1:1", true),
            ("https://example.invalid", false),
        ] {
            let result = profile(&[
                ("CODER_DECISION_PROFILE", "local"),
                ("CODER_DECISION_URL", url),
                ("CODER_DECISION_MODEL", "local-kev"),
            ]);
            assert_eq!(result.is_ok(), valid);
        }
        assert!(
            profile(&[
                ("CODER_DECISION_PROFILE", "http"),
                ("CODER_DECISION_URL", "http://127.0.0.1:1"),
                ("CODER_DECISION_MODEL", "shared-kev")
            ])
            .is_err()
        );
    }
}
