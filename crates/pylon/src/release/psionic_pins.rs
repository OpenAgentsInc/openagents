//! The Psionic decision server (`psionic-openai-server`) this release
//! pins per platform: the binary's sha256 and the commit it was built
//! from. `scripts/release/psionic.sh` fills these in when it publishes the
//! engine binaries (#12188); until then every platform is `None` and
//! `setup` builds or finds the server instead of downloading it.

/// One platform's pinned engine binary.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PsionicPin {
    /// `macos-arm64`, `linux-x86_64-cuda`, or `linux-x86_64`.
    pub platform: &'static str,
    /// The binary's sha256 (64 hex characters), once published.
    pub sha256: Option<&'static str>,
    /// The commit the binary was built from, once published.
    pub build_commit: Option<&'static str>,
}

/// Every platform a pylon release names.
pub const PSIONIC_PINS: &[PsionicPin] = &[
    PsionicPin {
        platform: "macos-arm64",
        sha256: None,
        build_commit: None,
    },
    PsionicPin {
        platform: "linux-x86_64-cuda",
        sha256: None,
        build_commit: None,
    },
    PsionicPin {
        platform: "linux-x86_64",
        sha256: None,
        build_commit: None,
    },
];
