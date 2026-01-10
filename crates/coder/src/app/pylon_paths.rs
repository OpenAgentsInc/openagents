use std::path::{Path, PathBuf};

pub(crate) fn pylon_data_dir() -> Option<PathBuf> {
    let home = dirs::home_dir()?;
    let pylon_dir = home.join(".openagents").join("pylon");
    let config_path = pylon_dir.join("config.toml");
    let data_dir_fallback = pylon_dir.clone();

    if !config_path.exists() {
        return Some(data_dir_fallback);
    }

    if let Ok(contents) = std::fs::read_to_string(&config_path) {
        if let Ok(value) = toml::from_str::<toml::Value>(&contents) {
            return value
                .get("data_dir")
                .and_then(|v| v.as_str())
                .map(|value| expand_home_path(value, &home))
                .or(Some(data_dir_fallback));
        }
    }

    Some(data_dir_fallback)
}

fn expand_home_path(value: &str, home: &Path) -> PathBuf {
    let trimmed = value.trim();
    if trimmed == "~" {
        return home.to_path_buf();
    }
    if let Some(stripped) = trimmed.strip_prefix("~/") {
        return home.join(stripped);
    }
    PathBuf::from(trimmed)
}
