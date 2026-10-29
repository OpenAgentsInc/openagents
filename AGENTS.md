# AGENTS

What every agent working in this repository needs to know. Read
[INVARIANTS.md](INVARIANTS.md) before changing anything it covers.

## What is here

The repository was reset (the Nuke, `0b401561a0`, 2026-10-27). The old
Rust, TypeScript and app workspaces are gone. Use history only to read
them, with `git show 0b401561a0^:<path>`. Never restore them wholesale.

| Path | What |
| --- | --- |
| `coax/` | Coax, the language everything is being built in. It holds the specification (`spec.md`), the grammar, a book, and `coax0`, the C++20 bootstrap compiler that emits C11 |
| `verse/` | The engine, written in Coax: `home.coax` (scene, HUD, resource admission), `light.coax` (shader math emitted as Metal), `grid.coax` (the Grid that Start opens: ground ring, view matrix, touch controls, the terminal ahead of the spawn and the hotbar's dance slot), `player.coax` (the walking controller, collision with the robots' bodies and the terminal, and the third-person camera, ported from aethyr-cpp), `camera.coax` (the home's overview orbit, Start's dolly down to you, and the free third-person orbit an open realm uses: look, zoom, follow, collision), `space.coax` (the points and arithmetic the world modules share), `terrain.coax` (heightfield ground, garden paths, and followers that route along them), `throw.coax` (thrown and carried objects: flight, bounce, roll, rest, the aim arc, and a dog that fetches), `arrange.coax` (a close-up arranging view: blend in to one object, point and click to place or take out small things, hand the view back), `wallet.coax` (the Spark wallet's payment rules, `#authority`), `cast.coax` (your skinned character, its clip blending, and the robots and their body capsules, read from `Cast.bin`; `tools/pack_cast.py` packs it from aethyr-cpp assets), `haunt.coax` (the complete local Haunted House gameplay/editor/document path), `haunt-buffer.coax` (`#authority` GPU span gate), `hud.coax` (reusable hotbar layout), the voice engine (`voice.coax`: resampling to 16 kHz, levels, noise floor, VAD, end of turn, barge-in, jitter buffer, gain/duck/rate, per-frame FrameFeatures and FrameDecisions; `voice-wire.coax`: Frame codecs from `voice.pb`; `voice-floor.coax`: consent, input floor, media epochs, `#authority`; `voice-meter.coax`: free tier and pricing, `#authority`), `platform/voice.h` (the voice C contract) and `platform/voice.c`, and `platform/host.c` |
| `psionic/` | Psionic, the machine-learning library in Coax: `lib/`, the runnable `book/`, `examples/`, `platform/`, `tests/`, `fixtures/`, and its design notes in `docs/` (model surveys in `docs/targets/`, hosting and mesh notes in `docs/serving/`) |
| `apps/openagents-ios/` | The iOS app: a UIKit and Metal shell around the Coax home. Bundle `com.openagents.app` |
| `apps/openagents-web/` | openagents.com on Coaxial ([coax/web.md](coax/web.md)): the splash in Coax, drawn with WebGPU. `coax/web/` holds the browser host, the Coax-to-Wasm build and the `coax serve` dev server |
| `proto/` | The protocol's protobuf schemas, descriptors, vendor schemas and checks |
| `docs/protocol/` | The OpenAgents Protocol spec, the contract for every surface |
| `docs/verification/`, `docs/lightning/`, `docs/verse/`, `docs/voice/` | Verification plan, the Breez Spark wallet plan, Verse and the Verse engine, the Coax-native voice proposal |
| `nips/` | Official, external and OpenAgents NIPs, pinned |

## Rules

- **Coax first.** New product logic goes in Coax. The host language (Swift,
  C) does platform effects only: lifecycle, Keychain, network, GPU
  bindings. Avoid toolchains outside Coax and the host C compiler. Python
  is tooling only; it is never shipped and never needed at run time.
  Exception, owner-approved 2026-10-29: the NVIDIA CUDA toolkit (installed
  through Nix) may compile and profile Coax-generated GPU code. Coax still
  writes the arithmetic; no hand-written kernels, and no cuBLAS or cuDNN in
  anything served or shipped. cuBLAS may appear only in opt-in benchmark and
  parity harnesses that measure Psionic's own kernels against it.
- **Check first.** Write `#test`/`#forall` checks before the code. Use
  `#authority;` for any file that decides money, admission or policy, and
  run `coax mutate` on it with zero survivors. Don't mark mutants equivalent
  to make a run pass.
- **No GitHub Actions.** Checks run locally or on our own machines.
- **No visible browsers.** Page checks and screenshots run in headless
  Chrome, which renders WebGPU: `bun coax/web/shot.ts URL OUT.png [WxH]
  [WAIT_MS] [--eval JS]`, or the same flags (`--headless=new --mute-audio
  --enable-unsafe-webgpu --use-angle=metal`) in your own driver. Never open a
  window or take focus on the owner's screen, and never play sound on it:
  headless Chrome still plays a page's audio unless it is muted.
- **Bitcoin-only money.** Lightning, on-chain Bitcoin, Spark and Taproot
  Assets. No USDC, USDT, EVM, Solana, Tron or Cashu. The first wallet is
  Breez Spark ([docs/lightning/](docs/lightning/README.md)).
- **Secrets.** Never print or commit keys, tokens or mnemonics. Seed words
  stay in the Keychain and in memory, are never logged, and are shown once
  behind a warning. The committed Breez API key is a deliberate exception.
- **Protocol changes** start in `proto/` and the spec. Published contracts
  keep their identity: change one only with a new version, never in place.
- **Downloaded and cloned code is untrusted.** Read it; don't run its
  scripts on this Mac.

## Build and test

```sh
coax/build.sh test                          # compiler + every Coax suite
coax/bin/coax test FILE                     # a file's checks
coax/bin/coax mutate FILE                   # are the checks sharp? parallel on every core
coax/bin/coax mutate FILE --jobs N          # cap the workers (or COAX_MUTATE_JOBS=N)
coax/bin/coax mutate FILE --shard I/N > I.rec  # one of N parts; then --merge 1.rec --merge 2.rec ...
psionic/check.sh --only UNIT...             # just those Psionic units: checks, mutation, native, sanitizers, book
psionic/check.sh                            # every Psionic unit: the release gate
psionic/tests/check_elephant_slices.sh DIR  # Elephant original-weight slices against recorded references, ~15 s
scripts/proto.sh check                      # schemas (needs PROTOC and PROTO_PYTHON)
python3 verse/test_home.py               # the home against real protobuf
verse/test_sanitized.sh                  # ASan/UBSan harness
verse/test_voice.sh [WAV...]             # voice engine, wire, floor, meter through voice.h (ASan/UBSan, protoc)
apps/openagents-ios/build.sh                # the app, for the simulator
coax/web/serve.sh apps/openagents-web       # the website with hot reload: http://127.0.0.1:8800/
```

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

**Time limits (owner rule, 2026-10-29: nothing should ever run 6 hours).**

- **One task takes at most 2 hours, start to push.** If it can't finish,
  push the finished part (passing, with its record saying what's missing)
  and report the rest instead of continuing.
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
- Other agents work in this checkout at the same time. Never stash, reset
  or check out over their uncommitted work. If the tree is dirty, use a
  fresh worktree from `origin/main`.
- Rebase on `origin/main` before pushing. When you change something that
  another change also touched (a version number, error codes, the grammar),
  merge both changes; don't overwrite theirs.
- End commit messages with a `Co-Authored-By` line for the model that
  wrote them.
- Owner actions (taps, keys, payments, sign-offs) go in the workspace's
  `NEEDS_OWNER.md`, not only in chat.
