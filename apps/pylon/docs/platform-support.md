# Pylon v0.3 Platform Support Matrix

Date: 2026-06-10

Issue: #4655
Promise: `pylon.release_tomorrow.v1`
Blocker: `blocker.product_promises.native_windows_wsl_unproven`

## Current Support Claim

Pylon v0.3 release-candidate support is limited to macOS and Linux. Native
Windows and WSL Ubuntu are not supported claims until install and bootstrap
smokes run on real hosts and their results are recorded here.

The package manifest currently restricts installs to:

- `darwin`
- `linux`

The bootstrap surface reports the same supported target set.

## Matrix

| Platform | Current claim state | Smoke evidence | Notes |
| --- | --- | --- | --- |
| macOS arm64 | Supported rc target | `bun run release:gate` passed locally on macOS arm64 on 2026-06-10 at commit `76769a1a6`. At commit `f86b54ad7`, `bun install --frozen-lockfile`, `bun run test`, `bun pm pack --dry-run`, and `bun run smoke:install:local` also passed from the Pylon workflow worktree. A fresh #4655 local smoke passed `bun run smoke:install:local`, and `pylon bootstrap --json` reported `packageName=@openagentsinc/pylon`, `bin=pylon`, `platform.current=darwin`, `platform.supported=true`, `platform.supportedTargets=[darwin, linux]`. | This proves the declared macOS target for the current repo state. It is not Windows/WSL evidence. The bootstrap projection still reports version `0.3.0-rc1`, so release docs should cite the package manifest separately when discussing rc2 packaging. |
| Linux | Supported rc target pending CI proof | #4654 moved the Pylon release gate workflow to `.github/workflows/pylon-release-gate.yml` at commit `f86b54ad7`. The workflow is active and is configured to run `bun install --frozen-lockfile`, `bun run test`, `bun pm pack --dry-run`, and `bun run smoke:install:local` on `ubuntu-latest` and `macos-latest`. | The first run, https://github.com/OpenAgentsInc/openagents/actions/runs/27307383813, failed before any runner step started because GitHub reported the account is locked due to a billing issue. That is not Linux smoke evidence. Tailnet `archlinux` was online, but no Linux install smoke was run in this #4655 recheck. |
| WSL Ubuntu | Unproven, unsupported claim | Not run. | No Windows/WSL host was listed in `tailscale status` on 2026-06-10. Requires operator-provided Windows host with WSL enabled or an equivalent CI/manual host. |
| Native Windows | Unproven, unsupported claim | Not run. | `apps/pylon/package.json` excludes Windows via the `os` field, and OpenTUI dashboard support is unproven. Requires an operator-provided native Windows host before support can be claimed. |

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

## Registry Transition Recommendation

Do not clear `blocker.product_promises.native_windows_wsl_unproven` as universal
platform proof. The honest transition is a re-scope: `pylon.release_tomorrow.v1`
may proceed toward green for a stable release only with copy that names the
supported platform matrix explicitly and does not imply native Windows or WSL
support.

Before any registry edit, record a product-promise transition receipt that
either:

- clears the blocker with real WSL and native Windows smoke evidence; or
- replaces the blocker with an explicit platform-scope caveat for macOS/Linux
  only.

## Next Evidence Needed

Run these on real hosts and append public-safe summaries:

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
