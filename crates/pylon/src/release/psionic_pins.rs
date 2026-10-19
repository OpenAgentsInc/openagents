//! The Psionic decision server (`psionic-openai-server`) this release pins
//! per platform: where the published binary lives, its sha256 and size, and
//! the commit it was built from. `scripts/release/psionic.sh` builds each
//! platform and prints these rows after the release agent publishes them
//! under `psionic/<version>/<platform>/` in the CLI release bucket. Until a
//! platform's row is filled, `setup` says its engine is not published yet
//! and keeps looking for a `psionic-openai-server` the user built.

/// One platform's pinned engine binary.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PsionicPin {
    /// `macos-aarch64` (Metal), `linux-x86_64-cuda` (CUDA 13), or
    /// `linux-x86_64-cpu`.
    pub platform: &'static str,
    /// The published binary's https URL, once published.
    pub url: Option<&'static str>,
    /// The binary's sha256 (64 hex characters), once published.
    pub sha256: Option<&'static str>,
    /// The binary's size in bytes, once published.
    pub size: Option<u64>,
    /// The commit the binary was built from, once published.
    pub build_commit: Option<&'static str>,
}

impl PsionicPin {
    /// A platform with nothing published yet.
    const fn unpublished(platform: &'static str) -> Self {
        Self {
            platform,
            url: None,
            sha256: None,
            size: None,
            build_commit: None,
        }
    }
}

/// Every platform a pylon release names.
pub const PSIONIC_PINS: &[PsionicPin] = &[
    PsionicPin::unpublished("macos-aarch64"),
    PsionicPin::unpublished("linux-x86_64-cuda"),
    PsionicPin::unpublished("linux-x86_64-cpu"),
];
