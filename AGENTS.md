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
| `metareal/` | The engine, written in Coax: `home.coax` (scene, HUD, resource admission), `light.coax` (shader math emitted as Metal), and `platform/host.c` |
| `apps/openagents-ios/` | The only app: a UIKit and Metal shell around the Coax home. Bundle `com.openagents.app` |
| `proto/` | The protocol's protobuf schemas, descriptors, vendor schemas and checks |
| `docs/protocol/` | The OpenAgents Protocol spec, the contract for every surface |
| `docs/verification/`, `docs/lightning/`, `docs/verse/` | Verification plan, the Breez Spark wallet plan, Verse and the Metareal engine |
| `nips/` | Official, external and OpenAgents NIPs, pinned |

## Rules

- **Coax first.** New product logic goes in Coax. The host language (Swift,
  C) does platform effects only: lifecycle, Keychain, network, GPU
  bindings. Avoid toolchains outside Coax and the host C compiler. Python
  is tooling only; it is never shipped and never needed at run time.
- **Check first.** Write `#test`/`#forall` checks before the code. Use
  `#authority;` for any file that decides money, admission or policy, and
  run `coax mutate` on it with zero survivors. Don't mark mutants equivalent
  to make a run pass.
- **No GitHub Actions.** Checks run locally or on our own machines.
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
coax/bin/coax mutate FILE                   # are the checks sharp?
scripts/proto.sh check                      # schemas (needs PROTOC and PROTO_PYTHON)
python3 metareal/test_home.py               # the home against real protobuf
metareal/test_sanitized.sh                  # ASan/UBSan harness
apps/openagents-ios/build.sh                # the app, for the simulator
```

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
