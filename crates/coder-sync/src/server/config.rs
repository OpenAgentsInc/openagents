use std::{collections::HashMap, net::SocketAddr};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AuthMode {
    Disabled,
    Dev,
    OpenAgents,
}

pub struct Config {
    pub listen: SocketAddr,
    pub database_url: String,
    pub bucket: String,
    pub auth: AuthMode,
    pub account_deployment: openagents::Deployment,
    pub revision: String,
    pub push: Option<super::push::Settings>,
    pub proof_origin: String,
    /// `CODER_SYNC_ROOM_TOKEN_KEYS`: the website's room-token public keys,
    /// comma-separated lowercase hex (#12263).
    pub room_keys: Vec<String>,
}

impl Config {
    pub fn from_env() -> Result<Self, &'static str> {
        Self::parse(
            std::env::vars().collect(),
            std::env::args().any(|a| a == "--allow-dev-auth"),
        )
    }

    pub fn parse(vars: HashMap<String, String>, allow_dev: bool) -> Result<Self, &'static str> {
        let get = |key: &str| vars.get(key).map(String::as_str);
        // A missing deployment environment is production-safe too.
        let local = matches!(get("CODER_SYNC_ENV"), Some("local" | "staging"));
        let auth = match get("CODER_SYNC_AUTH").unwrap_or("openagents") {
            "disabled" => AuthMode::Disabled,
            "openagents" => AuthMode::OpenAgents,
            "dev" if local && allow_dev => AuthMode::Dev,
            "dev" => {
                return Err(
                    "Dev authentication requires a local or staging environment and --allow-dev-auth.",
                );
            }
            _ => return Err("Unsupported authentication mode."),
        };
        let database_url = get("CODER_SYNC_DATABASE_URL")
            .filter(|s| !s.is_empty())
            .ok_or("Set CODER_SYNC_DATABASE_URL.")?
            .to_owned();
        let bucket = get("CODER_SYNC_BUCKET")
            .filter(|s| !s.is_empty())
            .ok_or("Set CODER_SYNC_BUCKET.")?
            .to_owned();
        let listen = get("CODER_SYNC_LISTEN")
            .map(str::to_owned)
            .unwrap_or_else(|| format!("0.0.0.0:{}", get("PORT").unwrap_or("8080")))
            .parse()
            .map_err(|_| "Invalid listen address.")?;
        let proof_origin = get("CODER_SYNC_PUBLIC_ORIGIN")
            .unwrap_or("https://sync.openagents.com")
            .to_owned();
        let origin = reqwest::Url::parse(&proof_origin).map_err(|_| "Invalid public origin.")?;
        if !matches!(origin.scheme(), "http" | "https")
            || origin.host().is_none()
            || origin.path() != "/"
            || origin.query().is_some()
            || origin.fragment().is_some()
            || !origin.username().is_empty()
            || origin.password().is_some()
        {
            return Err("Invalid public origin.");
        }
        let room_keys: Vec<String> = get("CODER_SYNC_ROOM_TOKEN_KEYS")
            .unwrap_or_default()
            .split(',')
            .map(str::trim)
            .filter(|key| !key.is_empty())
            .map(str::to_owned)
            .collect();
        if room_keys.iter().any(|key| {
            key.len() != 64
                || !key
                    .bytes()
                    .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        }) {
            return Err("Invalid room token key.");
        }
        Ok(Self {
            room_keys,
            proof_origin,
            listen,
            database_url,
            bucket,
            auth,
            account_deployment: openagents::Deployment::new(
                get("CODER_SYNC_ACCOUNT_API").unwrap_or("https://api.openagents.com/v1"),
                get("CODER_SYNC_ACCOUNT_WEBSITE").unwrap_or("https://openagents.com"),
            )
            .map_err(|_| "Invalid account service configuration.")?,
            revision: get("CODER_SYNC_REVISION")
                .unwrap_or("development")
                .to_owned(),
            push: super::push::Settings::parse(&|key| get(key).map(str::to_owned))?,
        })
    }
}
