# Repository Map & Build/Test Command Reference

Companion reference to [AGENTS.md](../AGENTS.md). Kept separate so root `AGENTS.md` stays comfortably inside every agent harness's rule budget (<24 KB in Antigravity).

## What is here

The repository was reset (the Nuke, `0b401561a0`, 2026-10-27). The old Rust, TypeScript and app workspaces are gone. Use history only to read them, with `git show 0b401561a0^:<path>`. Never restore them wholesale.

| Path | What |
| --- | --- |
| `coax/` | Coax, the language everything is being built in: specification ([coax/spec.md](../coax/spec.md)), grammar, book (`python3 psionic/tests/book.py --book coax`), `coax0` (the C++20 bootstrap compiler emitting C11/Wasm/Metal/WGSL), `coax/lib/` (stdlib including `inky/`, `microcoder/`, `agent/`, `crypto/`, `bitcoin/`, `lightning/`, `nostr/`, `net/`, `sys/`, `lean/`, `tbench/`), `coax/INVARIANTS.md`, `coax/web/` (browser host, Wasm build, `coax serve`, HTTP API), and `coax/remote/` (build hosts, hooks, green-main pipeline) |
| `metaverse-engine/` | The OpenAgents Metaverse Engine, the engine every realm runs on, in Coax: engine-core (`core/loop.coax`, see [metaverse-engine/README.md](../metaverse-engine/README.md)), physics, rendering, audio, animation, networking (`net/voice-wire.coax`), session, realm editor (`metaverse-engine/editor/`), `metaverse-engine/INVARIANTS.md`, and `book/` (check with `python3 psionic/tests/book.py --book aethyr` or `--book metaverse-engine`) |
| `rules/` | The SRD 5.2.1 combat rules (`rules/srd/`), their own context (#13270): dice, checks, combat, spells, conditions and rests, used by Everglade's room and by other worlds. They load only the engine and `coax/`; [docs/dnd/README.md](dnd/README.md) is their specification |
| `metaverse/` | The OpenAgents Metaverse's realms and content in Coax: `home.coax`, `light.coax`, `grid.coax`, `gridwalk.coax`, `cast.coax` (`tools/pack_cast.py`), `haunt.coax`, `haunt-buffer.coax`, `hud.coax`, `splice-bots/`, voice engine (`voice.coax`, `voice-floor.coax`, `voice-asr.coax`, `voice-host/`, `platform/voice.h`, `platform/voice.c`, `platform/host.c`; wire codecs in `metaverse-engine/net/voice-wire.coax`, metering in `pay/voice-meter.coax`) |
| `everglade/` | Everglade, the MMORPG on the D&D 5E SRD: `book/` (`python3 psionic/tests/book.py --book everglade`), `docs/`, `model/`, `world/`, `server/`, `client/`, `shared/`, `hud/`, `content/`, `everglade/INVARIANTS.md`, and `features.coax`. Check: `coax/web/tool.sh everglade-parity` |
| `gamebuilder/` | GameBuilder 3D creation core, CLI/MCP, standalone worlds (`worlds/`), and `gamebuilder.org` (`web/`, `deploy/`), folded into the monorepo (#13854, #13855). Every action routes through `core/commands.coax`; see [gamebuilder/INVARIANTS.md](../gamebuilder/INVARIANTS.md). Check: `gamebuilder/scripts/check.sh` |
| `pay/` | The Payments context's money rules (#13320), every one `#authority` with zero surviving mutants: Spark wallet (`wallet.coax`, `wallet-backup.coax`, `wallet-vault.coax`, `wallet-policy.coax`, `wallet-vault-store.coax`), Lightning Addresses (`lnaddr-policy.coax`), tips and upgrades (`makerpay-policy.coax`), speech metering (`voice-meter.coax`), and Executor escrow (`pay/INVARIANTS.md`). Adapters stay with their host; Bitcoin, Lightning and LNURL libraries stay in `coax/lib/`. See [contexts](architecture/contexts.md) |
| `components/` | The component system ([spec](../components/spec.md), #13153): one set of UI components in Coax for every app, in layers: `core/` (the one Markdown parser, text, colour, syntax roles), `layout/` (the one line wrapper), `paint/`, `skin/`, `backend/` (`gpu.coax`, `gpu-shader.coax`, `dom.coax`, `terminal.coax`), `kit/`, `chat/` (with `fonts/` and `icons/`; `zeron/` was retired in #13847), `app/`, and `gallery/`. Never write a second Markdown parser, line wrapper or edit buffer outside it; check with `coax/web/tool.sh components-check` |
| `psionic/` | Psionic, the machine-learning library and inference/training engine in Coax: `lib/` (including native Metal Inkling 2 Mini runtime/server), runnable `book/` (`python3 psionic/tests/book.py --book psionic`), `examples/`, `platform/`, `tests/`, `fixtures/`, `psionic/INVARIANTS.md`, and `docs/` (`docs/targets/`, `docs/serving/`) |
| `coder/` | Coder, the coding agent: `book/` (*Coder: Coding Agents From Scratch*, `python3 psionic/tests/book.py --book coder`), `tui/` (`inky.coax`, `host.coax`, `agyworker.coax`, `gitguard.coax`), `plugins/`, `golemancy/`, and `tests/` |
| `bitcoin/` | *Bitcoin From Scratch* (`book/`, `python3 psionic/tests/book.py --book bitcoin`), `examples/`, `fixtures/`, `explorer/`, `lab/`, and `bitcoin/INVARIANTS.md`. The libraries it teaches live under `coax/lib/crypto/`, `coax/lib/bitcoin/`, and `coax/lib/lightning/` |
| `nostr/` | Nostr: `nips/` (official, external and OpenAgents NIPs, pinned), `nostr/INVARIANTS.md`, and `book/`, *Nostr From Scratch* (`python3 psionic/tests/book.py --book nostr`; libraries in `coax/lib/crypto/`, `coax/lib/nostr/`, and `coax/lib/net/`) |
| `openagents/` | *The OpenAgents Book* (`book/`): openagents.com and the apps on Coax Native first, then the general agent and its economy. Contract: [openagents/book/CONTRACT.md](../openagents/book/CONTRACT.md); check with `python3 psionic/tests/book.py --book openagents` (#13512) |
| `apps/openagents-desktop/` | OpenAgents Desktop, the Coax Native desktop app targeting macOS (with shared Coax core and Linux/Windows hosts): local Inkling 2 Mini on Psionic Metal via the Inky worker (`coax/lib/inky/`, `coder/tui/inky.coax`), Antigravity (Gemini 4), and feature-flagged capabilities (`coax/lib/agent/desktop/flags.coax`). **Primary build/install is `--minimal`** (`build.sh`, `install.sh` now default to `--minimal`; `--full` only when requested); check with `apps/openagents-desktop/check.sh` and `v1-bundle-check.coax`. See [apps/openagents-desktop/README.md](../apps/openagents-desktop/README.md) and [docs/desktop/coax-desktop.md](desktop/coax-desktop.md) |
| `apps/openagents-ios/` | OpenAgents for iOS (#13993): dedicated native SwiftUI/UIKit mobile client exclusively for Antigravity over Tailscale (`Sources/` + shared `apps/shared/antigravity/` + `apps/shared/native/AppTailnet.swift`) over the lean compiled Coax C bridge (`apps/shared/antigravity/client-core.coax` → `Generated/client_core.c` covering NIP-44 `seal.coax`, relay-free direct grants & push frames in `tailnet.coax`, environment/workspace Git/PR wire, and `#authority` Antigravity-only policy). Zero JavaScript, WebKit, Metal GPU canvas, cloud accounts, or public relays (`apps/openagents-ios/check.sh`). Bundle `com.openagents.app`. See [apps/openagents-ios/README.md](../apps/openagents-ios/README.md) |
| `apps/openagents-macos/` | OpenAgents macOS (#13993): clean-sheet native SwiftUI macOS client + Tailscale Host Server + `MenuBarExtra` companion exclusively for Antigravity over Tailscale, sharing 100% of `apps/shared/antigravity/` with OpenAgents iOS. Installs directly as `/Applications/OpenAgents.app` (bundle `com.openagents.desktop`, using the original `OpenAgents.icns` icon). Talks to local `coder-host` via `~/.openagents/coder/host/service.sock`, serves port `47109` direct Tailscale grants, NIP-44 RPCs, and live push frames to iOS & remote Macs, and manages the optional `LaunchAgent` (`coder-host serve --no-relay`). Check with `apps/openagents-macos/check.sh`. See [apps/openagents-macos/README.md](../apps/openagents-macos/README.md) |
| `apps/openagents-web/` | openagents.com on **Coax Web** (formerly Coax Native Web / Coaxial; [coax/web.md](../coax/web.md), [coax/web/SKELETON.md](../coax/web/SKELETON.md); desktop/mobile framework [Coax Native](../coax/native.md)): Coax-to-Wasm pages drawn with WebGPU and the shared DOM backend. `coax/web/` holds the Coax Web framework runtime — browser host, Wasm build, `coax web` CLI (`coax/web/cli.sh`), `coax serve` dev server, and HTTP API ([coax/web/api/COAX.md](../coax/web/api/COAX.md): new API rules go in `#authority` `*-policy.coax` compiled from source at build time; SQL-rules routes move when next changed) |
| `apps/openagents-cli/` | The `openagents` command line in Coax ([plan](cli/2026-11-01-openagents-cli-v1.md)): `nostr` (NIP-06 identity in a 0600 key file, never the Keychain unless `OPENAGENTS_KEYCHAIN=1`; signing and NIP-98), `pylon`, `project`/`projects`, `chat`, `wallet`, `transcribe`, `labs`, and more. Build with `apps/openagents-cli/build.sh` |
| `apps/shared/` | Coax and native host code shared across apps: `antigravity/` (cross-Apple Coax `#authority` C core + SwiftUI/Network client engine shared by `apps/openagents-ios/` and `apps/openagents-macos/`), `app/` (shared app core and views), `host/` (Apple/Linux GPU hosts), `native/` (Swift/C services), `auth/`, `chat/`, `composer/`, `login/`, `home/`, `aethyrforge/`, `tester/`, and dictation (`dictation*.coax`); `apps/INVARIANTS.md` covers app-layer invariants |
| `evals/` | Automated evaluation suites: `evals/openagents-desktop/` (Tiers 0–4 and compaction; `evals/openagents-desktop/run.sh`, [docs/coder/desktop-evals-spec.md](coder/desktop-evals-spec.md)), `evals/session-history/` (read-only `ATIF-v1.8` trajectory ingest/mine/sanitize/friction pipeline; [docs/coder/session-history-evals-spec.md](coder/session-history-evals-spec.md)), and `evals/terminal-bench/` (Harbor adapter and policies) |
| `os/` | Metaverse OS ([spec](os/2026-11-02-verse-os.md)), which CoderOS becomes: the machine's programs in Coax (compositor, desk, camera, hands, recording, CPU limits, update agent, and `verse-session.coax` with `#authority` `verse-pylon-policy`), and `os/nix/`, the NixOS modules, realms package and image (owner-approved 2026-11-02: the one place Nix lives here) |
| `proto/` | The protocol's protobuf schemas, descriptors, `registry.json`, vendor schemas and checks (`coax/web/tool.sh wire-check`, `scripts/proto.sh check`) |
| `contexts.txt` | Bounded-context folder/file ownership and allowed-load table enforced by `coax/web/tool.sh layer-check` ([docs/architecture/contexts.md](architecture/contexts.md)) |
| `assets/` | Shared asset registry ([assets/registry/README.md](../assets/registry/README.md): lockfiles, collections, recipes, schemas) and `assets/metaverse/` |
| `tools/`, `scripts/`, `bench/`, `resources/` | `tools/` holds `coax-nostr/` (guard, fuzz and audit harnesses), `coax-unicode/`, and `ple-ifc-to-scene.py`; `scripts/` holds repo-wide entrypoints (`check.sh`, `proto.sh`, `join-pylon.sh`, `check-books.sh`, NIP sync); `bench/` holds voice benchmarks (`bench/voice/`); `resources/` holds `lean-checker/OpenAgentsVerify.lean` |
| `docs/` | Architecture ([docs/architecture/contexts.md](architecture/contexts.md)), protocol spec ([docs/protocol/openagents-protocol-spec.md](protocol/openagents-protocol-spec.md)), cross-cutting invariants ([docs/INVARIANTS-CROSS-CUTTING.md](INVARIANTS-CROSS-CUTTING.md)), plans ([docs/plans/README.md](plans/README.md)), audits, verification, Lightning/Spark wallet, Metaverse, voice, desktop/coder specs, books glossary and transcripts |
| `.agents/` | Antigravity & OpenAgents progressive-disclosure workspace customizations: `.agents/rules/` (always-on hygiene, glob-scoped Coax/Desktop rules, model-decision git rules) and `.agents/skills/` (on-demand runbooks: `coax-pitfalls`, `desktop-ship-verify`, `session-handoff`, `transcript-hillclimb`) |

## Shared asset library

Read [the asset registry usage guide](../assets/registry/README.md) before importing, using or remixing Quaternius assets. The portable catalog is `assets/registry/`; originals, immutable revisions and editable Blender forks live under `~/.openagents/assets/`. The shared command is `~/.openagents/bin/asset-registry`. Hydrate lockfile-pinned binary assets (`assets/registry/collections/*.lock.json`) into `~/.openagents/assets/sha256/` with `coax/web/tool.sh assets-sync [--group all|realms|psionic|gamebuilder|splicebots] [--check]`. Use stable asset IDs and pinned collections; preserve source provenance and Character Create's frozen save keys. Do not add new app-private source copies.

## Full Command Reference

```sh
# Remote / heavy commands & web pages
coax/remote/pages.sh origin/main                       # main's built pages into this worktree (never a full build.sh)
coax/remote/remote.sh run -- coax/bin/coax test FILE   # runs here, or on a host when the Mac is busy
OA_REMOTE=1 coax/remote/remote.sh run -- coax/web/api/test.sh            # full API suite (~16m, never run on Mac)
OA_REMOTE=1 coax/remote/remote.sh run -- coax/web/api/test.sh --part 1/2 # and 2/2 side by side (~13m)
OA_REMOTE=1 coax/remote/remote.sh run -- env API_TESTS_ONLY=auth_checks.coax coax/web/api/test.sh

# Coax compiler, tests & mutation
coax/build.sh                                # install published compiler in seconds (or COAX_MUTATION=0 coax/build.sh)
coax/tests/run.sh --part I/8                 # one eighth of Coax suites (each <15m)
coax/tests/run.sh --only UNIT...             # just those suites (--list names them)
coax/bin/coax test FILE                      # file's checks (--list, --only SUBSTR)
coax/bin/coax mutate FILE --changed          # mutate procedures changed since origin/main (--unit A --unit B, --jobs N, --shard I/N)
coax/bin/coax status --jobs                  # shared build-worker budget
coax/bin/coax serve status                   # resident compiler daemon (COAX_SERVE=0 disables)

# Apps & Evals
apps/openagents-desktop/build.sh             # primary --minimal desktop build (--fast for -Onone local dev; --full only if requested)
apps/openagents-desktop/install.sh           # build origin/main (--minimal), verify v1-bundle-check, install /Applications/OpenAgents.app, sync daemon
coax/web/tool.sh apps/openagents-desktop/v1-bundle-check.coax PATH
evals/openagents-desktop/run.sh              # Tier 0, Tier 1, compaction (--tier 2 / --tier 3 / --tier 4)
evals/session-history/run.sh friction        # transcript friction hillclimb scorecard across Antigravity + OpenAgents Desktop
apps/openagents-ios/build.sh                 # iOS simulator build (full steps in apps/openagents-ios/README.md)
apps/openagents-ios/check.sh                 # verifies zero JS/WebKit/Metal/legacy symbols
apps/openagents-cli/build.sh
gamebuilder/scripts/check.sh

# Psionic, Books, Proto, Metaverse & Repo Checks
psionic/check.sh --only UNIT...
psionic/tests/check_elephant_slices.sh DIR   # Elephant original-weight slices (~15s)
scripts/check-books.sh                       # or python3 psionic/tests/book.py --book NAME
scripts/check.sh                             # outside-contributor local check suite
scripts/proto.sh check
python3 metaverse/test_home.py
metaverse/test_sanitized.sh
metaverse/test_voice.sh [WAV...]
coax/tests/net/tls.sh [--public [URL]]
coax/web/serve.sh apps/openagents-web
coax/web/tool.sh assets-sync --check
coax/web/tool.sh coax/web/wgsl-module.coax --site --check
coax/web/tool.sh coax/web/changelog-page.coax --check
coax/web/tool.sh everglade-parity
coax/web/tool.sh everglade/model/tools/zone-cells.coax --check everglade/world/zone-cells.coax everglade/content/everglade.content
coax/web/tool.sh components-check
coax/web/tool.sh everglade2-drift --page ORIGIN --room PORT
coax/web/tool.sh layer-check
coax/web/tool.sh prepush --replay COMMIT     # or --replay-check
coax/web/tool.sh code-route-check
coax/web/tool.sh realm-frame-check
coax/web/tool.sh agent-reference --write
coax/web/tool.sh wire-check
coax/web/tool.sh shot URL OUT.png 1280x720 --keep --eval JS
coax/web/tool.sh shot --compare A.png B.png
coax/web/tool.sh shot URL OUT.png --eval "JSON.stringify(coaxial.loadMarks())"
```
