//! The capability contract: what a manifest may claim, and what a host may
//! do with one.
//!
//! A capability is something a host can hand work to — another agent's CLI
//! on the same computer, a cloud lane, a subprocess with a protocol.
//! [NIP-CAP](../../../nips/openagents/NIP-CAP.md) defines the manifest that
//! describes one: the transport, the argv that reports a version, the argv
//! that asks about a workspace, the bounds the adapter claims to keep, and
//! the bounds it will silently ignore. A host reads the same body from a
//! file until a relay serves it.
//!
//! # Discovery is inert
//!
//! [`Registry::open`] reads and validates every manifest in the searched
//! directories and runs nothing. A manifest is untrusted input: it names
//! an executable and its arguments, and a manifest that reached `exec` on
//! the strength of being read would let any repository run any interpreter
//! the host carries. Probing is the second step, not a side effect of the
//! first.
//!
//! # Trust is the host's, and it names bytes
//!
//! An executable probe runs only when [`Trust`] holds a record approving
//! the manifest — a record written by `capability-trust approve`, or the
//! operator's store edited by hand. The record pins the manifest by
//! digest, the adapter by canonical path and content digest, and every
//! file an argv word names by the word, its canonical path, and its
//! digest — a decision re-resolves each word where the argv would run, so
//! approving a file approves only what was approved, and a retargeted
//! link approves nothing. Where a manifest sits — an
//! operator-named directory, a repository's `capabilities/` — decides
//! which manifest a slug names. It never decides whether it may run.
//!
//! # Probes are bounded and their answers are typed
//!
//! [`Entry::probe`] runs the manifest's `detect` argv — version first,
//! workspace probe second — through the one process supervisor, under a
//! wall clock and an output cap. Every outcome is a [`Presence`]:
//! `present`, `absent`, `present and unavailable`, `unprobed`, or
//! `unknown`. A probe that timed out, exited wrong, or answered past the
//! cap is `unknown`, and `unknown` is not a route: a failed probe cannot
//! manufacture availability.
//!
//! # The operator's path
//!
//! The `capability-trust` binary is the approval path: `approve` resolves
//! a slug through the same [`search`] order a survey uses and records the
//! pinned identities, `list` shows the store, `revoke` removes a slug's
//! records. Nothing else approves: there is no unconditional trust keyed
//! on a slug or a binary name.
//!
//! # A claim is not proof
//!
//! A manifest's `enforces` list is what the adapter says it keeps. The
//! approval lets the probe run and lets a delegation ask the adapter to
//! hold a bound; it does not verify that the bound is held. Host-side
//! enforcement — deadlines, concurrency, isolation — is proven by the
//! host's own machinery and recorded apart from the claim.

pub mod bounded;
mod manifest;
mod probe;
mod registry;
mod trust;

use std::path::{Path, PathBuf};

pub use manifest::{
    Claim, Detect, Manifest, RELAY, Refusal, SUBPROCESS, WorkspaceProbe, executor_document,
};
pub use probe::{Found, Presence};
pub use registry::{Entry, Registry, Source, SourceDir, resolve, search, search_dirs};
pub use trust::{
    Approval, Decision, Pinned, Proof, Record, Trust, Verified, digest_bytes, digest_file,
    store_path,
};

/// The event kind a manifest publishes as, per NIP-CAP.
pub const MANIFEST_KIND: u16 = 30180;

/// The manifest body version this reads. A host refuses a `v` it does not
/// know rather than reading the fields it recognizes.
pub const MANIFEST_VERSION: u32 = 1;

/// The name a probe records itself under in a trace.
pub const PROBE_CALL: &str = "capability_probe";

/// The variable that moves the manifest directory.
pub const DIR_ENV: &str = "CODER_CAPABILITY_DIR";

/// The variable that adds directories to the executable search, ahead of
/// everything else.
pub const PATH_ENV: &str = "CODER_CAPABILITY_PATH";

/// The variable that moves the trust store, for a host that keeps its
/// policy somewhere other than `~/.openagents`.
pub const STORE_ENV: &str = "CODER_CAPABILITY_TRUST";

/// The capabilities a host may offer, out of what a probe found.
///
/// This is the whole reason the probe answers five states. The option set
/// for a routing decision is built from what is here, so an absent
/// capability drops out of the list rather than becoming a route that
/// fails, an unprobed one stays out rather than running a manifest nobody
/// approved, and an operator without the executor is offered a shorter
/// list rather than a broken one.
#[must_use]
pub fn options(found: &[Found]) -> Vec<&Found> {
    found.iter().filter(|one| one.available()).collect()
}

/// Approves probing the capability `slug` names, at the manifest it
/// resolves to right now.
///
/// The approval names the manifest body by digest, so a file that changes
/// afterwards approves nothing; it pins the adapter by canonical path and
/// content digest, so a replaced binary approves nothing; and it pins
/// every file the manifest's argvs name by word, path, and digest, so a
/// script that changes — or a link that is retargeted — under an
/// unchanged manifest approves nothing either.
///
/// `writable` lists adapter state a filesystem boundary may let the
/// executor write — the operator's word that those paths belong to the
/// adapter. Each must be absolute and must exist to be approved: the
/// canonical path is what the record pins.
///
/// # Errors
///
/// Returns why the approval was not recorded: the slug is not a slug, no
/// registry holds it, the adapter does not resolve, a `writable` path is
/// relative or does not resolve, the trust store sits inside the
/// repository it would approve, or the store could not be read or
/// written.
pub fn approve(
    repository: Option<&Path>,
    slug: &str,
    writable: &[PathBuf],
) -> Result<Approval, String> {
    Trust::load(&store_path())?.approve(repository, slug, writable)
}

/// The first token of `line` that reads as a dotted version.
///
/// `devin 3000.10.31 (b98cc431)` gives `3000.10.31`: the commit is not a
/// version and a probe that reported it as one would compare two machines
/// by their build hashes.
#[must_use]
pub fn version_in(line: &str) -> Option<String> {
    line.split_whitespace().find_map(|token| {
        let token = token.trim_start_matches(['v', 'V']);
        let core = token.trim_matches(|c: char| !c.is_ascii_digit());
        let dotted = core.contains('.')
            && core.starts_with(|c: char| c.is_ascii_digit())
            && core.chars().all(|c| c.is_ascii_digit() || c == '.');
        dotted.then(|| core.to_string())
    })
}

/// The first line with anything on it, trimmed.
pub(crate) fn first_line(text: &str) -> &str {
    text.lines()
        .find(|line| !line.trim().is_empty())
        .unwrap_or("")
        .trim()
}

/// Whether `slug` matches the NIP-CAP slug grammar.
#[must_use]
pub fn is_slug(slug: &str) -> bool {
    let mut chars = slug.chars();
    let first = chars
        .next()
        .is_some_and(|c| c.is_ascii_lowercase() || c.is_ascii_digit());
    first
        && slug.len() <= 64
        && slug
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_' || c == '-')
}

/// Whether two argv name the same binary, by file name.
///
/// A manifest's `detect.binary` is what the probe resolves, and every argv
/// the manifest carries starts with its own name. The check is by file
/// name so an absolute `detect.binary` and a bare argv head still agree.
pub(crate) fn same_binary(binary: &str, argv0: &str) -> bool {
    Path::new(binary).file_name() == Path::new(argv0).file_name()
        && !binary.is_empty()
        && !argv0.is_empty()
}
