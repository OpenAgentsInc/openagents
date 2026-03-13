pub const BUY_MODE_ENV: &str = "OPENAGENTS_ENABLE_BUY_MODE";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DesktopShellMode {
    Production,
}

impl DesktopShellMode {
    pub const fn from_env() -> Self {
        Self::Production
    }
}

pub fn buy_mode_enabled_from_env() -> bool {
    std::env::var(BUY_MODE_ENV)
        .ok()
        .map(|value| {
            !matches!(
                value.trim().to_ascii_lowercase().as_str(),
                "0" | "false" | "no" | "off"
            )
        })
        .unwrap_or(true)
}

#[cfg(test)]
mod tests {
    use super::{DesktopShellMode, buy_mode_enabled_from_env};

    #[test]
    fn unified_shell_defaults_without_env_gate() {
        assert!(
            buy_mode_enabled_from_env(),
            "buy mode should be visible by default"
        );
        assert_eq!(DesktopShellMode::from_env(), DesktopShellMode::Production);
    }
}
