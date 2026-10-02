//! The background watchers line every app shows at startup: how many
//! background processes (the disk cleanup monitor, `crates/background`)
//! run on this computer, and which.

/// "1 background watcher · disk cleanup", "2 background watchers · disk
/// cleanup, logs"; `None` when none run, so nothing shows.
#[must_use]
pub fn line(names: &[String]) -> Option<String> {
    if names.is_empty() {
        return None;
    }
    let noun = if names.len() == 1 {
        "background watcher"
    } else {
        "background watchers"
    };
    Some(format!("{} {noun} · {}", names.len(), names.join(", ")))
}

#[cfg(test)]
mod tests {
    #[test]
    fn counts_and_names_the_watchers_or_says_nothing() {
        assert_eq!(super::line(&[]), None);
        assert_eq!(
            super::line(&["disk cleanup".into()]).as_deref(),
            Some("1 background watcher · disk cleanup")
        );
        assert_eq!(
            super::line(&["disk cleanup".into(), "logs".into()]).as_deref(),
            Some("2 background watchers · disk cleanup, logs")
        );
    }
}
