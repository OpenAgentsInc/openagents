# Basic chat: composer editing foundation

Status: the first implementation checkpoint for
[#9996](https://github.com/OpenAgentsInc/openagents/issues/9996). Full basic
desktop chat remains in progress. This checkpoint has no rendered composer,
transcript painter, hosted conversation integration, or desktop deployment.

The worktree starts at remote `main`, `90fa6eddaa22866c1f0d2e035e08f1702ae09a63`,
on branch `issue-9996-desktop-chat`. The public Zeron reference was fetched at
`ed3b1aae4a5189eef67143db7b8c5c3ee7a933c5`; its `ComposerInput` is the source
design, reimplemented here without vendored source or GPUI dependencies.

## Implemented behavior

- `rust_native::edit::Editor`: bounded UTF-8 drafts, directional selection,
  grapheme and word movement, logical line movement, multiline paste,
  coalesced undo, redo, checked native UTF-16 conversion, and atomic refusals.
- IME composition retains the text and selection from before preedit. Updates
  replace the marked range, commit takes one undo step, and cancellation
  restores the earlier state. Ordinary edits refuse while composition owns
  the draft.
- `rust_native_desktop::composer::ComposerDraft`: retained editing state across
  semantic rerenders, new lifetimes on token changes or remounts, and callback
  checks against the view revision and edit sequence.
- Sends check the current composer, enabled/busy state, composition, empty
  messages, byte limits, and that a choice belongs to this composer. Stop
  resolves through the existing typed activation. Accepted submissions clear
  only their own editing sequence, preserving text entered after submission.
- Both lockfiles add the existing `unicode-segmentation` 1.13.3 dependency
  edge to Rust Native; no package version changes. Debug output omits drafts.

## Checks

Checked on macOS, Apple silicon, with the pinned Rust 1.97.1 toolchain. The
worktree uses its own `target` directory; the phone dependency graph uses
`target/phone-core`.

```sh
cargo fmt --check -p rust-native -p rust-native-desktop
cargo clippy --locked -p rust-native -p rust-native-desktop \
  --all-targets --target-dir "$PWD/target" -- -D warnings
cargo test --locked -p rust-native -p rust-native-desktop --all-targets \
  --features rust-native/ffi --target-dir "$PWD/target"
cargo check --locked -p openagents-desktop --all-targets \
  --target-dir "$PWD/target"
cargo test --locked --manifest-path crates/openagents-mobile/Cargo.toml \
  -p rust-native edit:: --target-dir "$PWD/target/phone-core"
git diff --check
```

Formatting and strict Clippy pass. The combined test run has 119 passing
tests: 87 in Rust Native and 32 in the desktop adapter, including 32 new
editing and composer tests. Three existing layout benchmark/corpus-generation
tests are ignored. The desktop app's all-target compilation passes. All
19 editor tests also pass under the phone dependency graph. The phone command
selects the shared dependency; it does not build or test the phone application.

## Existing failures outside this checkpoint

The unfiltered shared-core test run under the phone manifest has 86 passing
tests, one failure, and three ignored tests. The existing
`line_breaks_match_coretext_for_the_bundled_fonts` fails before line comparison:
fixture corpus digest `a993b0190687602a` differs from the serialized corpus
digest `1f4d7f19f6172e12`. The same focused test reproduces that exact failure
on the unchanged original checkout at `482d96d687`, using its separate
`target/issue-9996-baseline-core`. The fixture and test are unchanged between
that commit and this worktree's remote-main base. No fixture is regenerated.

`./scripts/check-dependencies.sh` exits 15. All four policy categories fail
on existing dependencies: Git sources in the wallet graph, license refusals
including `bip21`, `hex_lit`, `musig2`, and `webpki-roots`, wildcard dependencies
in `verse-ruins`, and advisories for `cgmath` and `wasmtime`. Their versions and
sources remain those of remote main. The only dependency change is the new
edge to the already locked Unicode segmentation package.

## Next implementation work

Connect native keyboard, IME, pointer selection, clipboard, focus, caret
painting, and application submission to the desktop controller. Implement
the desktop transcript painter over the existing immutable layout frames and
measure visible-row painting. Then extract and bind shared chat state and
the host-backed hosted-chat transport, followed by the remaining Zeron basic
chat components. The issue stays open until its full chat acceptance passes.

## Shared chat checkpoint

The hosted phone chat implementation now lives in `crates/openagents-chat`.
The phone re-exports the same transport, router, and lifecycle, with injected
wake callbacks; `coder-computers` re-exports the same cache. Storage paths and
wire fields remain compatible. The shared crate avoids the mobile wallet
workspace and gives the desktop host one implementation to call.

`cargo test --offline -p openagents-chat` passed 21 tests, with the live test
ignored. This includes a local NIP-42 relay, authenticated and encrypted
answers, connection reuse, ordered streaming, refusal, retry, and encrypted
relaunch. The mobile consumer passed `cargo check --offline --manifest-path
crates/openagents-mobile/Cargo.toml`.
