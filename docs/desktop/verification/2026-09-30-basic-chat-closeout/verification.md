# Basic desktop chat: closeout

Issue: [#9996](https://github.com/OpenAgentsInc/openagents/issues/9996), the
basic-chat umbrella under tracker
[#10003](https://github.com/OpenAgentsInc/openagents/issues/10003). The slices
landed as #10002 and #10004 through #10019, with follow-ups #10020 through
#10028. Exact Zeron visual fidelity is tracked separately in #10029 and is not
part of this record.

This record maps each acceptance item to the commit, test, or verification
record that shows it, and adds the checks rerun on the closeout code:
`083303221c` on `main` (September 30, 2026, Apple M5 Max, pinned Rust 1.97.1).

## Acceptance and evidence

| # | Acceptance item | Evidence | Status |
| --- | --- | --- | --- |
| 1 | A fresh install opens a new chat with an editable composer and no terminal or model-key setup. | `11cc80897e` (#10006), [persisted chat](../2026-09-30-persisted-chat/verification.md). Test `shell::tests::fresh_store_opens_an_editable_chat_without_setup`, rerun below. | Met |
| 2 | A real hosted reply streams; a follow-up keeps context; chats, titles, and completed messages survive restart. | `b8aa0ef52a` (#10007), [hosted chat](../2026-09-30-hosted-chat/verification.md); `11cc80897e` and `6bede2ea46` (#10012) for restart. The live fixture was rerun on the closeout code (below). Restart tests: `cache::encrypted_cache_survives_restart_and_refuses_wrong_key_and_swapped_entries` and `service::management_survives_restart_and_list_pages_bind_to_one_revision` in `openagents-chat`, plus the scratch-host restart in `coder-host --test control`. | Met |
| 3 | Switching chats during a reply never puts text in another chat. Stop and retry reflect real backend outcomes and never silently duplicate sends. | [Hosted chat](../2026-09-30-hosted-chat/verification.md): the live run switched chats mid-stream. Tests `service::independent_streams_stop_retry_and_late_results_stay_bound_to_their_chat`, `service::sends_are_exactly_bound_across_relaunch_and_stop_keeps_the_partial`, `session::late_acknowledgment_and_archive_stay_bound_to_their_conversation`, and `shell::tests::pending_send_acknowledgements_do_not_clear_another_conversations_draft`. Stop says that the hosted worker may still finish, because NIP-CJ has no remote cancellation. Retry reinvokes the reply without appending a second user turn. | Met |
| 4 | Unicode, selection, undo/redo, multiline paste, and IME work. Stale view updates never replace a composing draft or submit under an old token. | `57e34331ba` ([foundation](../2026-09-29-basic-chat/foundation.md)), `4a90278909` (#10004, [composer](../2026-09-30-composer/verification.md)), `e9ef828c54` (#10028, [phone contracts](../2026-09-30-phone-contracts/verification.md)). Tests include `edit::tests::an_ime_composition_replaces_the_selection_and_undoes_as_one_edit`, `multiline_paste_and_following_typing_are_separate_undo_steps`, `a_keyboard_deletion_never_splits_a_grapheme`, `composer::tests::stale_sequences_and_view_revisions_never_change_the_draft`, `changing_the_token_starts_a_new_draft_and_a_new_undo_history`, and `field::delayed_paste_cannot_replace_a_newer_edit`. Scripted Japanese preedit and commit pass. The system Japanese input source was not exercised; that is an owner check. | Met in code; owner check listed |
| 5 | Markdown, links, selection and copy, attachment controls, working and error states, supported router cards, and transcript scrolling work in scripted fixtures and live checks. | `f5e8802b25` (#10005, [transcript](../2026-09-30-transcript/verification.md)), `43ac875c90` (#10010, [rich text](../2026-09-30-rich-text/verification.md)), `d56d97d606` (#10011, [images](../2026-09-30-images/verification.md), including the real macOS file picker), `20689a18f3` (#10009, [chat cards](../2026-09-30-chat-cards/verification.md)), `db0a0468ed` ([chat controls](../2026-09-30-chat-controls/verification.md)), `a6df77a112` (#10016, [task chat](../2026-09-30-task-chat/verification.md) for queue, question, and approval controls), and `5f5e7dbe9b` (#10015, [chat handoff](../2026-09-30-chat-handoff/verification.md)). The images slice shows that hosted image sends are refused with an explicit reason, as the scope requires. Captures of today's live run: [streaming](live-streaming.png) and [completed](live-completed.png). | Met |
| 6 | A 3,300-row transcript and a large chat list keep bounded view trees and visible-row painting. Record frame-time percentiles, idle and streaming CPU, memory, and 1x/2x behavior at default and minimum sizes, with and without the Verse backdrop. | The full eight-case matrix is in the [transcript record](../2026-09-30-transcript/verification.md) (`f5e8802b25`) and was **rerun on the closeout code** (below; [summary](matrix-summary.json)). The 512-chat list paints fewer than 20 rows and fewer than 300 foreground operations ([chat list](../2026-09-30-chat-list/verification.md)). Single-case repeats appear in the chat-cards, rich-text, images, chat-list, and chat-controls records. | Met on macOS; Linux hardware is an owner check |
| 7 | Targeted formatting, strict Clippy, and Rust tests pass on the pinned toolchain for the changed crates and phone consumers. Existing pairing regression checks pass. | Rerun below. | Met |
| 8 | Integrate on current main and deploy the desktop build from that commit. Keep the running commit and a verification record. Put owner-only checks in `NEEDS_OWNER.md`. | Code is on `main`. The release artifacts were built locally from the closeout code (below). Publishing to users is held for an explicit release decision. | Release publish pending |

Scope items outside the acceptance list:

- **Shared with the phones.** The shared state lives in `openagents-chat` and
  `openagents-chat-app` (`72123d8364`, `81abef60b3`, #10008;
  [extraction](../2026-09-30-shared-chat/extraction.md)). The shared application
  crate has no wallet, Breez, or SQLite dependency, which settles the phone
  workspace boundary.
- **Pairing.** Pairing stays reachable, and its codes are cancelled when the
  person leaves the pairing screen. Tests: `codes::tests::a_pairing_cancels_every_other_code`
  and `shell::tests::leaving_pairing_for_a_chat_cancels_the_code`.
- **Host-owned credentials.** Chat runs over the host's same-user control
  channel, and no signing key enters window state
  ([persisted chat](../2026-09-30-persisted-chat/verification.md)).
- **Zeron attribution.** Each slice reimplements Zeron's design in Rust Native
  and names Zeron. No Zeron code, engine, CRDT sync, or harness is copied.
- **Retained painter.** No GPU foreground rewrite is justified. Every scroll p99
  is below 8.3 ms.

## Native matrix on the closeout code

This run used `scripts/benchmark-desktop-chat.py` against an optimized
`openagents-desktop` built from `083303221c`. The fixture has 3,300 transcript
rows and 500 sidebar chats. Default size is 1,200 × 840 points; minimum size is
760 × 540 points. The 1x and 2x cases are effective densities on the Mac's 2x
display. Each active phase has 115 samples. A frame's time is the scripted
application update plus CPU frame submission. It does not include GPU
completion or scanout. CPU is process CPU as a percentage of one core. Memory is
the process's peak RSS across all phases. Idle lasts five seconds with a
1,000-line draft. Verse is the real renderer with a refused loopback relay.

| Case | Scroll p50 / p99 ms | Stream p50 / p99 ms | Sidebar p50 / p99 ms | Idle CPU | Stream CPU | Peak RSS MiB |
| --- | --- | --- | --- | --- | --- | --- |
| default-1x-plain | 1.27 / 1.85 | 1.77 / 3.23 | 1.84 / 2.19 | 0.44% | 17.85% | 156.7 |
| default-1x-verse | 2.76 / 4.11 | 4.07 / 4.43 | 3.00 / 3.21 | 2.79% | 41.73% | 178.4 |
| default-2x-plain | 3.80 / 4.50 | 3.16 / 3.68 | 1.74 / 2.05 | 0.42% | 25.15% | 185.9 |
| default-2x-verse | 2.71 / 3.48 | 3.57 / 4.10 | 2.32 / 4.92 | 4.11% | 31.57% | 203.3 |
| minimum-1x-plain | 1.93 / 2.18 | 3.81 / 4.74 | 1.97 / 3.34 | 0.32% | 26.22% | 157.8 |
| minimum-1x-verse | 2.03 / 2.15 | 3.76 / 4.31 | 2.82 / 2.96 | 3.00% | 30.05% | 174.8 |
| minimum-2x-plain | 2.41 / 2.56 | 4.08 / 4.45 | 2.68 / 2.77 | 0.38% | 27.78% | 168.4 |
| minimum-2x-verse | 3.11 / 3.29 | 2.56 / 4.07 | 1.79 / 2.02 | 2.98% | 24.69% | 183.1 |

All eight scroll cases pass the 8.3 ms target, and every phase is below 5 ms at
p99. Compared with the first matrix (`f5e8802b25`), streaming p99 fell from
5.8–7.5 ms to 3.2–4.7 ms, even with cards, rich text, images, and chat
management added since. Verse adds about 2.4–3.7 points of idle CPU and 15–22
MiB of RSS. [`matrix-summary.json`](matrix-summary.json) also has the warm,
composer, command-palette, and chat-menu phases. Linux hardware timings remain
an owner check.

## Checks rerun on the closeout code

All checks used pinned Rust 1.97.1 and a scratch target directory:

| Command | Result |
| --- | --- |
| `cargo fmt --check -p openagents-desktop -p openagents-chat-app -p openagents-chat -p rust-native-desktop -p rust-native` | pass |
| `cargo fmt --check --manifest-path crates/openagents-mobile/Cargo.toml -p openagents-mobile` | pass |
| `cargo clippy --locked -p openagents-desktop -p openagents-chat-app -p openagents-chat -p rust-native-desktop -p rust-native --all-targets -- -D warnings` | pass |
| `cargo clippy --locked --manifest-path crates/openagents-mobile/Cargo.toml -p openagents-mobile --all-targets -- -D warnings` | pass |
| `cargo test --locked -p openagents-desktop` (library, binary, `version_lockstep`) | 229 passed, 5 opt-in ignored |
| `cargo test --locked -p openagents-chat -p openagents-chat-app` | 175 passed, 2 opt-in ignored |
| `cargo test --locked -p rust-native-desktop -p rust-native --features rust-native/ffi,rust-native/shaping` | 192 passed, 4 ignored; the CoreText corpus check passes |
| `cargo test --locked --manifest-path crates/openagents-mobile/Cargo.toml` (phone consumer) | 148 passed, 18 opt-in ignored |
| Pairing regression: `cargo test --locked -p coder-host --test control --test iroh --test nearby --test threads` | 30 passed |
| Pairing regression: `cargo test --locked -p coder-computers --test connect` (scanned-code pairing) | 4 passed |
| `git diff --check` | pass |

The live hosted fixture ran on the same code. It makes two public hosted
inference calls with a fresh random identity, a temporary encrypted store, and
one synthetic question:

```sh
OPENAGENTS_CHAT_CAPTURE_DIR=… cargo test --locked -p openagents-desktop \
  --bin openagents-desktop live_hosted_reply_reaches_desktop_painter -- --ignored --nocapture
```

Result: first words in 5.96 s, the complete reply in 8.29 s, and three
different streaming states painted. The contextual follow-up completed in
4.33 s. The test passed. No owner host, keychain, home directory, or saved chat
was used.

## Release build (not published)

The Mac release process is `scripts/desktop/package-macos.sh`, documented in
[`docs/desktop/release.md`](../../release.md). It was run from the closeout code
(`083303221c`) with `--no-notarize`. That option skips the Apple notarization
round trip and creates nothing users can download:

```sh
CARGO_TARGET_DIR=… scripts/desktop/package-macos.sh --no-notarize --out …/desktop-release
```

Result: a universal (`x86_64 arm64`) `OpenAgents.app` and `OpenAgents-1.0.0.dmg`,
both signed with `Developer ID Application: OpenAgents, Inc. (HQWSG26L43)`
with the hardened runtime and a timestamp. `codesign` reports
`valid on disk` and that the app satisfies its Designated Requirement.
`CFBundleVersion` is 28657. The `.dmg` SHA-256 is
`fc7defdd2ba2b95c22e4a796db526197f8b4f993c7ca7a88f64d165cca5b14d4`. The build
took 24 minutes. These local artifacts are not notarized, so Gatekeeper refuses
them on another Mac. They have not been uploaded.

**Updater status.** The signed Mac manifest at
`desktop/macos/manifest.json` already names **1.0.0** (published
2026-09-30T03:08:59Z), and so does the Linux manifest. `main` is also at
1.0.0 in lockstep (`MARKETING_VERSION`, the desktop crate, and the Mac
`Info.plist`). The updater's `decide` treats an equal version as up to date.
`sign-manifest.sh` refuses to sign a version that is not newer than the
published one (`the published manifest is already 1.0.0; sign a newer
version`). A 1.0.0 build from this commit therefore cannot reach installed
apps through the updater. Shipping it requires a lockstep version bump on
`main` first (`MARKETING_VERSION` in `bins/openagents-ios/host/project.yml`,
`crates/openagents-desktop/Cargo.toml`, `bins/openagents-desktop-macos/Info.plist`,
and Android's fallback `versionName`). Then run the documented steps on the
release Mac:

```sh
scripts/desktop/package-macos.sh --notary-env ~/path/to/appstoreconnect.env
scripts/desktop/sign-manifest.sh --version $v --app target/desktop-release/OpenAgents.app --upload
gcloud storage cp target/desktop-release/OpenAgents-$v.dmg \
  gs://openagentsgemini-oa-updates/desktop/macos/$v/OpenAgents-$v.dmg
```

The `sign-manifest.sh --upload` step pushes the build to installed users. Each
installed app checks the manifest at launch and every six hours, then offers
**Restart to Update**. Publishing is held for an explicit release decision. It
is the only part of acceptance item 8 that is still open.

## Owner checks

These are in the workspace `NEEDS_OWNER.md`, under "Check typing and pasting
in the Mac app's chat, and time it on Linux (#9996)":

- The macOS system Japanese input source in the desktop composer.
- Pasting an image copied from another Mac app into the composer.
- The native chat matrix on real Linux hardware.

Earlier entries still cover the second-Mac `.dmg` open and the first update
through the app.
