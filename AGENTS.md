# AGENTS

What every agent working in this repository needs to know. Read
[INVARIANTS.md](INVARIANTS.md) before changing anything it covers, and follow
the [root glossary](docs/books/glossary.md) for new identifiers, tables, registry IDs and wire fields.

## What is here

The repository was reset (the Nuke, `0b401561a0`, 2026-10-27). The old
Rust, TypeScript and app workspaces are gone. Use history only to read
them, with `git show 0b401561a0^:<path>`. Never restore them wholesale.

| Path | What |
| --- | --- |
| `coax/` | Coax, the language everything is being built in. It holds the specification (`spec.md`), the grammar, a book, and `coax0`, the C++20 bootstrap compiler that emits C11 |
| `aethyr/` | Aethyr, the engine every realm runs on, in Coax: the engine-core modules (GitHub #12609; `loop.coax` first, see `README.md`), the engine modules moved from `verse/` (player, camera, figure, contact, terrain, stand, sticks, points and the rest, listed in its README), and `book/`, which builds the engine from its smallest primitives, one checked piece per chapter, through physics, big worlds, Aethyrforge and every realm (design drafts first; check with `python3 psionic/tests/book.py --book aethyr`). Aethyrforge, the realm builder, builds on it |
| `verse/` | The Verse's realms and their content, written in Coax (Aethyr's engine modules moved to root `aethyr/`, GitHub #12609): `home.coax` (scene, HUD, resource admission), `light.coax` (shader math emitted as Metal), `grid.coax` (the Grid that Start opens: ground ring, view matrix, the touch controls drawn, the terminal ahead of the spawn and the hotbar's dance slot), `gridwalk.coax` (the web Grid's session on Aethyr's engine core: you, the thumbs and the record each step replays from), `wallet.coax` (the Spark wallet's payment rules, `#authority`), `cast.coax` (your skinned character, its clip blending, and the robots and their body capsules, read from `Cast.bin`; `tools/pack_cast.py` packs it from aethyr-cpp assets), `haunt.coax` (the complete local Haunted House gameplay/editor/document path), `haunt-buffer.coax` (`#authority` GPU span gate), `hud.coax` (reusable hotbar layout), the voice engine (`voice.coax`: resampling to 16 kHz, levels, noise floor, VAD, end of turn, barge-in, jitter buffer, gain/duck/rate, per-frame FrameFeatures and FrameDecisions; `voice-wire.coax`: Frame codecs from `voice.pb`; `voice-floor.coax`: consent, input floor, media epochs, `#authority`; `voice-meter.coax`: free tier and pricing, `#authority`), `platform/voice.h` (the voice C contract) and `platform/voice.c`, and `platform/host.c` |
| `components/` | The component system ([spec](components/spec.md), GitHub #13153): one set of UI components in Coax for every app, in layers. `core/` (state and rules: the one Markdown parser, text, colour, syntax roles), `layout/` (measuring in pixels or terminal cells, the one line wrapper), `paint/` (the display list), `skin/` (tokens and frames: `zeron`, `everglade`, `verse`, `terminal`), `backend/` (`gpu.coax` and the one shader, `terminal.coax`), `kit/` (the components), `gallery/`, and `zeron/` (the chat set still moving onto the layers). Never write a second Markdown parser, line wrapper or edit buffer outside it; check with `coax/web/tool.sh components-check` |
| `psionic/` | Psionic, the machine-learning library in Coax: `lib/`, the runnable `book/`, `examples/`, `platform/`, `tests/`, `fixtures/`, and its design notes in `docs/` (model surveys in `docs/targets/`, hosting and mesh notes in `docs/serving/`) |
| `apps/openagents-ios/` | The iOS app: a UIKit and Metal shell around the Coax home. Bundle `com.openagents.app` |
| `apps/openagents-web/` | openagents.com on Coaxial ([coax/web.md](coax/web.md)): the splash in Coax, drawn with WebGPU. `coax/web/` holds the browser host, the Coax-to-Wasm build, the `coax serve` dev server, and the HTTP API ([coax/web/api/COAX.md](coax/web/api/COAX.md): new API rules go in `#authority` `*-policy.coax`, SQL-rules routes move when next changed) |
| `apps/openagents-cli/` | The `openagents` command line in Coax ([plan](docs/cli/2026-11-01-openagents-cli-v1.md)): `nostr` (your NIP-06 identity in a 0600 key file, never the Keychain unless `OPENAGENTS_KEYCHAIN=1`; signing and NIP-98) and `pylon` (start, stop, status and logs around `scripts/join-pylon.sh`). Build with `apps/openagents-cli/build.sh` |
| `components/` | The one component system for every app and realm ([spec](components/spec.md), GitHub #13153). Today it holds `zeron/`, the chat components (GPUI's drawing model, the composer, Markdown, the transcript, tool chips, pickers) that openagents.com/chat, the Mac app and Aethyrforge draw with; the spec's layers and skins are not built yet |
| `os/` | Verse OS ([spec](docs/os/2026-11-02-verse-os.md)), which CoderOS becomes: the machine's programs in Coax (the compositor, desk, camera and hands, recording, CPU limits, the update agent, and the Verse OS session: `verse-session.coax` with `verse-boot`, `verse-agents`, `verse-panel`, `verse-hands` and the `#authority` `verse-pylon-policy`), and `os/nix/`, the NixOS modules, the realms package and the image (owner-approved 2026-11-02: the one place Nix lives here) |
| `proto/` | The protocol's protobuf schemas, descriptors, vendor schemas and checks |
| `docs/protocol/` | The OpenAgents Protocol spec, the contract for every surface |
| `docs/verification/`, `docs/lightning/`, `docs/verse/`, `docs/voice/` | Verification plan, the Breez Spark wallet plan, Verse and the Verse engine, the Coax-native voice proposal |
| `nostr/` | Nostr: `nips/` (official, external and OpenAgents NIPs, pinned) and `book/`, *Nostr From Scratch*, which builds every primitive, the client, the relay and every NIP in Coax (implemented chapters cite the existing `coax/lib/crypto/`, `coax/lib/nostr/` and `coax/lib/net/` code; check with `python3 psionic/tests/book.py --book nostr`; epic #12612) |

## Shared asset library

Read [the asset registry usage guide](assets/registry/README.md) before importing,
using or remixing Quaternius assets. The portable catalog is `assets/registry/`;
originals, immutable revisions and editable Blender forks live under
`~/.openagents/assets/`. The shared command is `~/.openagents/bin/asset-registry`.
Use stable asset IDs and pinned collections; preserve source provenance and
Character Create's frozen save keys. Do not add new app-private source copies.

## Rules

- **Never hand-roll what already exists** (owner, 2026-11-11;
  [docs/reuse.md](docs/reuse.md)). Before writing a text box, button, undo
  stack, camera, controller, terrain, parser, hash or model call, find the
  shared one (`components/`, `aethyr/`, `everglade/`, `coax/lib/`,
  `psionic/`) in docs/reuse.md and use it. If it falls short, extend the
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
  `#authority;` for any file that decides money, admission or policy, and
  run `coax mutate` on it with zero survivors. Don't mark mutants equivalent
  to make a run pass.
- **No GitHub Actions.** Checks run locally or on our own machines.
- **Shared OpenAgents files** (owner, 2026-11-04). App-owned persistent files
  live under `~/.openagents/`, including future desktop apps. Aethyrforge uses
  `~/.openagents/aethyrforge/`. Reusable audio/text model checkpoints live once
  under `~/.openagents/models/`; shared executables live in `~/.openagents/bin/`.
  Use the shared CLI inventory and path helpers rather than an app-private model
  download or `Library/Application Support`. Explicit path overrides remain valid.
- **No visible browsers.** Page checks and screenshots run in headless
  Chrome, which renders WebGPU: `coax/web/tool.sh shot URL OUT.png [WxH]
  [WAIT_MS] [--eval JS] [--keep]`, or the same flags (`--headless=new --mute-audio
  --enable-unsafe-webgpu --use-angle=metal`) in your own driver. Never open a
  window or take focus on the owner's screen, and never play sound on it:
  headless Chrome still plays a page's audio unless it is muted.
- **Take pictures the fast way** ([docs/verse/fast-shots.md](docs/verse/fast-shots.md)).
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
- **Three rounds per problem.** After three failed rounds on one problem
  (a bug, a failing check, a build), change approach or ask a person. Don't
  run a fourth round of the same idea with small changes.

## Build and test

Before you build a Coaxial page or a realm, read
[coax/start-here.md](coax/start-here.md): the workflow, the drive commands
and the five costliest pitfalls. `coax vocab NAME` prints what a library
name, keyword, error code or foreign spelling is.

Heavy commands (wasm.sh, build.sh, `coax test` of a big unit, test suites)
run on a remote build host, not the Mac: prefix them with
`coax/remote/remote.sh run --` (or set `OA_HEAVY_REMOTE=1` for heavy.sh). It
sends the checkout's commit and uncommitted changes, streams the output, and
brings the written files back; with no host it runs on the Mac's slots
(docs/verse/remote-builds.md).

The API suite (`coax/web/api/test.sh`: Postgres, ASan, every route) is one of
them. Never run it on the Mac; run it on a host, whole or in two parts side
by side (docs/verse/remote-builds.md, "The API suite"):

```sh
coax/remote/remote.sh run -- coax/web/api/test.sh            # the whole suite, about 16 minutes
coax/remote/remote.sh run -- coax/web/api/test.sh --part 1/2 # and 2/2 in a second call at once: 13 minutes
coax/remote/remote.sh run -- env API_TESTS_ONLY=auth_checks.coax coax/web/api/test.sh  # only that tests/ file (or test_X.py)
```

To serve or drive pages in a fresh worktree, fetch main's built pages
(`coax/remote/pages.sh origin/main`, about 10 seconds) and rebuild only the
module you changed with `wasm.sh`. Don't run a full
`apps/openagents-web/build.sh` for that.

```sh
coax/remote/pages.sh origin/main                       # main's built pages into this worktree (never a full build.sh)
coax/remote/remote.sh run -- coax/bin/coax test FILE   # any command below, on a build host
coax/build.sh test                          # compiler (from ~/.cache/coax-release, else the published build for this platform, else a bootstrap) + every Coax suite (over 15 minutes: use parts)
coax/tests/run.sh --part I/8                # after coax/build.sh: one eighth of the Coax suites (1/8 ... 8/8, each under 15 minutes)
coax/tests/run.sh --only UNIT...            # just those suites (--list names them)
COAX_MUTATION_FULL=1 coax/build.sh          # the full mutation check: every survivor gates (normally only changed lines)
coax/bin/coax test FILE                     # a file's checks, one line each as it ends, with its time
coax/bin/coax test FILE --list              # name them, run none
coax/bin/coax test FILE --only SUBSTR       # just the checks whose name contains SUBSTR (skips #authority coverage)
coax/bin/coax mutate FILE                   # are the checks sharp? parallel within the machine's build-worker budget
coax/bin/coax mutate FILE --root-only       # entry-source sites; all baseline checks still run
coax/bin/coax mutate FILE --jobs N          # cap the workers (or COAX_MUTATE_JOBS=N, COAX_JOBS=N)
coax/bin/coax status --jobs                 # the shared build-worker budget and who holds it
coax/bin/coax serve status                  # the resident compiler c, check and test run in (it starts itself; COAX_SERVE=0 turns it off)
coax/bin/coax mutate FILE --shard I/N > I.rec  # one of N parts; then --merge 1.rec --merge 2.rec ...
psionic/check.sh --only UNIT...             # just those Psionic units: checks, mutation, native, sanitizers, book
psionic/check.sh                            # every Psionic unit: the release gate
psionic/tests/check_elephant_slices.sh DIR  # Elephant original-weight slices against recorded references, ~15 s
scripts/proto.sh check                      # schemas (needs PROTOC and PROTO_PYTHON)
python3 verse/test_home.py               # the home against real protobuf
verse/test_sanitized.sh                  # ASan/UBSan harness
verse/test_voice.sh [WAV...]             # voice engine, wire, floor, meter through voice.h (ASan/UBSan, protoc)
coax/tests/net/tls.sh [--public [URL]]   # wss://: tls-native.c under ASan/UBSan against local TLS endpoints (OpenSSL from Nix)
apps/openagents-ios/build.sh                # the app, for the simulator
coax/web/serve.sh apps/openagents-web       # the website with hot reload: http://127.0.0.1:8800/
coax/web/tool.sh coax/web/wgsl-module.coax --site --check  # committed apps/openagents-web/*.wgsl match their Coax (build.sh writes them; commit them)
coax/web/tool.sh everglade-parity           # no Everglade zone carries its own copy of a shared feature (everglade/features.coax, #13075)
coax/web/tool.sh components-check           # the component system's layers, no looks in a component, one Markdown parser (components/spec.md, #13153)
coax/web/tool.sh layer-check                # no load from aethyr/ up into verse/, apps/, everglade/ or coax/web/, none from everglade/ into apps/, no product load of tests/; the allowlist only shrinks (aethyr/README.md "Layers", #13264)
coax/web/tool.sh prepush --replay COMMIT    # would the pre-push check refuse COMMIT? (--replay-check: the historical clobbers, #13265)
coax/web/tool.sh wire-check                 # every wire shape is in proto/registry.json with one codec; the allowlist only shrinks (proto/README.md, #13236)
coax/web/tool.sh shot URL OUT.png 1280x720 --keep --eval JS  # a picture of a page; --keep reuses the loaded page (docs/verse/fast-shots.md)
coax/web/tool.sh shot --compare A.png B.png  # how much two pictures differ
coax/web/tool.sh shot URL OUT.png --eval "JSON.stringify(coaxial.loadMarks())"  # where a page's load time went (docs/verse/fast-loads.md)
```

Wait for a long command without polling. Run it in the foreground with a long
timeout (up to 10 minutes). If it runs longer, start it in the background and wait
with the Monitor tool on an until-loop over its log. Never loop on `sleep N`: in
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
shard split, so zero survivors stays the rule; name the units you touched with
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

## Git

- Work on `main`. Commit only the files you changed, and push to `main`.
- **Never edit files in the main checkout**
  (`/Users/christopherdavid/work/openagents`). It stays clean on `main` so
  anyone can `git pull --ff-only` it. Do every edit, build and test in your
  own worktree: `git worktree add ../oa-<topic> origin/main`, then push
  `HEAD:main` from there and remove the worktree when you finish.
- If you find uncommitted changes in the main checkout, they are another
  agent's. Never stash, reset or check out over them; ask that agent to
  move them to a worktree.
- Rebase on `origin/main` before pushing. When you change something that
  another change also touched (a version number, error codes, the grammar),
  merge both changes; don't overwrite theirs.
- End commit messages with a `Co-Authored-By` line for the model that
  wrote them, and an `Agent: <id>` line with your agent ID (the one your
  brief gives you), so a failure on `main` reaches the agent that caused it
  ([docs/verse/green-main-pipeline.md](docs/verse/green-main-pipeline.md)).
- A pre-push check runs on every push to `main` (it is not a queue: about
  10 to 20 seconds, never more than two minutes). It refuses a commit that
  undoes another agent's work of the last 24 hours (a tree written from a
  stale snapshot), a commit with no `Agent:` line, and a failing
  `tools-check`, `everglade-parity`, `wire-check`, `layer-check` or touched
  suite. The hooks are tracked in `coax/remote/hooks/`; a clone turns them on
  once with `git config core.hooksPath coax/remote/hooks` (this Mac's clone
  has it, so every worktree gets it). When a refusal is right, fix the
  commit; when you mean to delete or restore another agent's file, name the
  file (or its folder/, or the commit you undo) in the message. Emergency
  only: `OA_PREPUSH_BYPASS="why" git push ...`, which is logged. Never use
  `--no-verify` ([the pre-push check](docs/verse/green-main-pipeline.md#the-pre-push-check)).
- `coax/remote/after.sh state` shows whether `main` is green: `main`,
  `main-green` (the newest head that passed the after-landing check), the last
  runs and the red list. When `main-green` is behind and you need a base that
  passes, start from it: `git worktree add ../oa-<topic> origin/main-green`.
- The allowlist counts (`coax/remote/counts.sh show`: parity known and gaps,
  components to move, the wire allowlist, `tools-port.txt`, layer-check per
  rule) only fall: a head that raises one is red and names its commit. A
  commit that must add a line also says why in a trailer,
  `Rebaseline: NAME: why`. A head worse than production in Everglade II's
  room pair (`check.sh e2`) is red too
  ([the comparing gates](docs/verse/green-main-pipeline.md#the-comparing-gates)).
- Before you investigate a failure on `main`, check the red list (in that
  output). A failure listed there is already owned. When an entry names your
  commit, fixing it comes before new work: a culprit still failing 30 minutes
  after it is reported is reverted.
- Owner actions (taps, keys, payments, sign-offs) go in the workspace's
  `NEEDS_OWNER.md`, not only in chat.
