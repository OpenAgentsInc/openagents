# 2026-04-27 Pylon Windows Build And Binary Audit

Date: 2026-04-27
Repos reviewed: `openagents.com`, `openagents`, `psionic`
Tracking issue: https://github.com/OpenAgentsInc/openagents/issues/4468

## Scope

This audit checks the current Pylon install instructions and release plumbing,
then answers whether OpenAgents can ship a precompiled Windows binary instead
of making Windows operators build from source.

I treated the user's "Scionic" reference as `psionic`, the sibling runtime repo.

## Short Answer

OpenAgents does not ship a native Windows Pylon binary today.

The fastest useful fix for Windows users is not native Windows first. It is to
ship current Linux Pylon assets for WSL Ubuntu on every Pylon release. Our docs
already tell Windows users to run Pylon inside WSL Ubuntu. If `pylon-v0.1.16`
had current `linux-x86_64` and `linux-arm64` assets, most Windows/WSL users
could use the precompiled path immediately.

A native Windows `.exe` package is feasible, but it is not proven. The repo has
some Windows-aware runtime code, and the known `cln-rpc` Windows compile
blocker was removed in March. The installer, release script, archive format,
asset naming, source fallback, and smoke tests still need explicit Windows
work before native Windows should be advertised.

## Current Release Truth

`@openagentsinc/pylon` latest on npm is `0.1.17`.

The latest Pylon GitHub release is `pylon-v0.1.16`, published on 2026-04-27.
Its assets are only:

- `pylon-v0.1.16-darwin-arm64.tar.gz`
- `pylon-v0.1.16-darwin-arm64.tar.gz.sha256`

The newest releases with Linux assets are older:

- `pylon-v0.1.12-darwin-arm64.tar.gz`
- `pylon-v0.1.12-linux-arm64.tar.gz`
- `pylon-v0.1.12-linux-x86_64.tar.gz`

Across all `pylon-v...` GitHub release assets currently visible:

- Darwin assets exist.
- Linux assets exist.
- Windows assets do not exist.

This matters because the launcher selects the newest tagged Pylon release with
matching assets for the current platform. A WSL Ubuntu `linux-x86_64` operator
can resolve an older Linux asset, but not the current recommended
`pylon-v0.1.16` binary. If the operator pins `--version 0.1.16` on WSL today,
the launcher must fall back to a source build because that tag has no Linux
asset.

## Website And Agent Instructions

`openagents.com/resources/js/pages/welcome.tsx` contains the copied agent
instructions shown on the public site.

The current public instruction flow says:

- prefer `npx @openagentsinc/pylon`
- use direct GitHub release assets only when the user does not want npm/bun
- fall back to source only when no matching release asset exists or the user is
  modifying/validating code
- on Windows, use WSL Ubuntu rather than native PowerShell or `cmd` unless the
  user explicitly wants a native Windows path

There is one stale detail in `openagents.com`: the copied prompt still says the
launcher checks GitHub every 30 seconds while the TUI is running. Current
`packages/pylon-bootstrap` and its README say the bounded cadence is six hours.
That is adjacent to this audit but not the main Windows blocker.

## Issues Reviewed

### `openagents#4355`

Title: `Suggested updates to agent install instructions to include Windows with WSL/Ubuntu installation`

URL: https://github.com/OpenAgentsInc/openagents/issues/4355

State: closed on 2026-04-18.

This issue is the main WSL decision. It updated the root `openagents` README
and `docs/pylon/README.md` so Windows users are steered into WSL Ubuntu, with
the checkout, models, build artifacts, Rust, Ollama, and `nvidia-smi` checks
inside Ubuntu. It explicitly says not to use native PowerShell or `cmd` unless
the user asks for a separate Windows-native lane.

Shipped summary from the issue:

- root README agent-install prompt now prefers WSL Ubuntu on Windows
- `docs/pylon/README.md` has a Windows-first WSL bring-up section
- WSL instructions include NVIDIA passthrough verification and Ubuntu
  prerequisite packages

### `openagents#4262`

Title: `Update Autopilot to use the current Pylon earnings/provider path`

URL: https://github.com/OpenAgentsInc/openagents/issues/4262

State: closed on 2026-04-11.

This issue records a user/contributor spending time debugging WSL and GPU
bring-up through Autopilot before discovering that Autopilot earnings were
stale and live provider earnings had moved to Pylon. It is relevant because the
support burden came from Windows/WSL bring-up being pointed at the wrong
product surface. It does not implement Windows binary support.

### `openagents#3429`

Title: `Build fails on Windows`

URL: https://github.com/OpenAgentsInc/openagents/issues/3429

State: closed on 2026-03-13.

This was a native Windows compile failure on Windows 11 with Visual Studio
2022 and CUDA 12.6. The reported blocker was `cln-rpc`, which imported
Unix-only Tokio socket types.

The issue was closed after `cln-rpc` was removed from the workspace dependency
path. Current local verification matches that: `cargo tree -i cln-rpc` no
longer resolves a package in `openagents`.

The closing comment said a Windows-target `cargo check` progressed past the old
`cln-rpc` failure, then stopped later because the verifying Mac did not have a
Windows/MSVC C toolchain. That means the known blocker was fixed, not that
native Windows Pylon was fully proven.

### `psionic`

I did not find a relevant `psionic` GitHub issue that made a Windows or WSL
Pylon support decision. Narrow searches for WSL, PowerShell, `cln-rpc`, and
Windows build issues returned no relevant Psionic issue. Broader "Windows"
searches mostly hit unrelated "training windows" wording.

The Psionic repo does contain some Windows-aware sandbox binary discovery, but
there is no current Psionic release issue that proves `psionic-train.exe` as a
packaged Pylon runtime on Windows.

## Current Installer And Release Gaps

`packages/pylon-bootstrap/src/index.js` currently rejects native Windows:

- supported platforms: `darwin`, `linux`
- supported architectures: `arm64`, `x64`
- `win32` throws before prebuilt lookup or source fallback

`scripts/release/pylon-binary-release.sh` also rejects native Windows:

- `host_os()` supports only Darwin and Linux
- `host_arch()` supports arm64/aarch64 and x86_64/amd64
- the script relies on Bash, `uname`, `install`, `chmod`, and tar packaging

There is partial Windows awareness in the runtime:

- Pylon resolves `pylon-tui.exe` when `cfg!(windows)`
- Pylon uses `std::env::consts::EXE_SUFFIX` for `psionic-train`
- proof process termination has a Windows `taskkill` path
- training process liveness has a Windows `tasklist` path

But the packaging and bootstrap paths do not use those runtime affordances yet.
The bootstrap also assumes installed binary paths named `pylon` and
`pylon-tui`, and source-build fallback expects `target/release/pylon` and
`target/release/pylon-tui`, not `.exe` files.

## Feasibility

### WSL Ubuntu prebuilt binaries: feasible now

This is the lowest-risk path and matches current docs.

WSL Ubuntu reports `process.platform === "linux"` to Node, so the existing
bootstrap target resolver already works in WSL. The missing piece is release
coverage. Every current Pylon tag should include:

- `pylon-v<version>-linux-x86_64.tar.gz`
- `pylon-v<version>-linux-x86_64.tar.gz.sha256`
- `pylon-v<version>-linux-arm64.tar.gz`
- `pylon-v<version>-linux-arm64.tar.gz.sha256`

That immediately makes the documented Windows path binary-first for normal
Windows operators using WSL Ubuntu.

### Native Windows binaries: feasible but not release-ready

Native Windows should be treated as a new platform bring-up, not as a small
release-script tweak.

Reasons it is feasible:

- Pylon is Rust and already has some `cfg(windows)` code.
- The known `cln-rpc` Unix-socket build blocker was removed.
- `psionic-train` is a Rust binary and the Pylon code already computes its
  filename with `EXE_SUFFIX`.
- The npm bootstrap architecture already knows how to map one platform to one
  release asset and verify checksums.

Reasons it is not ready:

- no current Windows release asset exists
- no native Windows bootstrap target exists
- no Windows asset naming contract exists
- no Windows archive/extract contract exists
- no Windows source fallback path exists
- no retained `pylon.exe` / `pylon-tui.exe` / `psionic-train.exe` smoke proof
  exists
- the graceful training drain path is Unix-only and currently falls back to a
  hard stop on non-Unix platforms

## Recommended Pathway

### Phase 1: Fix Windows via WSL assets first

1. Cut or backfill current Linux assets for the recommended Pylon binary tag.
   The practical target is the next release after `pylon-v0.1.16`, or
   `pylon-v0.1.16` if we intentionally backfill assets.
2. Build on clean Linux x86_64 and Linux arm64 hosts with:
   `scripts/release/pylon-binary-release.sh --version <version> --publish`.
3. Verify the release has matching Darwin and Linux assets before announcing
   it as the current paid-training floor.
4. Run the npm bootstrap from WSL Ubuntu with `--no-launch` and confirm the
   install method is prebuilt, not source build.
5. Run:
   - `pylon --version`
   - `pylon status --json`
   - `pylon inventory --json`
   - `pylon training status --json`
6. Confirm a WSL-installed Pylon can advertise the packaged training runtime
   without a sibling Psionic checkout.

This should be the immediate support answer for Windows operators.

### Phase 2: Add native Windows package support

1. Establish a clean Windows x86_64 builder with:
   - Visual Studio Build Tools
   - Rust stable
   - `x86_64-pc-windows-msvc`
   - Node/npm or Bun for bootstrap tests
   - `gh`
2. Build:
   - `cargo build --release -p pylon -p pylon-tui`
   - `cargo build --manifest-path ../psionic/Cargo.toml --release -p psionic-train`
3. Package:
   - `pylon.exe`
   - `pylon-tui.exe`
   - `psionic/target/release/psionic-train.exe`
   - the minimal Psionic runtime files currently copied into Linux/macOS
     archives
4. Prefer a `.zip` archive for native Windows unless we deliberately verify
   Windows tar behavior. Publish a `.sha256` beside it.
5. Extend `packages/pylon-bootstrap`:
   - map `win32` to a release OS label such as `windows`
   - use `.exe` names in install paths and source-build checks
   - select `pylon-v<version>-windows-x86_64.zip` or an explicitly documented
     Windows archive name
   - extract Windows archives without relying on POSIX `tar` or `chmod`
   - forward CLI subcommands to `pylon.exe`
   - launch `pylon-tui.exe`
6. Add bootstrap tests for:
   - `resolvePlatformTarget("win32", "x64")`
   - Windows asset name selection
   - Windows extraction/install paths
   - Windows source-build fallback looking for `.exe`
7. Smoke on native Windows:
   - `pylon.exe --help`
   - `pylon.exe --version`
   - `pylon.exe init`
   - `pylon.exe status --json`
   - `pylon.exe inventory --json`
   - `pylon.exe training status --json`
   - `pylon-tui.exe --help`
   - default `npx @openagentsinc/pylon --no-launch`
   - default `npx @openagentsinc/pylon status --json`
8. Only after that, upload:
   - `pylon-v<version>-windows-x86_64.zip`
   - `pylon-v<version>-windows-x86_64.zip.sha256`

Do not advertise native Windows as supported until the smoke proof exists.

### Phase 3: Decide whether native Windows is worth product support

Native Windows is useful if the goal is a frictionless PowerShell install for
non-technical users. It is less urgent if the immediate users are comfortable
with WSL Ubuntu and the current paid work is server-dispatched homework rather
than local Windows-specific inference.

The product decision should be explicit:

- WSL Ubuntu is the supported Windows path now.
- Native Windows is a separate platform with separate support commitments.
- Native Windows should not replace WSL until it has install, update, runtime,
  wallet, training, and payout proof.

## Bottom Line

It is feasible to stop making Windows users build everything from source.

The practical first step is to make the documented WSL Ubuntu path truly
binary-first by shipping current Linux assets on every Pylon release. That is
low risk and aligns with the existing instructions.

Native Windows binaries are feasible, but they require targeted platform work.
The known `cln-rpc` blocker is gone, and parts of Pylon already know about
`.exe` names, `tasklist`, and `taskkill`. The missing work is the release and
installer contract around `pylon.exe`, `pylon-tui.exe`, and
`psionic-train.exe`, plus one retained native Windows smoke run before public
support language changes.
