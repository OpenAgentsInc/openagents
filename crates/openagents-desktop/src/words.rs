//! Words the desktop app never shows.
//!
//! The wireframe's "Words on screen" bans jargon on primary surfaces, and
//! the auto-pairing spec adds the desktop screens' own list: no key, host,
//! relay, grant, workspace, tailnet, Tailscale, npub, or nsec. "Project"
//! names a workspace and "phone" names a device.

/// What the screens call the computer they run on: "Mac" on a Mac, and
/// "computer" on Linux and Windows, where "Mac" would be wrong.
pub const COMPUTER: &str = if cfg!(target_os = "macos") {
    "Mac"
} else {
    "computer"
};

/// Banned on every desktop screen, compared whole-word and ignoring case
/// and a plural `s`.
pub const BANNED: &[&str] = &[
    "npub",
    "nsec",
    "key",
    "relay",
    "nostr",
    "nip",
    "atif",
    "tailnet",
    "tailscale",
    "wasm",
    "plugin",
    "extension",
    "tool",
    "benchmark",
    "terminal-bench",
    "tb",
    "eval",
    "evaluation",
    "suite",
    "case",
    "grader",
    "rubric",
    "judge",
    "baseline",
    "arm",
    "harness",
    "stand-in",
    "mock",
    "pilot",
    "jev",
    "luna",
    "microcoder",
    "verifier",
    "trace",
    "recipe",
    "grant",
    "sats",
    "btc",
    "₿",
    "lightning",
    "invoice",
    "host",
    "workspace",
    "pubkey",
    "hex",
    "iroh",
    "invitation",
    "endpoint",
    "socket",
    "daemon",
];

/// The banned words in `text`.
pub fn banned_in(text: &str) -> Vec<String> {
    let mut found = Vec::new();
    for word in text
        .split(|c: char| !(c.is_alphanumeric() || c == '-' || c == '₿'))
        .filter(|word| !word.is_empty())
    {
        let lower = word.to_lowercase();
        let singular = lower.strip_suffix('s').unwrap_or(&lower);
        if BANNED.contains(&lower.as_str()) || BANNED.contains(&singular) {
            found.push(word.to_string());
        }
    }
    found
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn banned_words_are_found_whole_and_plural() {
        assert_eq!(banned_in("Copy the host keys"), vec!["host", "keys"]);
        assert!(banned_in("Starting Coder on this Mac…").is_empty());
        assert!(banned_in("Starting Coder on this computer…").is_empty());
        // Whole words only: "monkey" is not "key".
        assert!(banned_in("A monkey hosted nothing").is_empty());
    }
}
