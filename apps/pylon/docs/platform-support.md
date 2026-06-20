# Pylon v0.3 Platform Support Matrix

Date: 2026-06-10

Issue: #4655
Promise: `pylon.release_tomorrow.v1`
Former blocker: `blocker.product_promises.native_windows_wsl_unproven`
Closeout: owner scoped Windows/WSL out of Pylon v0.3 release-candidate
support on 2026-06-10; registry version `2026-06-10.26` removed this blocker.
Live version `2026-06-10.27` preserves that scope.

## Current Support Claim

Pylon v0.3 release-candidate support is limited to macOS and Linux. Native
Windows and WSL Ubuntu are DELIBERATELY OUT OF SCOPE by owner decision
(2026-06-10): strongly deprioritized for the foreseeable future, removed
from roadmaps and promise blockers. This is a scope decision, not an
unproven claim awaiting evidence. The supported targets are macOS arm64
and Linux x64/arm64.

The package manifest currently restricts installs to:

- `darwin`
- `linux`

The bootstrap surface reports the same supported target set.

## Matrix

| Platform | Current claim state | Smoke evidence | Notes |
| --- | --- | --- | --- |
| macOS arm64 | Supported rc target | `bun run release:gate` passed locally on macOS arm64 on 2026-06-10 at commit `76769a1a6`. At commit `f86b54ad7`, `bun install --frozen-lockfile`, `bun run test`, `bun pm pack --dry-run`, and `bun run smoke:install:local` also passed from the Pylon workflow worktree. A fresh #4655 local smoke passed `bun run smoke:install:local`, and `pylon bootstrap --json` reported `packageName=@openagentsinc/pylon`, `bin=pylon`, `platform.current=darwin`, `platform.supported=true`, `platform.supportedTargets=[darwin, linux]`. | This proves the declared macOS target for the current repo state. It is not Windows/WSL evidence. The bootstrap projection still reports version `0.3.0-rc1`, so release docs should cite the package manifest separately when discussing rc2 packaging. |
| Linux | Supported rc target pending CI proof | #4654 moved the Pylon release gate workflow to `.github/workflows/pylon-release-gate.yml` at commit `f86b54ad7`. The workflow is active and is configured to run `bun install --frozen-lockfile`, `bun run test`, `bun pm pack --dry-run`, and `bun run smoke:install:local` on `ubuntu-latest` and `macos-latest`. | The first run, https://github.com/OpenAgentsInc/openagents/actions/runs/27307383813, failed before any runner step started because GitHub reported the account is locked due to a billing issue. That is not Linux smoke evidence. Tailnet `archlinux` was online, but no Linux install smoke was run in this #4655 recheck. |
| WSL Ubuntu | Deliberately out of scope | Not required for the v0.3 rc claim. | Owner decision on 2026-06-10 removed Windows/WSL from the release promise and roadmaps. Do not route more #4655 work toward finding a WSL host unless a future issue reopens that platform scope. |
| Native Windows | Deliberately out of scope | Not required for the v0.3 rc claim. | `apps/pylon/package.json` excludes Windows via the `os` field, and OpenTUI dashboard support is unproven. This is not a blocker for the macOS/Linux rc claim after the owner scope decision. |

## Tailnet Probe

`tailscale status` on 2026-06-10 listed:

- `macbook-pro-m5` on macOS, online;
- `archlinux` on Linux, online;
- `imac-pro-bertha` on macOS, online;
- iOS devices offline;
- `macbook-pro-m2` on macOS, offline.

No Windows or WSL host was available. Earlier SSH probes to `archlinux`
requested a fresh Tailscale SSH browser authentication check, so no Linux
remote install smoke was run from those sessions. A noninteractive SSH probe to
`imac-pro-bertha` failed with `Permission denied`; it is also macOS, not a
Windows/WSL target.

## Registry Scope Decision

Do not clear `blocker.product_promises.native_windows_wsl_unproven` as universal
platform proof. The registry took the honest re-scope path instead:
`pylon.release_tomorrow.v1` now names macOS and Linux explicitly, says
Windows/WSL is deliberately out of scope, and only retains
`blocker.product_promises.pylon_v03_stable_release_not_green`.

This was a same-state blocker/copy edit rather than a green transition:

- version `2026-06-10.26` removed the Windows/WSL blocker by owner decision;
- version `2026-06-10.27` is live and still shows only the stable-release
  blocker for `pylon.release_tomorrow.v1`;
- no Windows/WSL support claim is made.

## Copy-Drift Guard (`pylon.consumer_compute_earns_bitcoin_self_serve.v1`)

Blocker: `blocker.product_promises.windows_wsl_consumer_install_coverage_missing`.

The Episode 238 promise copy ("anybody can plug in consumer compute") must stay
narrowed to the platforms actually proven — macOS/Linux — and must never drift
back to an unqualified "any platform" or a "Windows/WSL covered" claim. That
honest-copy requirement is now machine-checkable in
`apps/pylon/src/consumer-install-platform-support.ts`:

- `classifyConsumerInstallPlatform(platform)` — pure, public-safe disposition for
  any platform: `supported` (darwin/linux, sharing `bootstrap.isSupportedPlatform`)
  vs `out-of-scope` (native Windows `win32`, the WSL Linux userland contributors
  conflate with native Linux, and everything else), with honest guidance refs and
  the blocker ref. Emits no machine identifiers, paths, usernames, or private
  material.
- `verifyConsumerInstallPlatformClaim(claim)` — audits an untrusted stated
  platform-support claim and returns `{ valid, overpromises, reasons[] }`.
  `overpromises === true` (the reviewer fail signal) when the supported set is not
  exactly `{darwin, linux}`, when it names `win32`/`windows`/`wsl`, or when it
  flags windows-in-scope, wsl-in-scope, or any-platform. Closed key allowlist so a
  copy fixture cannot smuggle an unreviewed assertion past the guard.

Tests: `apps/pylon/src/consumer-install-platform-support.test.ts` (14 pass).

This does NOT clear the blocker and changes no promise state. It does not build
Windows/WSL support or run a host probe; it locks the scope decision as an
enforceable gate so launch copy cannot over-promise platform coverage. Clearing
the blocker still requires the owner-facing copy-narrowing decision (and any
future reopen would follow the path below).

## Future Windows/WSL Reopen Path

If a future owner decision brings Windows or WSL back into scope, open a new
platform-support issue and run these on real hosts before changing copy:

```sh
bun install --frozen-lockfile
bun run --cwd apps/pylon smoke:install:local
PYLON_HOME="$(mktemp -d)" bun --cwd apps/pylon src/index.ts bootstrap --json
```

For native Windows, use the platform's temporary-directory equivalent instead
of `mktemp` if needed. Record only public-safe summaries: platform, package
name, binary name, supported flag, failure class, and whether dashboard startup
is CLI-only or dashboard-specific. Do not record private paths, usernames,
machine identifiers, wallet material, tokens, or raw environment dumps.
