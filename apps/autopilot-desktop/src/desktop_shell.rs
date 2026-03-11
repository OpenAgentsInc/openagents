pub const DEV_MODE_ENV: &str = "OPENAGENTS_ENABLE_DEV_MODE";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DesktopShellMode {
    Production,
    Dev,
}

impl DesktopShellMode {
    pub fn from_env() -> Self {
        if dev_mode_enabled_from_env() {
            Self::Dev
        } else {
            Self::Production
        }
    }

    pub const fn is_dev(self) -> bool {
        matches!(self, Self::Dev)
    }
}

pub fn dev_mode_enabled_from_env() -> bool {
    std::env::var(DEV_MODE_ENV)
        .ok()
        .map(|value| {
            matches!(
                value.trim().to_ascii_lowercase().as_str(),
                "1" | "true" | "yes" | "on"
            )
        })
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::{DesktopShellMode, dev_mode_enabled_from_env};

    #[test]
    fn dev_mode_defaults_off() {
        assert!(
            !dev_mode_enabled_from_env(),
            "tests should keep dev mode disabled by default"
        );
        assert_eq!(DesktopShellMode::from_env(), DesktopShellMode::Production);
    }
}
