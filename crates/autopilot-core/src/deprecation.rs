pub fn autopilot_warning() -> &'static str {
    "Deprecated: `autopilot` binary is deprecated. Use `openagents autopilot` instead."
}

pub fn autopilotd_warning() -> &'static str {
    "Deprecated: `autopilotd` binary is deprecated. Use `openagents autopilot` instead."
}

#[cfg(test)]
mod tests {
    use super::{autopilot_warning, autopilotd_warning};

    #[test]
    fn warnings_mentions_openagents() {
        let warning = autopilot_warning();
        assert!(warning.contains("Deprecated"));
        assert!(warning.contains("openagents autopilot"));
        assert!(warning.contains("autopilot`"));
    }

    #[test]
    fn autopilotd_warning_mentions_openagents() {
        let warning = autopilotd_warning();
        assert!(warning.contains("Deprecated"));
        assert!(warning.contains("openagents autopilot"));
        assert!(warning.contains("autopilotd`"));
    }
}
