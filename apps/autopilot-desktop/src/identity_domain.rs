use std::path::PathBuf;

use anyhow::Result;
use pylon::PylonConfig;
use runtime::UnifiedIdentity;

pub(crate) fn load_pylon_config() -> Result<PylonConfig> {
    let mut config = PylonConfig::load()?;
    if config.default_model.trim().is_empty() {
        config.default_model = "llama3.2".to_string();
    }
    Ok(config)
}

pub(crate) fn identity_path_for_config(config: &PylonConfig) -> Result<PathBuf> {
    Ok(config.data_path()?.join("identity.mnemonic"))
}

pub(crate) fn pylon_identity_exists(config: &PylonConfig) -> bool {
    identity_path_for_config(config)
        .map(|path| path.exists())
        .unwrap_or(false)
}

pub(crate) fn load_or_init_identity(config: &PylonConfig) -> Result<UnifiedIdentity> {
    let identity_path = identity_path_for_config(config)?;
    if identity_path.exists() {
        let mnemonic = std::fs::read_to_string(&identity_path)?.trim().to_string();
        return UnifiedIdentity::from_mnemonic(&mnemonic, "")
            .map_err(|err| anyhow::anyhow!("Failed to load identity: {err}"));
    }

    if let Some(parent) = identity_path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let identity = UnifiedIdentity::generate()
        .map_err(|err| anyhow::anyhow!("Failed to generate identity: {err}"))?;
    std::fs::write(&identity_path, identity.mnemonic())?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&identity_path, std::fs::Permissions::from_mode(0o600))?;
    }

    Ok(identity)
}
