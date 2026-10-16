# Merging the Coder Desktop fork into the monorepo

Status: assessment, 2026-10-16. Time estimates were revised the same day from the measured commit pace of the previous 7 days ([how](#how-these-estimates-were-set)). Owner decisions the same day: the phone app is the **OpenAgents mobile app** on `com.openagents.app`, and Android gets a Kotlin host over the fork's core. A [fit review](#does-this-core-fit-the-rest-of-the-product) against `nips/` and `docs/` was added. Owner request: move all relevant code from
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

1. **Import (hours).** Bring the fork in with its history as its own Cargo
   workspace inside this repository, the way `crates/psionic` and
   `crates/openagents-mobile` already live here. That sidesteps the two hard
   build conflicts (below), the toolchain gap and the fork's build profiles.
   Leave `edge/` (TypeScript) and the marketing sites behind: `coder-sync`
   already replaces the edge (#11842–#11847).
2. **One identity (about a day).** Rename the apps to OpenAgents on the
   existing OpenAgents bundle IDs, migrate local data, and move release
   scripts out of GitHub Actions.
3. **One app per platform (days).** The fork's desktop is the stronger base
   (GPUI, about 330,000 lines with its engine), so OpenAgents desktop
   features (hosted chat, Verse, Gym, pairing, wallet) move into it and the
   current `openagents-desktop` retires after parity. On the phone, the fork's
   iOS app becomes the OpenAgents mobile app (`com.openagents.app`), taking
   over from `bins/openagents-ios` and `bins/coder-ios` after it gains Chat,
   Wallet and Account. A new Kotlin host over the same core replaces the
   current Android app.
   The OpenAgents domain crates (Nostr, chat jobs, pairing, wallet, push,
   Verse, Labs) are linked into both apps as libraries, not rebuilt; see
   [the fit review](#does-this-core-fit-the-rest-of-the-product).
4. **Converge (one to two weeks, incremental).** Remove duplicate systems (agent
   harness drivers, sync, Markdown/syntax, terminal) and fold the separate
   workspace into the root one once its dependency conflicts are resolved.

The two decisions that shape everything: **whether we keep pulling upstream
Zeron** (it ships about 6 commits a day), and **how the fork's Loro
sessions on `coder-sync` and our NIP-HOST/SESS/CTRL protocols become one
path** (recommended: Loro as the data, NIP-HOST as the authority and
transport).

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
| Phone name, bundle ID (**decided**) | OpenAgents Coder, `com.openagents.coder` | The **OpenAgents mobile app**, shown as "OpenAgents", on `com.openagents.app` for iOS and Android (the existing OpenAgents records; Android's `applicationId` is already `com.openagents.app`). `com.openagents.coder` is retired after the move |
| URL scheme, IPC port | `coder-desktop://`, 27854 | `openagents://` (check the current desktop's handler), one port |
| Updater | Zeron's feed, turned off | Our release channel |
| Binary, `ZERON_*` variables, crate names | `zeron` | Unchanged while pulling upstream; rename at the hard-fork decision |

Moving the phone app onto `com.openagents.app` replaces the current
OpenAgents iOS and Android builds for their testers. App Store Connect and
Play history stay with that record. Coder iOS testers on `com.openagents.coder`
get a final build that says the app moved, then the record goes quiet. Data migration also needs the Keychain service names
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
| Android (**decided**) | None, but `crates/mobile` is a UniFFI library meant for Swift and Kotlin, and `scripts/android/build-core.sh` already builds it | Thin Kotlin over `openagents-mobile` | Build a Kotlin host over `crates/mobile`, painting the same display lists the iOS app paints; replace the current Android app at parity |

## Does this core fit the rest of the product?

The goal is one OpenAgents desktop app and one OpenAgents mobile app that
everything else plugs into. This section checks the fork's core against the
ambitions written in [`nips/`](../../nips/README.md) (33 OpenAgents NIPs plus
the Block NIPs) and the product docs.

### Verdict

**Yes, as the application shell, local agent engine and session sync. No,
as the whole core on its own.** The fork has strengths the monorepo apps
lack:
- a mature GPUI desktop;
- multi-harness agents;
- a terminal, browser, diff pane, voice and visuals;
- a phone core whose transcript layout is already in Rust, so iOS and
  Android only paint;
- Loro session sync on `coder-sync`.

It has **no Nostr, no keys, no wallet, no Verse and no Gym**. Those are the
center of the rest of the product. All of them already exist here as
Rust crates that are not tied to a UI. So the merged apps are:

```text
OpenAgents desktop (GPUI)    OpenAgents mobile (UIKit iOS + Kotlin Android)
          \                         /
     fork shell: engine, harnesses, crates/mobile (UniFFI), Loro sync
          \                         /
   OpenAgents domain crates, shared with web, terminal, servers and hosts:
   nostr, nostr-transport, openagents-chat(-app), openagents-connect,
   coder-reach/coder-host clients, spark-wallet, push-gateway (client),
   xp, vault, verse runtime, lab-kit, openagents-sdk
```

The domain crates are the part to base other products on. The fork
provides the best shells for them. A product capability is added once, as
a domain crate plus a card or screen. It is never rebuilt inside one app.

### Ambitions against the core

| Ambition (source) | What the apps need | Fork today | Fit and work |
| --- | --- | --- | --- |
| Chat with OpenAgents: encrypted NIP-CJ jobs, a Decisions router, Gym and Coder cards (README; `docs/product/2026-09-28-app-wireframe.md`; NIP-CJ, NIP-DEC) | Nostr key custody, relay connections, send/stream/stop, cards | Reads and follows saved chats through the SDK only | **Gap.** Link `openagents-chat-app` (already used by both current apps) into `crates/mobile` and the desktop; don't rebuild chat over HTTP |
| Identity and keys (NIP-19, NIP-42, NIP-44, NIP-17; OA/AA admission) | One secp256k1 identity per person or device in Keychain/Keystore, next to the OpenAgents account session | Account session only (`coder-account`) | **Gap, contained.** Add a key-custody module beside `coder-account`, reusing `crates/nostr` |
| Device control: NIP-HOST pairing, NIP-REACH, NIP-TERM, NIP-SESS, NIP-CTRL ("the core of both apps") | Pair by QR or iroh, find hosts, drive threads and terminals, steer | Loro rooms plus the device relay on `coder-sync` | **Overlap: the main architectural decision.** Keep Loro documents as the session data model and `coder-sync` as relay and storage. Make NIP-HOST grants the authority for which device joins which room, and carry the device channel over NIP-HOST/REACH (#11854). Revise NIP-SESS and NIP-CTRL to describe the Loro rooms, or keep them for hosts that don't run the fork engine. One of the two must give way; running both is not an option |
| Wallet: Spark/Breez on every device, Bitcoin only (`docs/breez/`); x402 and NIP-POL spend approval on the phone (`docs/payments/`) | Seed in Keychain/Keystore, `crates/spark-wallet`, approval sheets | None | **Fits the build.** Breez needs `libsqlite3-sys` 0.30, the same as the fork. Only the server-side `ldk-node` needs 0.28. Port the wallet screens from `openagents-mobile` |
| Verse, the Grid, Everglade; NIP-MV presence, NIP-XP (`docs/verse/`) | A native wgpu surface running `verse::runtime::WorldRuntime`, touch and motion input, relay presence | None | **Fits, with work.** The desktop already hosts native macOS views under GPUI (the browser pane, `crates/ui/src/browser/macos.rs`). Host Verse there, and in a `UIView`/`SurfaceView` on phones via `coder-mobile`'s render handle. GPUI uses wgpu 29 and Verse uses 30; both can link (two copies) until aligned. Linux and Windows desktop embedding needs its own check |
| Gym, plugins, eval cards (`docs/gym/`, NIP-EVAL, NIP-REG/EXT) | Cards in chat, a hosted-runner client, signed records | None | **Gap, contained**, if cards come from one place (next row) |
| Labs: Video, CAD, Game, later Film and Photo (`docs/labs/README.md`) | Standard views: video, image, compare, glTF, playable, report | None (Video Lab issue #11883 plans the desktop row) | **Fits**; the Labs plan already assumes GPUI and UIKit |
| Push: NIP-PL leases through `push-gateway` | APNs/FCM token registration and lease | iOS push code that used Zeron's edge | **Gap, small.** Point it at `push-gateway`'s `client` feature (#11853) |
| Voice, camera, screen: NIP-LIVE | Audio and video capture and playback | Voice crates (`voice`, `orb`, `veil`, `audio`) | **Strength.** Bind to NIP-LIVE later |
| Vault (NIP-VAULT `user` tier), attested workloads (NIP-ATT) | Sealed personal data, verifying attestations | None | **Gap, later.** Domain crates exist |
| Market, labor, Pylon views (NIP-MKT, NIP-LAB, NIP-PYLON) | Buyer and provider views | None | **Later**, as cards |
| Computers, SDK (`docs/sdk/`, `docs/cloud/`) | Computers client, account sign-in | SDK sign-in and history | **Fits**: same SDK, now a path dependency |
| Web (openagents.com) and terminal (Coder) | Not app shells | — | **Unchanged.** They share the domain crates and cards, not the apps' UI code |

### One card layer for all four surfaces

The current rule makes Rust Native the single UI layer, with thin Swift and
Kotlin. The fork breaks that for app chrome: GPUI on the desktop, and on
iOS 34,000 lines of UIKit painting display lists laid out by `crates/mobile`.
Writing every Gym, Lab, wallet and approval card three times (GPUI, UIKit,
Kotlin) plus once for the web would undo the main benefit of shared Rust.

**Recommendation:**
- Keep **Rust Native as the card and sheet layer**: semantic views
  (`rust-native.view.v3`) for chat cards, Gym and Lab cards, wallet and
  approval sheets, and settings forms.
- Add a GPUI adapter for Rust Native views on the desktop. The existing
  SwiftUI and Kotlin adapters mount in UIKit and Kotlin hosts, so phone
  cards render through what is already built.
- The app shells (windows, sidebars, transcript, composer, terminal,
  browser) are GPUI and native UIKit/Kotlin, as in the fork.
- Amend AGENTS.md to say exactly this, rather than blanket exceptions.

### Two workspaces, by role

The `links` conflicts sort cleanly by role:

| Workspace | Members | SQLite | Notes |
| --- | --- | --- | --- |
| **Apps** (new, from the fork) | Desktop, `crates/mobile`, the fork crates, plus the domain crates they link: `nostr`, `openagents-chat-app`, `spark-wallet` with SQLite, `verse` runtime, `lab-kit`, `openagents-sdk` | 0.30 (fork and Breez) | Rust ≥ 1.98.1 for GPUI; its own profiles; replaces the separate `crates/openagents-mobile` workspace when that app retires |
| **Root** (today's) | Servers, hosts, CLI and terminal, relay, `ldk-node` wallet, Psionic excluded as now | 0.28 (`ldk-node`) | Unchanged |

Domain crates must build in both. They already avoid SQLite (as
`crates/spark-wallet` does in the root) or put it behind a feature. Moving
`ldk-node` forward would allow one workspace later; nothing depends on that.
`tree-sitter` aligns at 0.27 when the fork's grammars move up.

### Gaps to watch

- **Linux and Windows.** The current OpenAgents desktop is built for all
  three platforms. The fork's Linux and Windows packages are unbuilt under
  our identity, and native Verse embedding there is unproven.
- **Swift/Kotlin duplication.** Every new phone screen that isn't a Rust
  Native card costs two implementations. Keep screens few and cards many.
- **Upstream pull pressure.** Domain-crate wiring added inside upstream files
  makes Zeron merges harder. Add one hook per upstream file and keep the
  rest in our crates.

## Proposed plan

**Progress (2026-10-16):** step 1 is done. The fork (`cb5774df`, which
includes its Video Lab tools) is in `apps/openagents/` with its history, as
its own workspace on Rust 1.98.1, using the SDK by path. `edge/` (apart from
`edge/src/install.sh`), the sites and the workflows were removed. Checks run:
- `cargo build -p zeron` passes;
- `cargo test -p coder-account -p coder-brand -p zeron-update -p zeron-mcp`
  passes (83 tests);
- `scripts/ios/build-core.sh` builds the phone core with unchanged Swift
  bindings;
- an arm64 iOS simulator build installs and opens to its sign-in screen;
- the root workspace's 250 packages don't include the import.

**Step 2 is done (2026-10-16).**
- **Server:** the `coder-sync` server and its conformance suite now use the
  sync clients and the SDK by path from this tree. The image copies exactly
  those paths.
- **Root workspace:** it now excludes `apps/openagents`. Cargo otherwise
  resolves the imported crates' inherited fields against the root manifest.
- **Conformance:** `scripts/coder-sync/conformance.sh` passes in full,
  including the headless engine.
- **Links:** docs links point at `apps/openagents/`, and the open issues that
  named the fork have a comment.
- **Old repository:** `OpenAgentsInc/coder-desktop` is archived read-only.

Next is step 3, release scripts.

Estimates are wall-clock time at this repository's measured pace (next
section), with two to four agents working at once where the steps allow it.

| Step | Work | Estimate | Exit check |
| --- | --- | --- | --- |
| 0. Decisions | Upstream policy, sync channel and NIP-SESS/CTRL role, card layer, directory name (bundle IDs and Android are decided) | Owner; same day | Recorded here and in AGENTS.md |
| 1. Import | `git subtree add` the fork without `edge/`, the sites and workflows, into one directory (suggested `apps/openagents/`) as its own workspace, with a nested toolchain file; exclude it from the root workspace; switch `coder-account` to the path SDK; AGENTS.md exceptions (GPUI desktop, UIKit iOS) | 2–4 hours (a cold build of about 900 crates is a large part of it) | `cargo build -p zeron`, the fork's tests and `scripts/ios/build-core.sh` pass from the monorepo; the iOS simulator build runs; the monorepo's own checks are unchanged |
| 2. Retire the fork repo | Move FORK.md, patch notes and the conformance checkpoint (`scripts/coder-sync/conformance.sh` now runs in one tree); archive `coder-desktop` read-only; repoint issue and doc links | About 1 hour | No live references to the fork repo outside history |
| 3. Release scripts | macOS sign/notarize/package, iOS TestFlight (match `scripts/release/testflight.sh`), Windows and Linux packaging as local scripts; no GitHub workflows | 3–6 hours, plus owner-held signing steps | A signed macOS build and a TestFlight validate-only run from the monorepo |
| 4. OpenAgents identity | `crates/brand` values, icons, data migration, Keychain migration, updater channel, About and licenses | 4–8 hours | Coder Desktop users and OpenAgents desktop users both land in the new app with their data; dedicated test accounts only |
| 4b. Domain core | Link the domain crates into the apps workspace: key custody beside `coder-account`; `openagents-chat-app` (CJ chat send/stream/stop); NIP-HOST/REACH clients; `push-gateway` client; `spark-wallet`; a GPUI adapter for Rust Native cards | 1–2 days (can overlap steps 5–7) | Both apps send an encrypted chat, pair a computer and show a wallet balance on a test account, from the same crates the current apps use |
| 5. Desktop parity | Hosted chat sending through the SDK; Gym cards; Verse surface; settings | 1–3 days | Every row of the OpenAgents desktop table works in the merged app; `openagents-desktop` marked retired |
| 6. Phone parity | Chat sending, Wallet and Account in `crates/mobile` plus UIKit; release on the chosen bundle | 1–2 days (can overlap step 5) | TestFlight build with Chat, Wallet, Account, live sessions |
| 7. Android (decided) | A Kotlin host over `crates/mobile` painting its display lists, with the Rust Native card adapter; same `com.openagents.app` package | 2–4 days | Play internal-test build at parity with iOS; the current Android app retired |
| 8. Converge | One harness layer (fork's `harness` vs `coder-sdk`), one sync path, shared Markdown, syntax and theme; resolve the `links` conflicts and join the root workspace; delete retired apps and crates | 1–2 weeks, in the background, crate by crate | No duplicate implementations of the same capability |

**Steps 1–4 fit in one day.** They give one repository and apps named
OpenAgents, with the fork's features and the current OpenAgents features
side by side until parity.

**Steps 5–6 are the real product cost: about 2–4 days.** The largest single
item is sending hosted chats from the fork's apps through the SDK.

**One app per platform, Android included, takes about 7–12 working days in
total** (with steps 4b and 7), not counting convergence. Step 8 runs alongside later work, whenever upstream
tracking for a crate is dropped.

### How these estimates were set

The first version of this document used "agent-days" (about 2 weeks for
steps 1–4, and 4–7 weeks for steps 5–6). Measured pace over the 7 days to
2026-10-16 is far higher:

- **This repository:** about 1,930 commits, 170–310 a day. Leaving out 7
  very large commits (vendored or deleted trees), they added about
  1.25 million lines and removed 72,000.
- **The fork:** 23 commits of ours in 2 days (16,500 lines added, 4,700
  removed), beside 31 upstream commits in the same week.

Delivered work comparable to these steps:

| Work | Issues | Opened → closed (UTC, 2026-10-16) | Compares to |
| --- | --- | --- | --- |
| Fork iOS rename: identity, icon, Zeron services off | #11841 | 04:45 → 05:13 (28 min) | Step 4 |
| `coder-sync` server: skeleton, chat rooms, registry, device relay, blobs, conformance (8,700 lines) | #11842–#11847 | 04:45 → 08:31 (under 4 h for all six) | Steps 5 and 8 |
| Desktop SDK sign-in, sidebar, saved-chat reader, end-to-end proof | #11824, #11829–#11832 | 02:13 → 06:57 (under 5 h) | Step 5 |
| Shared sign-in, desktop-to-phone live sessions, first TestFlight build, saved chats on the phone | #11848–#11851 | 04:46 → 13:59 (about 9 h) | Steps 3 and 6 |
| CAD Lab hosted on openagents.com | #11653 | 2026-10-15 01:55 → 12:37 (about 11 h) | Step 5 (a feature with a 3D view) |

The whole Coder-for-iPhone epic (#11840) went from issues to a TestFlight
build in about 9 hours, so the multi-week figures were 5–10 times too high.

**What doesn't speed up with more agents:**
- owner-held steps: App Store records, signing and notarization
  credentials, TestFlight review, physical-device checks;
- cold builds of about 900 crates;
- local disk space (#11869 stalled below 8 GiB);
- GPUI parity work, which goes through a few very large files
  (`crates/ui/src/transcript.rs` is over 15,000 lines). Agents working in
  parallel there conflict, so that work runs one or two at a time.

The upper ends of the ranges allow for those.

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

## Owner decisions

Decided (2026-10-16):

- **Phone app:** the OpenAgents mobile app, on `com.openagents.app` for iOS
  and Android; `com.openagents.coder` retires after the move.
- **Android:** a Kotlin host over the fork's `crates/mobile`.

Also decided (2026-10-16), and recorded in [AGENTS.md](../../AGENTS.md):

- **Upstream:** keep pulling Zeron with `git subtree pull`; decide crate by
  crate later whether to stop.
- **Sessions:** Loro documents on `coder-sync`, NIP-HOST grants as the
  authority, NIP-HOST/REACH as transport. NIP-SESS and NIP-CTRL are marked
  for revision.
- **UI rule:** Rust Native is the card and sheet layer on every surface;
  GPUI, UIKit and Kotlin are the app shells.
- **Directory:** `apps/openagents/`.
