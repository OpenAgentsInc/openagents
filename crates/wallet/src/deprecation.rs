pub fn legacy_warning() -> &'static str {
    "Deprecated: `wallet` binary is deprecated. Use `openagents wallet` instead."
}

#[cfg(test)]
mod tests {
    use super::legacy_warning;

    #[test]
    fn warning_mentions_openagents() {
        let warning = legacy_warning();
        assert!(warning.contains("Deprecated"));
        assert!(warning.contains("openagents wallet"));
        assert!(warning.contains("wallet`"));
    }
}
