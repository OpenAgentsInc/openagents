//! Everglade, split out of `crates/verse` so an edit to it recompiles this
//! crate and what depends on it rather than all of Verse: the zone with its
//! city, studio, spells, hotbar, wildlife, and demolition yard
//! (`zones::everglade`), and its pinned pack with the compiler that builds
//! it (`zones::everglade_pack`). `verse` re-exports both modules under their
//! old paths. Read `docs/verse/README.md` and `docs/verse/zones.md`.

pub mod zones;

// Meteor Swarm and the sledgehammer in Everglade are a local test of
// destruction, never a production feature: a browser or phone build cannot
// carry them, so no page, setting, or console call can turn them on there.
#[cfg(all(
    feature = "dev-destruction",
    any(target_arch = "wasm32", target_os = "ios", target_os = "android")
))]
compile_error!(
    "the dev-destruction feature is for local desktop builds only; the web and phone builds must not enable it"
);

// The paths these zones were written against inside `crates/verse`.
use verse_core::{avatar, fx, tooltip, world};
use verse_gfx::{palette, ui};
use verse_pbr::{mesh, pbr};
use verse_world::social::controller;

/// The runtime names these zones use.
mod runtime {
    pub use verse_core::zone::InteractHint;
}

/// The content compiler's exports, as `verse::imported` names them.
mod imported {
    pub use verse_content::compiler::{characters, icons, inventory};
}

/// The scene label, as `verse::doors` names it.
mod doors {
    pub use verse_core::label::label as scene_label;
}

#[cfg(test)]
mod production_tests {
    /// Reads `path` under the workspace root.
    fn read(path: &str) -> String {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        std::fs::read_to_string(root.join(path)).unwrap_or_else(|e| panic!("{path}: {e}"))
    }

    /// The web build, the phone apps, and the release scripts never enable
    /// `dev-destruction`: only Verse's own opt-in feature names it.
    #[test]
    fn production_builds_never_enable_dev_destruction() {
        let verse = read("crates/verse/Cargo.toml");
        for line in verse.lines() {
            if line.contains("dev-destruction") && !line.trim_start().starts_with('#') {
                assert!(
                    line.starts_with("dev-destruction = "),
                    "only Verse's opt-in feature may name it: {line}"
                );
            }
        }
        for path in [
            "crates/everglade-web/Cargo.toml",
            "crates/coder-mobile/Cargo.toml",
            "crates/openagents-mobile/Cargo.toml",
            "crates/openagents-web/Dockerfile",
            "scripts/release/testflight.sh",
            "scripts/release/terminal.sh",
        ] {
            assert!(!read(path).contains("dev-destruction"), "{path}");
        }
        // The web build asks for no features and checks for this one.
        let web = read("scripts/build-everglade-web.sh");
        assert!(!web.contains("--features") && !web.contains("--all-features"));
        assert!(web.contains("grep -q 'dev-destruction'"));
    }
}
