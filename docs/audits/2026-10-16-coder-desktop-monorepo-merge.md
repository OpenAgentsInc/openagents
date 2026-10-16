# Merging the Coder Desktop fork into the monorepo

Status: assessment, 2026-10-16. Owner request: move all relevant code from
the Zeron fork ([OpenAgentsInc/coder-desktop](https://github.com/OpenAgentsInc/coder-desktop))
into this repository, accepting a mixed tech stack, and ship the result as
**the OpenAgents desktop and mobile apps** instead of separate "Coder Desktop"
and "OpenAgents Coder" products. No work has started; this document estimates
what it would take and recommends an order.

Sources: fork `origin/main` `57c13158` (23 commits on top of upstream
`3b2f7802`, Zeron v0.2.113 + 2), this repository's `origin/main`, the fork's
`FORK.md` and `docs/downstream-patches.md`, and the closed issues
#11824–#11832, #11840–#11851 and #11855.

## Summary

It is feasible and mostly mechanical for the first step, then real product
work after that:

1. **Import (days).** Bring the fork in with its history as its own Cargo
   workspace inside this repository, the way `crates/psionic` and
   `crates/openagents-mobile` already live here. That sidesteps the two hard
   build conflicts (below), the toolchain gap and the fork's build profiles.
   Leave `edge/` (TypeScript) and the marketing sites behind: `coder-sync`
   already replaces the edge (#11842–#11847).
2. **One identity (about a week).** Rename the apps to OpenAgents on the
   existing OpenAgents bundle IDs, migrate local data, and move release
   scripts out of GitHub Actions.
3. **One app per platform (weeks).** The fork's desktop is the stronger base
   (GPUI, about 330,000 lines with its engine), so OpenAgents desktop
   features (hosted chat, Verse, Gym, pairing, wallet) move into it and the
   current `openagents-desktop` retires after parity. On the phone, the fork's
   iOS app takes over from `bins/openagents-ios` and `bins/coder-ios`
   after it gains Chat, Wallet and Account. Android needs a decision.
4. **Converge (months, incremental).** Remove duplicate systems (agent
   harness drivers, sync, Markdown/syntax, terminal) and fold the separate
   workspace into the root one once its dependency conflicts are resolved.

The two decisions that shape everything: **whether we keep pulling upstream
Zeron** (it ships about 6 commits a day), and **which phone-to-desktop
channel survives** (the fork's Loro sync through `coder-sync`, or this
repository's pairing and NIP-HOST path).

## What is in the fork

| Area | Path | Size | Fate |
| --- | --- | --- | --- |
| Desktop UI (GPUI) | `crates/ui` | 231,000 lines Rust (709 files) | Import |
| Engine: sessions, workspaces, uploads, RPC, MCP | `crates/engine`, `rpc`, `mcp` | 106,000 | Import |
| Agent harness drivers (Claude, Codex, Cursor, OpenCode, Pi, ACP: Devin, Grok, Hermes, Antigravity) | `crates/harness` | 54,000 | Import; converge later with `coder-sdk` |
| Phone core (UniFFI layout engine) | `crates/mobile` | 13,600 | Import |
| Viewer client, sync (Loro), documents, protocol | `crates/client`, `sync`, `doc`, `proto` | 43,600 | Import |
| Text, Markdown, syntax, theme | `crates/text`, `markdown`, `syntax`, `theme` | 16,300 | Import; converge later |
| Visuals (Luau runtime, wgpu), previews | `crates/visual`, `visual-gpu`, `preview` | 14,500 | Import |
| Voice (orb, veil, voice, audio) | `crates/voice*`, `orb`, `veil`, `audio` | 6,800 | Import |
| Updater | `crates/update` | 2,600 | Import, then point at our release channel |
| OpenAgents account (SDK consumer) | `crates/coder-account` | 3,000 | Import; switch the Git pin to a path dependency |
| Identity constants | `crates/brand` | 115 | Import; becomes the rename switch |
| Desktop binary | `apps/zeron` | 1,600 | Import |
| iOS app (UIKit) | `apps/ios` | 34,000 lines Swift, 1,069 files with assets | Import (language exception, below) |
| Cloudflare edge | `edge/` | 7,500 lines TypeScript | **Drop**: replaced by `crates/coder-sync` |
| Landing and redirect sites | `apps/landing`, `apps/www-redirect` | TypeScript/static | Drop |
| GitHub workflows | `.github/workflows` (12) | | Drop; replace with local scripts |
| Docs, screenshots, media | `docs/` | 400+ files | Import selectively (design and regression docs; skip binary media) |

Totals without `edge/` and the sites: about 493,000 lines of Rust and
34,000 lines of Swift, in 27 crates. The fork's lockfile resolves 1,310
packages; this repository's resolves 1,741.

## What the monorepo already has

| Surface | Where | Size | Overlap with the fork |
| --- | --- | --- | --- |
| OpenAgents desktop (Rust Native, wgpu/winit) | `crates/openagents-desktop`, `rust-native`, `rust-native-desktop`, `bins/openagents-desktop-macos` (`com.openagents.desktop`) | 92,000 lines | Same product slot. It has hosted OpenAgents chat, local Coder, Gym cards, the Verse page, pairing and settings. The fork has multi-harness agents, workspaces, terminal, file tree, diff pane, browser, voice and visuals |
| OpenAgents iOS and Android (thin SwiftUI and Kotlin over Rust Native) | `crates/openagents-mobile`, `bins/openagents-ios` (`com.openagents.app`), `bins/openagents-android` | 35,600 lines Rust plus hosts | Same product slot. It has Chat, Wallet and Account |
| Coder iOS and Android (Verse plus read-only chats) | `crates/coder-mobile`, `bins/coder-ios`, `bins/coder-android` | 20,000 lines Rust | The fork's iOS app already took the `com.openagents.coder` App Store record (#11841, #11850) |
| Sync server for the fork's protocol | `crates/coder-sync` (`sync.openagents.com`) | 8,700 | Already here. This is why `edge/` is not imported |
| SDK the fork consumes | `crates/openagents-sdk` | | Becomes a path dependency |
| Agent host and harness transports | `crates/coder-sdk`, `claude_agent_sdk`, `coder-delegate`, `coder` host serve | | Duplicates `crates/harness` and part of `crates/engine` |

## Hard problems

### 1. Two `links` conflicts block a single Cargo workspace today

Cargo allows one package per `links` name in a dependency graph:

- **`sqlite3`:** the fork uses `rusqlite` 0.32 (`libsqlite3-sys` 0.30).
  The root workspace is held at 0.28 by `ldk-node` in `crates/wallet`. This
  is the same conflict that already keeps `crates/openagents-mobile` in its
  own workspace (its `Cargo.toml` explains it).
- **`tree-sitter`:** the fork uses 0.26.11; the root uses 0.27.0
  (`crates/briefing-lab`, `code-highlight`).

There are also non-blocking splits that would double build time if shared:
`wgpu` 29 (GPUI's renderer) against 30, `metal` 0.33 against 0.29, and
`tokio-tungstenite` 0.24 against 0.28–0.30.

**Fix for the import:** a separate workspace (with its own `Cargo.lock`).
**Fix for convergence:**
- move the fork to `rusqlite` 0.31, or move `ldk-node` forward;
- move the fork to `tree-sitter` 0.27 and matching grammars;
- move both sides to one `wgpu`, which waits on GPUI.

### 2. Toolchain and build profiles

- **Toolchain.** The root pins Rust 1.97.1. The fork's GPUI needs at least
  1.98.1 (it uses `std::hint::cold_path`). A nested `rust-toolchain.toml` in
  the imported directory handles this until the root moves up.
- **Profiles.** The fork's `[profile.dev.package."*"] opt-level = 2` and its
  `mobile` and `mobile-dist` profiles would change every monorepo build if
  merged into the root workspace. In a separate workspace they stay local.

### 3. Language policy

[AGENTS.md](../../AGENTS.md) says product code is Rust, with named
exceptions for thin Swift and Kotlin hosts.

- **iOS.** The fork's iOS app is 34,000 lines of UIKit: views, controllers,
  a transcript painter, sheets and a Live Activity. Its state and layout are
  in Rust (`crates/mobile`), but the Swift is not thin. The owner accepted a
  mixed stack, so add an explicit, named exception for this app, with the
  direction that new logic goes into `crates/mobile` and Swift stays
  presentation.
- **TypeScript.** None comes over: `edge/` and the sites stay behind.
- **Renderer rule.** The Rust Native renderer rule
  ([docs/coder-desktop/README.md](../coder-desktop/README.md) "GPUI is
  welcome") was waived only because the fork was a separate repository.
  Amend AGENTS.md so the desktop app is GPUI.

### 4. Upstream Zeron

Upstream made 95 commits in the first 15 days of October. The fork's current
rules (keep `zeron-*` crate names, never reformat upstream code, keep
identity in `crates/brand`) exist so `git merge upstream/main` stays cheap.
Inside the monorepo:

- **Keep pulling.** Import with `git subtree add --prefix=<dir>` and keep
  `git subtree pull` working: keep the directory layout and crate names
  unchanged and rename nothing outside `crates/brand`. The cost is that
  convergence work (deduplicating harnesses, sync, Markdown) creates
  conflicts on every pull.
- **Hard fork.** Import once, then take upstream changes only by hand.
  This frees renames and deduplication, but we own all future harness glue
  (new Claude, Codex and Cursor versions) ourselves. Upstream's harness glue
  was a stated reason for forking.

**Recommendation:** keep pulling through steps 1–3 while the work is
identity and features. Decide at step 4 whether to hard fork, crate by
crate: the UI and engine can follow upstream longer than sync and harness.

### 5. Two phone-to-desktop channels

- **The fork:** the phone mirrors sessions through Loro documents on
  `coder-sync` and drives the desktop through the device relay (#11843–#11849).
- **This repository:** pairing by QR code, `coder-connect`, NIP-HOST over
  Nostr, and `coder host serve` (#11854 already plans to carry the fork's
  device channel over NIP-HOST).

One app per platform needs one channel. **Recommendation:** keep
`coder-sync` for sessions (it is built, tested against the real clients,
and the phone already uses it). Carry the pairing and NIP-HOST work as the
transport underneath it per #11854, not as a second product path.

## Identity: becoming "OpenAgents"

The fork keeps every product identity value in `crates/brand`
(`coder-brand`). Renaming is mostly one file plus the platform manifests.

| | Today | Proposed |
| --- | --- | --- |
| Desktop name, bundle ID | Coder, `com.openagents.coder.desktop` | OpenAgents, `com.openagents.desktop` (the current OpenAgents desktop's ID, so it updates in place) |
| Desktop data | `~/Library/Application Support/Coder Desktop` | The OpenAgents desktop's directory, with a one-time import of Coder Desktop's local sessions and settings |
| iPhone name, bundle ID | OpenAgents Coder, `com.openagents.coder` | OpenAgents, on `com.openagents.app` (the OpenAgents iOS record). The alternative is to keep `com.openagents.coder` and retire `com.openagents.app` |
| URL scheme, IPC port | `coder-desktop://`, 27854 | `openagents://` (check the current desktop's handler), one port |
| Updater | Zeron's feed, turned off | Our release channel |
| Binary, `ZERON_*` variables, crate names | `zeron` | Unchanged while pulling upstream; rename at the hard-fork decision |

Moving the iPhone app onto `com.openagents.app` replaces the current
OpenAgents iOS build for its TestFlight testers, and App Store Connect
history stays with that record. That is an owner decision because it is
hard to reverse. Data migration also needs the Keychain service names
(`AppModel.swift` keeps them isolated today) and the account session from
`coder-account`.

## Feature reconciliation

| Feature | Fork desktop | OpenAgents desktop | Work in the merged app |
| --- | --- | --- | --- |
| Hosted OpenAgents chat (send, stream, stop) | Reads saved chats through the SDK (#11830, #11831), follows updates; does not send | Yes (`openagents-chat`, `openagents-chat-app`) | Add send, stream and stop to the SDK and the fork's reader. Largest gap |
| Local coding agents | Many harnesses, workspaces, worktrees, queue, steer | Coder via `coder-sdk` (Claude, Codex) | Keep the fork's |
| Terminal, file tree, diff pane, browser, previews | Yes | Diff pane only | Keep the fork's |
| Voice, visuals, MCP server | Yes | No | Keep the fork's |
| Verse page | No | Yes (wgpu, winit) | Embed Verse as a native child surface (the fork's GPUI already composites over native macOS views), or keep it as its own window |
| Gym and eval cards | No | Yes | Port the cards; the runner stays in shared crates |
| Wallet | No | No (phone has it) | Port from `openagents-mobile` for phone parity |
| Pairing phones and computers | Through `coder-sync` | QR pairing, NIP-HOST | See "two channels" above |
| Settings, themes | Yes (Coder Light and Noir added, #11836) | Yes | Keep the fork's; add missing OpenAgents settings |
| Windows, Linux | Upstream packages; ours unbuilt | Built (macOS, Linux, Windows) | Build and verify ours |

| Feature | Fork iOS | OpenAgents iOS and Android | Work |
| --- | --- | --- | --- |
| Live session list, transcript, composer, remote control, Live Activity | Yes | No | Keep |
| Saved OpenAgents chats | Read and follow (#11851) | Chat with send | Add send |
| Wallet, Account | Account sign-in only | Yes | Port into `crates/mobile` plus UIKit screens |
| Verse | No | Coder iOS has it; OpenAgents iOS has it off for release | Defer; reuse `coder-mobile`'s render handle later |
| Android | None (the core is designed for Kotlin) | Thin Kotlin over `openagents-mobile` | Either keep the current Android app until a Kotlin host over `crates/mobile` exists, or build that host |

## Proposed plan

| Step | Work | Estimate (agent-days) | Exit check |
| --- | --- | --- | --- |
| 0. Decisions | Upstream policy, sync channel, bundle IDs, Android, directory name | Owner, 1 day | Recorded here and in AGENTS.md |
| 1. Import | `git subtree add` the fork without `edge/`, the sites and workflows, into one directory (suggested `apps/openagents/`) as its own workspace, with a nested toolchain file; exclude it from the root workspace; switch `coder-account` to the path SDK; AGENTS.md exceptions (GPUI desktop, UIKit iOS) | 1–2 | `cargo build -p zeron`, the fork's tests and `scripts/ios/build-core.sh` pass from the monorepo; the iOS simulator build runs; the monorepo's own checks are unchanged |
| 2. Retire the fork repo | Move FORK.md, patch notes and the conformance checkpoint (`scripts/coder-sync/conformance.sh` now runs in one tree); archive `coder-desktop` read-only; repoint issue and doc links | 1 | No live references to the fork repo outside history |
| 3. Release scripts | macOS sign/notarize/package, iOS TestFlight (match `scripts/release/testflight.sh`), Windows and Linux packaging as local scripts; no GitHub workflows | 2–3 | A signed macOS build and a TestFlight validate-only run from the monorepo |
| 4. OpenAgents identity | `crates/brand` values, icons, data migration, Keychain migration, updater channel, About and licenses | 3–5 | Coder Desktop users and OpenAgents desktop users both land in the new app with their data; dedicated test accounts only |
| 5. Desktop parity | Hosted chat sending through the SDK; Gym cards; Verse surface; settings | 10–20 | Every row of the OpenAgents desktop table works in the merged app; `openagents-desktop` marked retired |
| 6. Phone parity | Chat sending, Wallet and Account in `crates/mobile` plus UIKit; release on the chosen bundle | 10–15 | TestFlight build with Chat, Wallet, Account, live sessions |
| 7. Android | A Kotlin host over `crates/mobile`, or keep the current app | 15–30, or 0 | Owner choice |
| 8. Converge | One harness layer (fork's `harness` vs `coder-sdk`), one sync path, shared Markdown, syntax and theme; resolve the `links` conflicts and join the root workspace; delete retired apps and crates | Ongoing, 20–40 | No duplicate implementations of the same capability |

Steps 1–4 are about two weeks of focused agent work. They give one
repository and apps named OpenAgents, with the fork's features and the
current OpenAgents features side by side until parity. Steps 5–6 are the
real product cost, about four to seven weeks. Step 8 can run indefinitely
and should be done crate by crate, whenever upstream tracking for that
crate is dropped.

## Risks

- **Build weight.** GPUI and the fork add about 1,300 packages; a cold
  debug build is roughly 900 crates. Use persistent target directories as
  today, and keep the separate workspace so root builds don't pay for it.
- **Upstream drift during parity work.** Every OpenAgents feature added
  inside upstream-owned files makes pulls harder. Put our features in our
  own crates and modules, with one hook each in upstream files, as
  `crates/coder-account` already does.
- **Two apps in the field.** Until step 5 ends, testers have the merged app
  and the old OpenAgents desktop. Keep the old one as is (no new features)
  and say which one is current in its About screen.
- **License hygiene.** Keep Zeron's MIT `LICENSE` (Wing) for the imported
  tree and its `THIRD_PARTY_NOTICES.md`, beside this repository's Apache-2.0.
  GPUI (Zed, via `zeronsh/zui`) is Apache-2.0; the `zui` fork already
  removed GPL tracing crates.
- **The Coder 1.0 freeze** doesn't apply. The frozen binaries are `coder`,
  `openagents`, `microcoder` and `coder-boundary`; nothing here changes them.

## Owner decisions needed

1. Keep pulling upstream Zeron for now, and decide per crate later
   (recommended), or hard fork at import.
2. Phone-to-desktop channel: `coder-sync` for sessions, with NIP-HOST
   underneath (recommended).
3. iPhone bundle: move to `com.openagents.app`, or keep `com.openagents.coder`
   and retire the old OpenAgents record.
4. Android: build a Kotlin host over the fork's core now, or keep the current
   Android app until later.
5. Directory name for the import (`apps/openagents/` suggested).
6. AGENTS.md exceptions: GPUI desktop and the UIKit iOS app.
