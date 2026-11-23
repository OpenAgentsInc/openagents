# AGENTS

What every agent working in this repository needs to know. Read
[INVARIANTS.md](INVARIANTS.md) and the per-context invariants file it indexes
for the area you are changing before modifying anything it covers, and add new
invariants to the matching context file. Follow the
[root glossary](docs/books/glossary.md) for new identifiers, tables, registry
IDs and wire fields.

## Git

- Work on `main`. **Anytime you make changes at a user's request, always commit only the files you changed, rebase onto `origin/main`, push to `main` (`git push origin HEAD:main`), and clean up any temporary worktrees and branches when finished.** Never leave user-requested changes sitting unpushed or only on a side branch.
- **Never edit files in the main checkout**
  (`/Users/christopherdavid/work/openagents`). It stays clean on `main` so
  anyone can `git pull --ff-only` it. Edit, build and test in your own
  worktree (`git worktree add ../oa-<topic> origin/main`), push `HEAD:main`
  from there, and remove the worktree when done. Never stash, reset or check
  out over uncommitted changes in the main checkout; they belong to another
  agent.
- **OpenAgents Desktop builds & installs (`/Applications/OpenAgents.app`).**
  Always build and install `--minimal`
  (`apps/openagents-desktop/build.sh --minimal` /
  `apps/openagents-desktop/install.sh --minimal`); `--minimal` is the primary
  desktop build. **Never** build `--full` unless the user specifically asks.
  Because `install.sh --minimal` builds from `origin/main` (under
  `~/.cache/openagents/coder-release`), verifies the bundle with
  `v1-bundle-check.coax`, and syncs `~/.openagents/bin/openagents-environment`,
  **always commit and push to `main` first**, then run
  `apps/openagents-desktop/install.sh --minimal` (never hand-copy a branch
  build into `/Applications/OpenAgents.app`).
- This section is for team agents. A contributor outside the team works
  from a fork and opens a pull request ([CONTRIBUTING.md](CONTRIBUTING.md));
  their one check is `scripts/check.sh`, the pre-push check's local checks
  with nothing that needs a secret or a team host.
- Rebase on `origin/main` before pushing. When you change something that
  another change also touched (a version number, error codes, the grammar),
  merge both changes; don't overwrite theirs.
- End commit messages with a `Co-Authored-By` line for the model that
  wrote them, and an `Agent: <id>` line with your agent ID (the one your
  brief gives you), so a failure on `main` reaches the agent that caused it
  ([docs/metaverse/green-main-pipeline.md](docs/metaverse/green-main-pipeline.md)).
- A pre-push check runs on every push to `main` (it is not a queue: about
  10 to 20 seconds, never more than two minutes). It refuses a commit that
  undoes another agent's work of the last 24 hours (a tree written from a
  stale snapshot), a commit with no `Agent:` line, and a failing
  `tools-check`, `everglade-parity`, `wire-check`, `layer-check`, `state-keep`
  test, `agent-reference --check`, `drive-hit-check` (a push that changes
  `coax drive`), `wgsl-module --site --check` (a push that changes Coax or a
  shader), `changelog-page --check` (a push that changes `coax/CHANGELOG.md`
  or its page), `coax check` on roots above changed `.coax` files, or touched
  suite (`coax test`, plus `coax mutate --changed` on touched `#authority`
  suites). The hooks are tracked in `coax/remote/hooks/`; a clone turns them on
  once with `git config core.hooksPath coax/remote/hooks` (this Mac's clone
  has it, so every worktree gets it). When a refusal is right, fix the
  commit; when you mean to delete or restore another agent's file, name the
  file (or its folder/, or the commit you undo) in the message. Emergency
  only: `OA_PREPUSH_BYPASS="why" git push ...`, which is logged. Never use
  `--no-verify` ([the pre-push check](docs/metaverse/green-main-pipeline.md#the-pre-push-check)).
- `coax/remote/after.sh state` shows whether `main` is green: `main`,
  `main-green` (the newest head that passed the after-landing check), the last
  runs and the red list. When `main-green` is behind and you need a base that
  passes, start from it: `git worktree add ../oa-<topic> origin/main-green`.
- The allowlist counts (`coax/remote/counts.sh show`: `parity-known`,
  `parity-gaps`, `components-to-move`, `wire-allowlist`, `tools-port.txt`,
  `layer-check` per rule, `sql-rules-routes`) only fall: a head that raises
  one is red and names its commit. A commit that must add a line also says why
  in a trailer, `Rebaseline: NAME: why`. A head worse than production in
  Everglade II's room pair (`check.sh e2`) is red too
  ([the comparing gates](docs/metaverse/green-main-pipeline.md#the-comparing-gates)).
- Before you investigate a failure on `main`, check the red list (in that
  output). A failure listed there is already owned. When an entry names your
  commit, fixing it comes before new work: a culprit still failing 30 minutes
  after it is reported is reverted.
- Owner actions (taps, keys, payments, sign-offs) go in the workspace's
  `NEEDS_OWNER.md`, not only in chat.

## What is here

The repository was reset (the Nuke, `0b401561a0`, 2026-10-27). The old
Rust, TypeScript and app workspaces are gone. Use history only to read
them, with `git show 0b401561a0^:<path>`. Never restore them wholesale.

| Path | What |
| --- | --- |
| `coax/` | Coax, the language everything is being built in: specification ([coax/spec.md](coax/spec.md)), grammar, book (`python3 psionic/tests/book.py --book coax`), `coax0` (the C++20 bootstrap compiler emitting C11/Wasm/Metal/WGSL), `coax/lib/` (stdlib including `inky/`, `microcoder/`, `agent/`, `crypto/`, `bitcoin/`, `lightning/`, `nostr/`, `net/`, `sys/`, `lean/`, `tbench/`), `coax/INVARIANTS.md`, `coax/web/` (browser host, Wasm build, `coax serve`, HTTP API), and `coax/remote/` (build hosts, hooks, green-main pipeline) |
| `metaverse-engine/` | The OpenAgents Metaverse Engine, the engine every realm runs on, in Coax: engine-core (`core/loop.coax`, see [metaverse-engine/README.md](metaverse-engine/README.md)), physics, rendering, audio, animation, networking (`net/voice-wire.coax`), session, realm editor (`metaverse-engine/editor/`), `metaverse-engine/INVARIANTS.md`, and `book/` (check with `python3 psionic/tests/book.py --book aethyr` or `--book metaverse-engine`) |
| `rules/` | The SRD 5.2.1 combat rules (`rules/srd/`), their own context (#13270): dice, checks, combat, spells, conditions and rests, used by Everglade's room and by other worlds. They load only the engine and `coax/`; [docs/dnd/README.md](docs/dnd/README.md) is their specification |
| `metaverse/` | The OpenAgents Metaverse's realms and content in Coax: `home.coax`, `light.coax`, `grid.coax`, `gridwalk.coax`, `cast.coax` (`tools/pack_cast.py`), `haunt.coax`, `haunt-buffer.coax`, `hud.coax`, `splice-bots/`, voice engine (`voice.coax`, `voice-floor.coax`, `voice-asr.coax`, `voice-host/`, `platform/voice.h`, `platform/voice.c`, `platform/host.c`; wire codecs in `metaverse-engine/net/voice-wire.coax`, metering in `pay/voice-meter.coax`) |
| `everglade/` | Everglade, the MMORPG on the D&D 5E SRD: `book/` (`python3 psionic/tests/book.py --book everglade`), `docs/`, `model/`, `world/`, `server/`, `client/`, `shared/`, `hud/`, `content/`, `everglade/INVARIANTS.md`, and `features.coax`. Check: `coax/web/tool.sh everglade-parity` |
| `gamebuilder/` | GameBuilder 3D creation core, CLI/MCP, standalone worlds (`worlds/`), and `gamebuilder.org` (`web/`, `deploy/`), folded into the monorepo (#13854, #13855). Every action routes through `core/commands.coax`; see [gamebuilder/INVARIANTS.md](gamebuilder/INVARIANTS.md). Check: `gamebuilder/scripts/check.sh` |
| `pay/` | The Payments context's money rules (#13320), every one `#authority` with zero surviving mutants: Spark wallet (`wallet.coax`, `wallet-backup.coax`, `wallet-vault.coax`, `wallet-policy.coax`, `wallet-vault-store.coax`), Lightning Addresses (`lnaddr-policy.coax`), tips and upgrades (`makerpay-policy.coax`), speech metering (`voice-meter.coax`), and Executor escrow (`pay/INVARIANTS.md`). Adapters stay with their host; Bitcoin, Lightning and LNURL libraries stay in `coax/lib/`. See [contexts](docs/architecture/contexts.md) |
| `components/` | The component system ([spec](components/spec.md), #13153): one set of UI components in Coax for every app, in layers: `core/` (the one Markdown parser, text, colour, syntax roles), `layout/` (the one line wrapper), `paint/`, `skin/`, `backend/` (`gpu.coax`, `gpu-shader.coax`, `dom.coax`, `terminal.coax`), `kit/`, `chat/` (with `fonts/` and `icons/`; `zeron/` was retired in #13847), `app/`, and `gallery/`. Never write a second Markdown parser, line wrapper or edit buffer outside it; check with `coax/web/tool.sh components-check` |
| `psionic/` | Psionic, the machine-learning library and inference/training engine in Coax: `lib/` (including native Metal Inkling 2 Mini runtime/server), runnable `book/` (`python3 psionic/tests/book.py --book psionic`), `examples/`, `platform/`, `tests/`, `fixtures/`, `psionic/INVARIANTS.md`, and `docs/` (`docs/targets/`, `docs/serving/`) |
| `coder/` | Coder, the coding agent: `book/` (*Coder: Coding Agents From Scratch*, `python3 psionic/tests/book.py --book coder`), `tui/` (`inky.coax`, `host.coax`, `agyworker.coax`, `gitguard.coax`), `plugins/`, `golemancy/`, and `tests/` |
| `bitcoin/` | *Bitcoin From Scratch* (`book/`, `python3 psionic/tests/book.py --book bitcoin`), `examples/`, `fixtures/`, `explorer/`, `lab/`, and `bitcoin/INVARIANTS.md`. The libraries it teaches live under `coax/lib/crypto/`, `coax/lib/bitcoin/`, and `coax/lib/lightning/` |
| `nostr/` | Nostr: `nips/` (official, external and OpenAgents NIPs, pinned), `nostr/INVARIANTS.md`, and `book/`, *Nostr From Scratch* (`python3 psionic/tests/book.py --book nostr`; libraries in `coax/lib/crypto/`, `coax/lib/nostr/`, and `coax/lib/net/`) |
| `openagents/` | *The OpenAgents Book* (`book/`): openagents.com and the apps on Coax Native first, then the general agent and its economy. Contract: [openagents/book/CONTRACT.md](openagents/book/CONTRACT.md); check with `python3 psionic/tests/book.py --book openagents` (#13512) |
| `apps/openagents-desktop/` | OpenAgents Desktop, the Coax Native desktop app targeting macOS (with shared Coax core and Linux/Windows hosts): local Inkling 2 Mini on Psionic Metal via the Inky worker (`coax/lib/inky/`, `coder/tui/inky.coax`), Antigravity (Gemini 4), and feature-flagged capabilities (`coax/lib/agent/desktop/flags.coax`). **Always build/install `--minimal`** (`build.sh --minimal`, `install.sh --minimal`) unless `--full` is requested; check with `apps/openagents-desktop/check.sh` and `v1-bundle-check.coax`. See [apps/openagents-desktop/README.md](apps/openagents-desktop/README.md) and [docs/desktop/coax-desktop.md](docs/desktop/coax-desktop.md) |
| `apps/openagents-ios/` | OpenAgents for iOS (#13993): dedicated native SwiftUI/UIKit mobile client exclusively for Antigravity over Tailscale (`Sources/` + shared `apps/shared/antigravity/` + `apps/shared/native/AppTailnet.swift`) over the lean compiled Coax C bridge (`apps/shared/antigravity/client-core.coax` → `Generated/client_core.c` covering NIP-44 `seal.coax`, relay-free direct grants & push frames in `tailnet.coax`, environment/workspace Git/PR wire, and `#authority` Antigravity-only policy). Zero JavaScript, WebKit, Metal GPU canvas, cloud accounts, or public relays (`apps/openagents-ios/check.sh`). Bundle `com.openagents.app`. See [apps/openagents-ios/README.md](apps/openagents-ios/README.md) |
| `apps/openagents-macos/` | OpenAgents macOS (#13993): clean-sheet native SwiftUI macOS client + Tailscale Host Server + `MenuBarExtra` companion exclusively for Antigravity over Tailscale, sharing 100% of `apps/shared/antigravity/` with OpenAgents iOS. Talks to local `coder-host` via `~/.openagents/coder/host/service.sock`, serves port `47109` direct Tailscale grants, NIP-44 RPCs, and live push frames to iOS & remote Macs, and manages the optional `LaunchAgent` (`coder-host serve --no-relay`). Bundle `com.openagents.macos`. Check with `apps/openagents-macos/check.sh`. See [apps/openagents-macos/README.md](apps/openagents-macos/README.md) |
| `apps/openagents-web/` | openagents.com on Coax Native Web, formerly Coaxial ([coax/web.md](coax/web.md); app framework [Coax Native](coax/native.md)): Coax-to-Wasm pages drawn with WebGPU. `coax/web/` holds the browser host, Wasm build, `coax serve` dev server, and HTTP API ([coax/web/api/COAX.md](coax/web/api/COAX.md): new API rules go in `#authority` `*-policy.coax` compiled from source at build time; SQL-rules routes move when next changed) |
| `apps/openagents-cli/` | The `openagents` command line in Coax ([plan](docs/cli/2026-11-01-openagents-cli-v1.md)): `nostr` (NIP-06 identity in a 0600 key file, never the Keychain unless `OPENAGENTS_KEYCHAIN=1`; signing and NIP-98), `pylon`, `project`/`projects`, `chat`, `wallet`, `transcribe`, `labs`, and more. Build with `apps/openagents-cli/build.sh` |
| `apps/shared/` | Coax and native host code shared across apps: `antigravity/` (cross-Apple Coax `#authority` C core + SwiftUI/Network client engine shared by `apps/openagents-ios/` and `apps/openagents-macos/`), `app/` (shared app core and views), `host/` (Apple/Linux GPU hosts), `native/` (Swift/C services), `auth/`, `chat/`, `composer/`, `login/`, `home/`, `aethyrforge/`, `tester/`, and dictation (`dictation*.coax`); `apps/INVARIANTS.md` covers app-layer invariants |
| `evals/` | Automated evaluation suites: `evals/openagents-desktop/` (Tiers 0–4 and compaction; `evals/openagents-desktop/run.sh`, [docs/coder/desktop-evals-spec.md](docs/coder/desktop-evals-spec.md)), `evals/session-history/` (read-only `ATIF-v1.8` trajectory ingest/mine/sanitize pipeline; [docs/coder/session-history-evals-spec.md](docs/coder/session-history-evals-spec.md)), and `evals/terminal-bench/` (Harbor adapter and policies) |
| `os/` | Metaverse OS ([spec](docs/os/2026-11-02-verse-os.md)), which CoderOS becomes: the machine's programs in Coax (compositor, desk, camera, hands, recording, CPU limits, update agent, and `verse-session.coax` with `#authority` `verse-pylon-policy`), and `os/nix/`, the NixOS modules, realms package and image (owner-approved 2026-11-02: the one place Nix lives here) |
| `proto/` | The protocol's protobuf schemas, descriptors, `registry.json`, vendor schemas and checks (`coax/web/tool.sh wire-check`, `scripts/proto.sh check`) |
| `contexts.txt` | Bounded-context folder/file ownership and allowed-load table enforced by `coax/web/tool.sh layer-check` ([docs/architecture/contexts.md](docs/architecture/contexts.md)) |
| `assets/` | Shared asset registry ([assets/registry/README.md](assets/registry/README.md): lockfiles, collections, recipes, schemas) and `assets/metaverse/` |
| `tools/`, `scripts/`, `bench/`, `resources/` | `tools/` holds `coax-nostr/` (guard, fuzz and audit harnesses), `coax-unicode/`, and `ple-ifc-to-scene.py`; `scripts/` holds repo-wide entrypoints (`check.sh`, `proto.sh`, `join-pylon.sh`, `check-books.sh`, NIP sync); `bench/` holds voice benchmarks (`bench/voice/`); `resources/` holds `lean-checker/OpenAgentsVerify.lean` |
| `docs/` | Architecture ([docs/architecture/contexts.md](docs/architecture/contexts.md)), protocol spec ([docs/protocol/openagents-protocol-spec.md](docs/protocol/openagents-protocol-spec.md)), cross-cutting invariants ([docs/INVARIANTS-CROSS-CUTTING.md](docs/INVARIANTS-CROSS-CUTTING.md)), plans ([docs/plans/README.md](docs/plans/README.md)), verification, Lightning/Spark wallet, Metaverse, voice, desktop/coder specs, books glossary and transcripts |

## Shared asset library

Read [the asset registry usage guide](assets/registry/README.md) before importing,
using or remixing Quaternius assets. The portable catalog is `assets/registry/`;
originals, immutable revisions and editable Blender forks live under
`~/.openagents/assets/`. The shared command is `~/.openagents/bin/asset-registry`.
Hydrate lockfile-pinned binary assets (`assets/registry/collections/*.lock.json`)
into `~/.openagents/assets/sha256/` with
`coax/web/tool.sh assets-sync [--group all|realms|psionic|gamebuilder|splicebots] [--check]`.
Use stable asset IDs and pinned collections; preserve source provenance and
Character Create's frozen save keys. Do not add new app-private source copies.

## Rules

- **Done means the real thing works** (owner, 2026-11-20). An issue, chapter
  or goal is done when what it asks for works the way a user would use it,
  checked end to end against every acceptance criterion it lists. A toy,
  synthetic, miniature, stubbed, mocked or placeholder stand-in is not done,
  and neither is code that only compiles or only passes tests and fixtures
  you wrote for it. A miniature fixture can be added next to the real check,
  never instead of it. (A chat closed seven Inkling 2 Mini issues in 14
  minutes with a vocabulary-of-4 model, CPU code named "metal", and a
  Download button that wrote a one-line `manifest.json` so the model showed
  as installed; asked whether it could run inference, it said no.)
  - **Can't do the real thing? Say so.** Missing weights, hardware, secrets,
    access or time is a blocker: report it and stop, or keep the issue open
    with what is left. Never swap in a smaller task and report it under the
    original's name.
  - **Close an issue only when every acceptance box holds**, and say in the
    closing comment how each one was checked (the command and its real
    output). Don't set a chapter's status to `implemented`, or make a model
    or feature selectable in the app, until it actually runs.
  - **No UI that lies.** Never write a marker file, flag or fake state that
    makes the app show something as installed, ready or working when it
    isn't.
  - **Report only what you ran, with its real result.** A check that was
    killed, timed out, skipped or not run did not pass; name it as such.
  - `/goal` chats in OpenAgents Desktop check a `[GOAL_COMPLETE]` with one
    more turn that re-reads every issue and runs the real thing before the
    goal closes (`apps/openagents-desktop/page/goal.coax`).
- **Never hand-roll what already exists** (owner, 2026-11-11;
  [docs/reuse.md](docs/reuse.md)). Before writing a text box, button, undo
  stack, camera, controller, terrain, parser, hash or model call, find the
  shared one (`components/`, `metaverse-engine/`, `everglade/`, `coax/lib/`,
  `psionic/`) in `docs/reuse.md` and use it. If it falls short, extend the
  shared one. No second copy, no "local version for now". A hand-rolled copy
  you find while working moves onto the shared module in the same change, or
  gets an issue naming both.
- **Tools are Coax too** (owner, 2026-10-30; [coax/tools-in-coax.md](coax/tools-in-coax.md)).
  A build script, check, test harness, recorder or CLI you write is Coax,
  run with `coax/web/tool.sh FILE.coax`. A new `.ts`, `.py`, `.js`, `.sh` or
  `.cjs` file needs a header line `Firm reason: N (...)` naming one of the
  six reasons in that doc: browser glue, an independent oracle, a reference
  implementation, the bootstrap, a thin wrapper with no logic, or a
  Swift-only or other non-C SDK. "Faster in TypeScript" is not one.
  Run `coax/web/tool.sh tools-check` yourself before you push;
  `coax/web/release.sh` reports a file with neither but doesn't stop.
- **Coax first.** New product logic goes in Coax. The host language (Swift,
  C) does platform effects only: lifecycle, Keychain, network, GPU
  bindings. Avoid toolchains outside Coax and the host C compiler. Python
  is tooling only; it is never shipped and never needed at run time.
  Exception, owner-approved 2026-10-29: the NVIDIA CUDA toolkit (installed
  through Nix) may compile and profile Coax-generated GPU code. Coax still
  writes the arithmetic; no hand-written kernels, and no cuBLAS or cuDNN in
  anything served or shipped. cuBLAS may appear only in opt-in benchmark and
  parity harnesses that measure Psionic's own kernels against it.
  Exception, owner-approved 2026-10-29: the C host may link one TLS library
  (OpenSSL 3 on Linux: the system's when it has one, else through Nix, and
  always through Nix on NixOS; the platform's TLS on Apple) to encrypt
  sockets, so native clients reach `wss://` relays. It does the TLS handshake,
  certificate verification and record encryption only; every protocol
  decision above the socket (WebSocket, Nostr, NIP-42, NIP-44) stays in Coax.
- **Check first.** Write `#test`/`#forall` checks before the code. Use
  `#authority;` for any file that decides money, admission or policy. Its
  first `coax mutate` pass goes to zero survivors or marked equivalents.
  After that, a change mutates the procedures it touched
  (`coax mutate FILE --changed origin/main`), and a survivor there blocks
  the change (`coax/remote/check.sh` runs it on every touched `#authority`
  suite). The whole sweep runs nightly on the build pool
  (`coax/remote/mutation-sweep.sh`), keeps one issue open while it finds
  survivors, closes it on a clean night, and blocks nothing. Mark an equivalent
  mutant (`// mutant: equivalent`) with a reason a reviewer can check; don't
  mark one to make a run pass, and don't rewrite shipped code only to suit the
  tool (#13583).
- **No GitHub Actions.** Checks run locally or on our own machines.
- **Shared OpenAgents files** (owner, 2026-11-04). App-owned persistent files
  live under `~/.openagents/`, including future desktop apps. The Metaverse
  Engine uses `~/.openagents/aethyrforge/`. Reusable audio/text model
  checkpoints live once under `~/.openagents/models/`; shared executables live
  in `~/.openagents/bin/`. Use the shared CLI inventory and path helpers rather
  than an app-private model download or `Library/Application Support`. Explicit
  path overrides remain valid.
- **No global filesystem or macOS TCC scans.** Keep searches scoped to the
  active workspace. Never run broad filesystem walks (`find`, `rg`, `ls -R`,
  `du`, `mdfind`, etc.) targeting `/`, `~`, `$HOME`, `/Users`, `/Volumes`,
  `~/Library`, or macOS TCC folders (`Desktop`, `Documents`, `Downloads`,
  `Photos`, etc.) outside cwd; this triggers macOS permission prompts and is
  blocked by `coax/lib/microcoder/git-guard.coax`.
- **No visible browsers.** Page checks and screenshots run in headless
  Chrome, which renders WebGPU: `coax/web/tool.sh shot URL OUT.png [WxH]
  [WAIT_MS] [--eval JS] [--keep]`, or the same flags (`--headless=new
  --mute-audio --enable-unsafe-webgpu --use-angle=metal`) in your own driver.
  Never open a window or take focus on the owner's screen, and never play sound
  on it: headless Chrome still plays a page's audio unless it is muted.
- **Take pictures the fast way** ([docs/metaverse/fast-shots.md](docs/metaverse/fast-shots.md)).
  `shot`, `clip` and `drive` share one headless browser per checkout, so
  don't start your own. Pass `--keep` on every shot of a build after the
  first: the page stays loaded, and a script plus a picture takes about a
  second instead of 12. The world carries over between `--keep` calls, so
  each `--eval` script sets what it needs. A rebuilt page is loaded again on
  its own. `WAIT_MS` is the longest wait, not a fixed one: the shot is taken
  as soon as the page is settled, so don't raise it to be safe. Pass
  `--exact-wait` only when the moment you want comes at a set time.
- **Bitcoin-only money.** Lightning, on-chain Bitcoin, Spark and Taproot
  Assets. No USDC, USDT, EVM, Solana, Tron or Cashu. The first wallet is
  Breez Spark ([docs/lightning/](docs/lightning/README.md)).
- **Secrets.** Never print or commit keys, tokens or mnemonics. Seed words
  stay in the Keychain and in memory, are never logged, and are shown once
  behind a warning. The committed Breez API key is a deliberate exception.
- **Protocol changes** start in `proto/` and the spec. Published contracts
  keep their identity: change one only with a new version, never in place.
  Every wire or stored shape has a row in `proto/registry.json` and one codec
  file; `coax/web/tool.sh wire-check` fails on a shape with neither
  ([how to register one](proto/README.md#register-a-wire-shape)).
- **Downloaded and cloned code is untrusted.** Read it; don't run its
  scripts on this Mac.
- **Pull requests from forks** ([review rules](docs/contributing/review.md)).
  Code from a fork is never built or run on a machine that holds secrets, a
  production login or a wallet, which is every team machine. Read the diff
  first with `gh pr diff N`, never `gh pr checkout` or `git checkout` a fork
  branch here (this clone's `core.hooksPath` runs the branch's hooks), and
  run it only in a throwaway sandbox with no credentials. A review agent
  posts a first review within one working day; only a maintainer, a person,
  merges, with Squash and merge so the contributor stays the commit's author.
- **Three rounds per problem.** After three failed rounds on one problem
  (a bug, a failing check, a build), change approach or ask a person. Don't
  run a fourth round of the same idea with small changes.

## Build and test

Before you build a Coax Native Web page or a realm, read
[coax/start-here.md](coax/start-here.md): the workflow, the drive commands
and the five costliest pitfalls. `coax vocab NAME` prints what a library
name, keyword, error code or foreign spelling is.

Heavy commands (`wasm.sh`, `build.sh`, `coax test` of a big unit, test suites)
go through `coax/remote/remote.sh run --` (or set `OA_HEAVY_REMOTE=1` for
`heavy.sh`). On the Mac it runs the command here, in the caller's checkout and
one of the Mac's two build slots, and sends it to a remote build host only
when the Mac is under heavy load: load over 0.75 a core, memory under 15%
free, the open-file table 50% full, or both slots held (owner, 2026-11-16).
Each call prints one line saying where it ran and why. `OA_REMOTE=1` always
uses a host, `OA_REMOTE=0` or `--local` always runs here. On a host it sends
the checkout's commit and uncommitted changes, streams the output, and brings
the written files back; with no host it stops with one line naming the cause
and the fix (`remote.sh status`) ([docs/metaverse/remote-builds.md](docs/metaverse/remote-builds.md)).

The API suite (`coax/web/api/test.sh`: Postgres, ASan, every route) still
never runs on the Mac: run it on a host with `OA_REMOTE=1`, whole or in two
parts side by side ([docs/metaverse/remote-builds.md](docs/metaverse/remote-builds.md),
"The API suite"):

```sh
OA_REMOTE=1 coax/remote/remote.sh run -- coax/web/api/test.sh            # the whole suite, about 16 minutes
OA_REMOTE=1 coax/remote/remote.sh run -- coax/web/api/test.sh --part 1/2 # and 2/2 in a second call at once: 13 minutes
OA_REMOTE=1 coax/remote/remote.sh run -- env API_TESTS_ONLY=auth_checks.coax coax/web/api/test.sh  # only that tests/ file (or test_X.py)
```

To serve or drive pages in a fresh worktree, fetch main's built pages
(`coax/remote/pages.sh origin/main`, about 10 seconds) and rebuild only the
module you changed with `wasm.sh`. Don't run a full
`apps/openagents-web/build.sh` for that.

```sh
coax/remote/pages.sh origin/main                       # main's built pages into this worktree (never a full build.sh)
coax/remote/remote.sh run -- coax/bin/coax test FILE   # any command below: here, or on a host when the Mac is busy
coax/build.sh test                          # compiler (from ~/.cache/coax-release, else the published build for this platform, else a bootstrap) + every Coax suite (over 15 minutes: use parts)
coax/tests/run.sh --part I/8                # after coax/build.sh: one eighth of the Coax suites (1/8 ... 8/8, each under 15 minutes)
coax/tests/run.sh --only UNIT...            # just those suites (--list names them)
COAX_MUTATION_FULL=1 coax/build.sh          # the full mutation check: every survivor gates (normally only changed lines)
coax/bin/coax test FILE                     # a file's checks, one line each as it ends, with its time
coax/bin/coax test FILE --list              # name them, run none
coax/bin/coax test FILE --only SUBSTR       # just the checks whose name contains SUBSTR (skips #authority coverage)
coax/bin/coax mutate FILE                   # are the checks sharp? parallel within the machine's build-worker budget
coax/bin/coax mutate FILE --root-only       # entry-source sites; all baseline checks still run
coax/bin/coax mutate FILE --changed         # only procedures changed since origin/main (or --changed REF): the gate
coax/bin/coax mutate FILE --unit A --unit B # several units, one baseline, one report each
coax/bin/coax mutate FILE --jobs N          # cap the workers (or COAX_MUTATE_JOBS=N, COAX_JOBS=N)
coax/bin/coax status --jobs                 # the shared build-worker budget and who holds it
coax/bin/coax serve status                  # the resident compiler c, check and test run in (it starts itself; COAX_SERVE=0 turns it off)
coax/bin/coax mutate FILE --shard I/N > I.rec  # one of N parts; then --merge 1.rec --merge 2.rec ...
apps/openagents-desktop/build.sh --minimal     # primary OpenAgents Desktop build (never --full unless requested)
apps/openagents-desktop/install.sh --minimal   # build origin/main (--minimal), verify with v1-bundle-check.coax, install /Applications/OpenAgents.app, sync resident daemon
coax/web/tool.sh apps/openagents-desktop/v1-bundle-check.coax PATH  # verify a --minimal desktop bundle's size, files and dynamic links
evals/openagents-desktop/run.sh                # desktop Tier 0, Tier 1 and compaction evals (--tier 2 / --tier 4 opt-in on local Inkling)
apps/openagents-ios/build.sh                # the iOS Antigravity-over-Tailscale app, for the simulator
apps/openagents-ios/check.sh                # verifies zero JS/WebKit/Metal, legacy GPU/AppCore/account/relay symbols or URLs
apps/openagents-cli/build.sh                # the openagents CLI
gamebuilder/scripts/check.sh                # GameBuilder core, worlds, CLI, layer-check and tools-check
psionic/check.sh --only UNIT...             # just those Psionic units: checks, mutation, native, sanitizers, book
psionic/check.sh                            # every Psionic unit: the release gate
psionic/tests/check_elephant_slices.sh DIR  # Elephant original-weight slices against recorded references, ~15 s
scripts/check-books.sh                      # metadata checks across all 8 books (or python3 psionic/tests/book.py --book NAME)
scripts/check.sh                            # outside-contributor local check suite
scripts/proto.sh check                      # schemas (needs PROTOC and PROTO_PYTHON)
python3 metaverse/test_home.py              # the home against real protobuf
metaverse/test_sanitized.sh                 # ASan/UBSan harness
metaverse/test_voice.sh [WAV...]            # voice engine, wire, floor, meter through voice.h (ASan/UBSan, protoc)
coax/tests/net/tls.sh [--public [URL]]      # wss://: tls-native.c under ASan/UBSan against local TLS endpoints (OpenSSL from Nix)
coax/web/serve.sh apps/openagents-web       # the website with hot reload: http://127.0.0.1:8800/
coax/web/tool.sh assets-sync --check        # verify lockfile-pinned binary assets (realms, psionic, gamebuilder, splicebots)
coax/web/tool.sh coax/web/wgsl-module.coax --site --check  # committed apps/openagents-web/*.wgsl match their Coax, and connect-pylon.wgsl is newland.wgsl (build.sh writes them; commit them; runs before a push)
coax/web/tool.sh coax/web/changelog-page.coax --check      # committed Coax changelog page matches coax/CHANGELOG.md (--write regenerates; runs before a push)
coax/web/tool.sh everglade-parity           # no Everglade zone carries its own copy of a shared feature (everglade/features.coax, #13075)
coax/web/tool.sh everglade/model/tools/zone-cells.coax --check everglade/world/zone-cells.coax everglade/content/everglade.content  # Everglade's cells and navigation are what the land builds, every spawn and named point is reachable (--write after a change to the land or a collider; docs/metaverse/navigation.md, #13281)
coax/web/tool.sh components-check           # the component system's layers, no looks in a component, one Markdown parser (components/spec.md, #13153)
coax/web/tool.sh everglade2-drift --page ORIGIN --room PORT  # Everglade II's page against a local room through a shaped network: your corrections and the others as drawn, against bounds (metaverse-engine/net/README.md "The drift check", #13277; a Mac with a GPU, after coax/web/room/local.coax --static)
coax/web/tool.sh layer-check                # no load from metaverse-engine/ up into metaverse/, apps/, everglade/ or coax/web/, none from everglade/ into apps/, no plain load of a tests/ file or of a checks file beside a module (`X-checks.coax`) by product code (a module loads its checks with `#load "FILE" #test;`, #13383); context loads match contexts.txt (#13317); no engine module without a product consumer or a line in metaverse-engine/consumers.txt (#13272); no authoring or engine context reaches the character record or the item ledger (the `durable` gate, #13431); no page links a room-only file (the `page-room` gate, #13440); the allowlist only shrinks (metaverse-engine/README.md "Layers", #13264)
coax/web/tool.sh prepush --replay COMMIT    # would the pre-push check refuse COMMIT? (--replay-check: the historical clobbers, #13265)
coax/web/tool.sh code-route-check           # a realm's code from the browser: the dev server's route lists, reads and writes only the page's own realm sources, and --static and the realm origin have no such route (metaverse-engine/INVARIANTS.md, #13421; after coax/web/serve.sh has built the server)
coax/web/tool.sh realm-frame-check          # a realm in a sandboxed frame on its own origin: the dev server with a realm origin, headless Chrome, the site framing Splice Bots (docs/metaverse/realm-origin.md, #13432; after coax/web/serve.sh has built the server)
coax/web/tool.sh agent-reference --write    # each realm's reference for agents, apps/openagents-web/realms/PAGE/llms.txt, from its wire, pins, content tables, limits and diagnostic codes; commit it (--check runs before a push, #13430)
coax/web/tool.sh wire-check                 # every wire shape is in proto/registry.json with one codec; the allowlist only shrinks (proto/README.md, #13236)
coax/web/tool.sh shot URL OUT.png 1280x720 --keep --eval JS  # a picture of a page; --keep reuses the loaded page (docs/metaverse/fast-shots.md)
coax/web/tool.sh shot --compare A.png B.png  # how much two pictures differ
coax/web/tool.sh shot URL OUT.png --eval "JSON.stringify(coaxial.loadMarks())"  # where a page's load time went (docs/metaverse/fast-loads.md)
```

Wait for a long command without polling. Run it in the foreground with a long
timeout (up to 10 minutes), or start it in the background and wait for its
completion notification rather than polling. Never loop on `sleep N`: in
the 36 hours to 2026-11-10, agents spent 32 hours in sleep polls, more than half
their tool time.

**Several small issues at once.** Write the code for all of them first, then
verify once. Five issues and an extra request took 2 h 15 min this way on
2026-11-10, against an hour or more each one at a time. To make that one
verification pass work:
1. Rebase onto `origin/main` before you verify, not after. Verifying an old base
   and then rebasing cost two extra rounds.
2. Fetch main's built pages (`coax/remote/pages.sh origin/main`) before you build
   modules, because `pages.sh` overwrites them.
3. Run suites through `coax/remote/check.sh only PATH`, never `tool.sh suites`
   directly on a host.
4. Group small suites into a few host jobs. Each job's snapshot and send can take
   minutes on busy hosts.
5. Fix what fails and rerun only that.
6. Commit one commit per issue, so each one reverts alone.

**Getting the compiler.** Never wait on a bootstrap you don't need.
`coax/build.sh` installs the published build of your tree's compiler in
seconds: it reads the release bucket, then the GitHub mirror (release tags
`coax-build-…`), which allowlisted cloud sessions can reach when the bucket is
refused. If it says the build is "missing" in both places, `main`'s compiler
changed after the last publish (the release hosts catch up within about an
hour and a half); to only use the compiler meanwhile, run
`COAX_MUTATION=0 coax/build.sh` in the background (no mutation gates, never
published) and work on something else. In a shallow clone, run
`git fetch --depth=2000 origin main` first: installing (published or
bootstrapped) checks the release documents, which name the changelog's commit,
and a clone without that commit is refused only after the build. Details:
`coax/RELEASE.md`, Published builds.

Keep iteration fast. Checks run while you work finish in minutes: test the
units you touched, use small slices and fixtures, and run long gates in the
background or in parallel rather than waiting on them. Never rerun a
whole-model decision on original weights on the CPU as a routine check;
compare against its recorded reference answer, or run it on the GPU path.
For model code, the Elephant slices check one linear layer, one
full-attention layer, the embedding and a readout chunk with the original
weights in seconds. `coax mutate` gives the same report for any `--jobs` or
shard split; on a change, mutate what it touched (`--changed`, or several
`--unit` in one run, which share one baseline); name the units you touched with
`--only` and leave the full `psionic/check.sh` for release.

**Check duration and progress.**

- **One command takes at most 15 minutes.** A check that would take longer
  gets split into slices, shards (`coax mutate --shard`) or a smaller unit,
  or runs on a faster path. It is never simply waited on.
- **Never repeat a passing result.** A mutation run, gate or test that
  passed on unchanged files is not rerun to "confirm". Cite the earlier
  result and its commit in the record.
- **Report progress at least every 30 minutes,** with what changed and
  what's left. Stop, don't wait, when a run exceeds its limit.

The full iOS test command and TestFlight steps are in
[apps/openagents-ios/README.md](apps/openagents-ios/README.md). Run the
tests for what you touched, not everything.
