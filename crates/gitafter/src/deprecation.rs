pub fn legacy_warning() -> &'static str {
    "Deprecated: `gitafter` binary is deprecated. Use `openagents gitafter` instead."
}

#[cfg(test)]
mod tests {
    use super::legacy_warning;

    #[test]
    fn warning_mentions_openagents() {
        let warning = legacy_warning();
        assert!(warning.contains("Deprecated"));
        assert!(warning.contains("openagents gitafter"));
        assert!(warning.contains("gitafter`"));
    }
}
