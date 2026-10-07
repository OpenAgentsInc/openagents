//! Jev for the CLI's judged commands: the background rules and Everglade's
//! rumor proposals. It stands on no Unix piece, so Windows builds it too.

/// Jev through Coder's decision door (`coder::decision::from_env`), else
/// the shared resolver every Jev caller uses (`jev_hosted::resolve`): the
/// person's own keys, the TypeSafe key `openagents settings provider-key
/// set typesafe` kept (keychain or file), else OpenAgents' hosted
/// decision service, which needs no key. A fresh install works without
/// anyone editing a settings file (#10310).
pub(crate) struct JevJudge {
    client: jev::Client,
}

/// Why Jev is unreachable here, with the one command that fixes it.
pub(crate) const NO_JEV: &str = "Jev, the judge background rules ask, can't be reached from this \
     computer. Check the connection, or keep your own TypeSafe key with `openagents settings \
     provider-key set typesafe`.";

impl JevJudge {
    /// The resolved client, for another caller that asks Jev the same way.
    pub(crate) fn client(&self) -> jev::Client {
        self.client.clone()
    }

    pub(crate) fn from_env() -> Option<Self> {
        // The same door the rest of Coder decides through: the configured
        // decision profile, `TYPESAFE_API_KEY`, or `~/.openagents/jev.json`.
        if let Some(client) = coder::decision::from_env().ok().flatten() {
            return Some(Self { client });
        }
        let stored = crate::provider_key::stored()
            .get(model_access::Provider::TypeSafe)
            .map(|key| key.expose().to_owned());
        let env = |name: &str| {
            if name == jev::env::API_KEY {
                std::env::var(name)
                    .ok()
                    .filter(|v| !v.trim().is_empty())
                    .or_else(|| stored.clone())
            } else {
                std::env::var(name).ok()
            }
        };
        let dir = jev_hosted::openagents_dir()?;
        let model = jev::defaults::MODEL.to_string();
        let resolved = jev_hosted::resolve(
            &env,
            &dir,
            &jev_hosted::Door {
                url: jev_hosted::DOOR,
                model: &model,
            },
            &|config| config,
        )
        .ok()?;
        Some(Self {
            client: resolved.client,
        })
    }
}
