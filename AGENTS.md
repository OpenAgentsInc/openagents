# AGENTS

What every agent working in this repository needs to know. Read [INVARIANTS.md](INVARIANTS.md) and the per-context invariants file for the area you touch, and add new invariants there. Follow [docs/books/glossary.md](docs/books/glossary.md) for new identifiers, tables, registry IDs and wire fields. Full directory layout & command reference: [docs/REPO-MAP.md](docs/REPO-MAP.md).

## Critical Contract (Read First)

- **Worktree isolation & canonical `origin/main`.** **Never edit files in the main checkout (`/Users/christopherdavid/work/openagents`).** Always `git fetch origin main` first, inspect canonical files via `origin/main` (not a potentially lagging local checkout), edit/build/test in your own worktree (`git worktree add ../oa-<topic> origin/main`; APFS-clone `coax/bin` with `cp -c -R` if helpful), rebase onto `origin/main`, push (`git push origin HEAD:main` unless a PR/branch was explicitly requested), and remove temporary worktrees/branches when finished. Never stash, reset, or overwrite uncommitted changes in the main checkout.
- **Done means the real installed thing works end-to-end.** Never declare done on compile checks, unit tests, static offscreen PNGs, synthetic stubs, or the wrong worker (`mcworker` when `--minimal` ships `inky`). Exercise the real user flow (real clicks/inputs/focus, live RPCs against real user state, cold `req#1` streaming, installed `/Applications/OpenAgents.app/Contents/Info.plist` `OACommit`/`OAProfile`). Never hide a user-requested core flow behind a feature flag unless explicitly asked.
- **OpenAgents Desktop (`--minimal` primary) & paired deploys.** Always build/install `--minimal` (`apps/openagents-desktop/build.sh` / `install.sh` default to `--minimal`; **never** build `--full` unless explicitly asked). Commit and push to `main` FIRST, then run `apps/openagents-desktop/install.sh` (never hand-copy an uncommitted branch build into `/Applications/OpenAgents.app` — the 15-min LaunchAgent overwrites it from `origin/main`). Check installed version via `Info.plist` (`OACommit`, `OAProfile`), never `ls -ld .app`. Mobile protocol/flow changes must merge and reinstall the Mac Desktop + `openagents-environment` daemon too.
- **Commit trailers.** Stage only files you changed. End every commit message with `Co-Authored-By: <model>` and `Agent: <id>`. Never use `--no-verify`. Check `git show --stat` before pushing so no critical file (`AGENTS.md`, `INVARIANTS.md`, `contexts.txt`) is accidentally emptied or clobbered.
- **Execution hygiene (zero hangs, zero polling, bounded output).**
  - **No polling:** Never loop on `manage_task(status)` or `sleep N`. Run commands with an explicit timeout or start in the background and stop to wait for reactive completion notifications.
  - **Mandatory timeouts + watchdogs:** Every live test, subprocess, or server probe MUST have a hard OS/script timeout (e.g., `timeout 60` or `select`/deadline). Never yield on an unbounded background test without a `schedule` watchdog timer.
  - **Safe probes:** Never run `agy` / OAuth diagnostic commands without `SSH_CONNECTION=1` (macOS `open` ignores `$BROWSER` and hijacks Chrome).
  - **Bounded output & targeted reads:** Pipe verbose shell commands (`| head -n 80`, `--stat`); use `rg`/`git grep` instead of dumping whole files; avoid re-reading unchanged lines.
  - **Clarify upfront, don't trail:** When requirements are genuinely ambiguous, ask structured questions upfront (e.g., `ask_question`) rather than guessing or appending trailing `"Would you like me to...?"` offers.
  - **Long sessions:** Persist baseline/delta metrics in a scratch ledger (`*_log.md`/`.json`) so context compactions lose zero state; after ~4–6 compactions or a shipped milestone, write a concise handoff and recommend a fresh chat.
- **No third-party branding leaks.** Never display third-party/retired product names or logos (e.g., `Zeron`, Anthropic logo/API key next to Inkling) in OpenAgents UI or docs.

## Git & Green-Main Pipeline

- Work on `main` (unless the user explicitly asks for a PR / side branch). Commit only the files you changed, rebase onto `origin/main` before verifying and pushing, merge concurrent edits cleanly (version numbers, error codes, grammar), and remove temporary worktrees/branches.
- Outside contributors work from a fork and open a PR ([CONTRIBUTING.md](CONTRIBUTING.md)); their local gate is `scripts/check.sh`.
- **Hooks (`coax/remote/hooks/`):** Enabled once per clone via `git config core.hooksPath coax/remote/hooks` (inherited by worktrees).
  - `prepare-commit-msg` auto-appends `Agent: <id>` when `OA_AGENT_ID` / harness env is set; `commit-msg` rejects missing `Agent:` immediately at commit time (`OA_PREPUSH_HUMAN=1` for humans).
  - `pre-push` (~10–20s, max 2m) refuses: stale-snapshot clobbers undoing another agent's work from the last 24h (excused when your commit message names the file/folder/commit), missing `Agent:` trailer, critical-file wipes/truncations (`AGENTS.md`, `INVARIANTS.md`, `contexts.txt` unless `Allow-Truncate: PATH: why`), and failing `tools-check`, `everglade-parity`, `wire-check`, `layer-check`, `state-keep`, `agent-reference --check`, `drive-hit-check`, `wgsl-module --site --check`, `changelog-page --check`, `coax check` on roots above changed `.coax` files, or touched suites (`coax test` + `coax mutate --changed` on `#authority` suites). Emergency only: `OA_PREPUSH_BYPASS="why" git push ...` (logged). Never `--no-verify`. Details: [docs/metaverse/green-main-pipeline.md](docs/metaverse/green-main-pipeline.md).
- `coax/remote/after.sh state` shows `main`, `main-green`, recent runs, and the red list. When `main-green` is behind and you need a passing base, branch from `origin/main-green`.
- Allowlist counts (`coax/remote/counts.sh show`: `parity-known`, `parity-gaps`, `components-to-move`, `wire-allowlist`, `tools-port.txt`, `layer-check`, `sql-rules-routes`) only fall. Raising one requires `Rebaseline: NAME: why`.
- Check the red list before investigating a `main` failure. If it names your commit, fixing it takes priority over new work (unfixed culprits are reverted after 30 minutes).
- Owner actions (taps, keys, payments, sign-offs) go in `NEEDS_OWNER.md`, not only in chat.

## Repository Map

The repository was reset in the Nuke (`0b401561a0`, 2026-10-27); never restore old Rust/TypeScript workspaces wholesale (read history with `git show 0b401561a0^:<path>`). Top-level areas: `coax/` (language, compiler, stdlib, web host, remote pipeline), `psionic/` (ML/Metal/CUDA), `coder/` (Inky/Agy/Host workers), `apps/openagents-desktop/` (`--minimal` macOS/Linux/Win app), `apps/openagents-ios/` (SwiftUI Antigravity-over-Tailscale), `apps/openagents-web/`, `apps/openagents-cli/`, `apps/shared/`, `components/`, `metaverse-engine/`, `metaverse/`, `everglade/`, `gamebuilder/`, `rules/`, `pay/`, `bitcoin/`, `nostr/`, `openagents/`, `os/`, `proto/`, `evals/`, `assets/`, `docs/`, `.agents/`. Full table, shared asset registry (`coax/web/tool.sh assets-sync`), and command index: [docs/REPO-MAP.md](docs/REPO-MAP.md).

## Engineering Rules

- **Done means the real thing works (owner, 2026-11-20).**
  - **Can't do the real thing? Say so.** Missing weights, hardware, secrets, access or time is a blocker: report it and stop, or keep the issue open with what is left. Never swap in a smaller task under the original's name.
  - **Close an issue only when every acceptance box holds**, citing the exact verification command and real output. Don't mark a chapter `implemented` or expose a model/feature until it actually runs.
  - **No UI that lies.** Never write a fake marker file, flag, or state showing something installed/ready when it isn't.
  - **Report only what you ran, with its real result.** A killed, timed-out, deferred, or skipped check did not pass; name it honestly.
  - `/goal` chats in OpenAgents Desktop verify `[GOAL_COMPLETE]` with one extra turn that re-reads issues and runs the real thing (`apps/openagents-desktop/page/goal.coax`).
- **Never hand-roll what already exists** ([docs/reuse.md](docs/reuse.md)). Reuse or extend `components/`, `metaverse-engine/`, `everglade/`, `coax/lib/`, `psionic/` — no second Markdown parser, line wrapper, edit buffer, or UI chrome copy (`coax/web/tool.sh components-check`).
- **Tools are Coax too** ([coax/tools-in-coax.md](coax/tools-in-coax.md)). Write build scripts, checks, harnesses, and CLIs in Coax (`coax/web/tool.sh FILE.coax`). Any new `.ts`/`.py`/`.js`/`.sh`/`.cjs` file needs a header line `Firm reason: N (...)` in its first 60 lines; verify with `coax/web/tool.sh tools-check`.
- **Coax first.** Product logic lives in Coax; Swift/C handle platform effects only (lifecycle, Keychain, network, GPU bindings). Python is tooling only, never shipped. Approved exceptions (2026-10-29): NVIDIA CUDA toolkit via Nix compiles Coax-generated GPU code (no hand-written kernels; cuBLAS only in opt-in parity/bench harnesses); host C links platform TLS / OpenSSL 3 for socket encryption only (WebSocket/Nostr/NIP-42/NIP-44 stay in Coax).
- **Check first & `#authority`.** Write `#test`/`#forall` checks before the code. Use `#authority;` for money, admission, or policy rules; mutate touched procedures (`coax mutate FILE --changed origin/main`) to zero surviving mutants (mark true equivalents `// mutant: equivalent` with a checkable reason; never rewrite shipped code merely to suit the tool).
- **No GitHub Actions.** Checks run locally or on our own build hosts.
- **Shared OpenAgents files.** App-owned persistent files live under `~/.openagents/` (`~/.openagents/aethyrforge/`, `~/.openagents/models/`, `~/.openagents/bin/`). Explicit path overrides remain valid.
- **No global filesystem or macOS TCC scans.** Scope all searches to the active workspace; never run broad walks (`find`, `rg`, `ls -R`, `du`, `mdfind`) targeting `/`, `~`, `$HOME`, `/Users`, `/Volumes`, `~/Library`, or Desktop/Documents/Downloads/Photos outside cwd (blocked by `coax/lib/microcoder/git-guard.coax`).
- **Headless browsers only (`--keep`).** Use `coax/web/tool.sh shot URL OUT.png [WxH] [WAIT_MS] [--eval JS] [--keep]` (or `--headless=new --mute-audio --enable-unsafe-webgpu --use-angle=metal`). Never open visible windows, steal focus, or play audio. Pass `--keep` after the first shot ([docs/metaverse/fast-shots.md](docs/metaverse/fast-shots.md)).
- **Bitcoin-only money.** Lightning, on-chain Bitcoin, Spark (`docs/lightning/README.md`), Taproot Assets. No USDC, USDT, EVM, Solana, Tron, or Cashu.
- **Secrets.** Never print or commit keys, tokens, or mnemonics (seed words stay in Keychain/memory; committed Breez API key is a deliberate exception).
- **Protocol & wire registry.** Start protocol changes in `proto/` and the spec; version new contracts instead of mutating in place; register every wire/stored shape in `proto/registry.json` (`coax/web/tool.sh wire-check`).
- **Untrusted code & fork PRs.** Never run scripts from downloaded/cloned repos on this Mac. Review fork PRs with `gh pr diff N` only — never `gh pr checkout` on a team machine (`core.hooksPath` runs branch hooks). Only a human maintainer squash-merges fork PRs ([docs/contributing/review.md](docs/contributing/review.md)).
- **Three rounds per problem.** After three failed rounds on one bug/build/check, change approach or ask a person instead of repeating small tweaks.

## Build, Test & Iteration Speed

- **Start here for Coax:** [coax/start-here.md](coax/start-here.md). Run `./coax/bin/coax vocab NAME` or `./coax/bin/coax explain CODE` before guessing on compiler errors. Always invoke `./coax/bin/coax` (not bare `coax`).
- **Getting the compiler fast:** `coax/build.sh` installs the published build from `~/.cache/coax-release` / release bucket / GitHub mirror in seconds (or APFS-clone `cp -c -R /Users/christopherdavid/work/openagents/coax/bin <wt>/coax/bin`). If missing, run `COAX_MUTATION=0 coax/build.sh` in background; in shallow clones run `git fetch --depth=2000 origin main` first ([coax/RELEASE.md](coax/RELEASE.md)).
- **Heavy commands & remote build hosts:** Route heavy builds/suites through `coax/remote/remote.sh run --` (runs locally when Mac is idle, offloads automatically under load; `OA_REMOTE=1` forces host, `--local` forces Mac; [docs/metaverse/remote-builds.md](docs/metaverse/remote-builds.md)). The API suite (`coax/web/api/test.sh`) always runs with `OA_REMOTE=1`. For web pages in a fresh worktree, fetch built pages with `coax/remote/pages.sh origin/main` (~10s) and rebuild only touched modules with `wasm.sh`.
- **Fast targeted iteration:** Test only the units you touched (`./coax/bin/coax test FILE --only SUBSTR`, `psionic/check.sh --only UNIT`, `psionic/tests/check_elephant_slices.sh DIR`, `coax/tests/run.sh --part I/8`, `coax mutate FILE --changed`). Never rerun whole-model CPU weights as a routine check. Full command list: [docs/REPO-MAP.md](docs/REPO-MAP.md).
- **Several small issues at once:** Code all fixes first, rebase onto `origin/main` before verifying (not after), fetch `pages.sh origin/main` before building modules, group suites via `coax/remote/check.sh only PATH`, rerun only failures, and commit one commit per issue.
- **Check duration & progress:**
  - **One command takes at most 15 minutes.** Split longer checks into slices/shards (`coax mutate --shard`) or run faster units; never sit waiting indefinitely.
  - **Never repeat a passing result.** Don't rerun unchanged passing suites to "confirm" — cite the earlier result and commit.
  - **Report progress at least every 30 minutes** with what changed and what's left. Stop when a run exceeds its limit.
- **iOS tests & TestFlight:** See [apps/openagents-ios/README.md](apps/openagents-ios/README.md). Run tests for what you touched.
