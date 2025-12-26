pub fn legacy_warning() -> &'static str {
    "Deprecated: `marketplace` binary is deprecated. Use `openagents marketplace` instead."
}

#[cfg(test)]
mod tests {
    use super::legacy_warning;

    #[test]
    fn warning_mentions_openagents() {
        let warning = legacy_warning();
        assert!(warning.contains("Deprecated"));
        assert!(warning.contains("openagents marketplace"));
        assert!(warning.contains("marketplace`"));
    }
}
