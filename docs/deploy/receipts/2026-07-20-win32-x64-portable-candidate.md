# Windows x64 unsigned experimental portable Desktop candidate receipt

- Date: 2026-07-20
- Program: [#8913](https://github.com/OpenAgentsInc/openagents/issues/8913)
- Issue: [DIST-07 #8920](https://github.com/OpenAgentsInc/openagents/issues/8920)
- ProductSpec: [OpenAgents Desktop cross-platform release](../openagents-desktop-cross-platform-release.md) §§2-7, 14.2
- Target key: `win32-x64`
- Channel: `rc`
- Version: `0.1.0-rc.25`
- Source revision: `e1a0514568d94f8a951ec296d44e4497b212c086` (identical to the
  2026-07-20 macOS arm64/x64 and Linux x64 candidates, so all form one coherent
  release set)
- Host: GCE `oa-rel-worker-win-x64`, project `openagentsgemini`, zone
  `us-central1-a`, `n2-standard-8`. Windows Server 2022 Datacenter, build 20348
  (`x86_64`). Node 24.13.1, standalone pnpm 11.10.0, rustc 1.97.1, Electron
  43.1.0, Electron Forge 7.11.2. `nasm` (for the `ring` assembly used by the
  native helper) and 7-Zip were provisioned on the worker. A standalone
  `pnpm.exe` was placed on `PATH` so the Node staging flow can resolve `pnpm`
  as a child process (Windows corepack ships only `pnpm.cmd`, which Node
  declines to run without a shell).

## Status — unsigned experimental portable (no Authenticode, no Azure)

This is the **unsigned experimental portable candidate**. It matches the
coverage that rc.20 and rc.21 already shipped
(`OpenAgents-0.1.0-rc.XX-rc-win32-x64-portable.zip`). By owner direction,
Windows for this candidate does **not** require signing: Azure Trusted Signing
and Authenticode are removed from the requirements for the experimental
portable path. This artifact:

- carries **no Authenticode signature** and was built with **no Azure Trusted
  Signing** (`--unsigned-dev`, `OA_ALLOW_UNSIGNED_DEV=1`);
- is **not** the ProductSpec §4/§14.2 per-user NSIS installer, and makes **no**
  Windows-trust or `Get-AuthenticodeSignature=Valid` claim;
- is **excluded from** the signed ReleaseSet, the signed update feed, channel
  promotion, `/download`, and every auto-update support claim;
- was **not** published, tagged, promoted, or uploaded to any feed.

#8920 stays **OPEN** pending its (amended) acceptance.

## Build

Built on the owned Windows worker with the repository entrypoint:

```
OA_ALLOW_UNSIGNED_DEV=1 node --import tsx \
  apps/openagents-desktop/scripts/stage-and-package.ts \
  --target win32-x64 --mode package --unsigned-dev
```

No repository code changes were required. The descriptor-first staging flow
(`scripts/stage-target.ts`) exported the exact source revision into a clean
temporary workspace, verified the immutable lockfile identity
(`a3955abfce13f3de6b7ad85b102c2c22be2b1d94ad823cf9f63c35150fee684f`), ran the
locked `win32`/`x64` target-only production install, built the owned native
voice helper `oa-desktop-audio` from the exported source with the explicit Rust
triple `x86_64-pc-windows-msvc`, ran the staged-tree oracle, and Electron Forge
`package` produced the packaged `win32-x64` application. The Apple Foundation
Models Swift sidecar is macOS-arm64-only and is correctly not built or staged
for this target.

Windows has no configured maker for a portable ZIP (the repository `MakerZIP`
targets `darwin` only) and the NSIS maker path is the signed production route.
The portable artifact is therefore the packaged Forge output archived into a
ZIP — the same experimental portable shape as rc.20 and rc.21.

- Staged native component ledger ref:
  `sha256:e4c85000eedd8d185c21c3c87ea17095f3ff8083203287ee092a1e64cc631d79`
  (2 native components: `oa-desktop-audio.exe` and the bundled
  `@anthropic-ai/claude-agent-sdk-win32-x64/claude.exe`).
- Live post-package ASAR admission gate: **pass** (81 entries, 74 unpacked, 2
  closure components byte-verified against the staged ledger for `win32-x64`).

## Artifact

Kept outside the repository (never committed).

| Artifact | SHA-256 | Bytes |
| --- | --- | --- |
| `OpenAgents-0.1.0-rc.25-rc-win32-x64-portable.zip` | `38f98a35aaaa5897afe6c217953e45a6b0020c6d15d82a9f87457201e171bce2` | 233443404 |

## Verification results (no signed-trust claim)

Run against the packaged `win32-x64` application. No Authenticode, Gatekeeper,
or notarization check applies to an unsigned artifact; the checks below are
architecture, Electron-fuse, ASAR-integrity, and native-closure verifications
only. All passed.

### Main executable architecture (PE machine)

| Check | Result |
| --- | --- |
| `OpenAgents RC.exe` PE machine field | `0x8664` (IMAGE_FILE_MACHINE_AMD64) → **x64** |

### Electron fuses (read from the packaged app with `@electron/fuses` `getCurrentFuseWire`)

All nine match `forge.config.ts`:

| Fuse | Value |
| --- | --- |
| RunAsNode | Disabled |
| EnableCookieEncryption | Enabled |
| EnableNodeOptionsEnvironmentVariable | Disabled |
| EnableNodeCliInspectArguments | Disabled |
| EnableEmbeddedAsarIntegrityValidation | Enabled |
| OnlyLoadAppFromAsar | Enabled |
| LoadBrowserProcessSpecificV8Snapshot | Disabled |
| GrantFileProtocolExtraPrivileges | Disabled |
| WasmTrapHandlers | Enabled |

### ASAR integrity

| Check | Result |
| --- | --- |
| Actual `app.asar` header-string SHA256 (`@electron/asar getRawHeader`) | `f8d272dba1a00274215a33bf800604145ba7a57dc41474745111641f507baaf6` |
| Declared header hash embedded in `OpenAgents RC.exe` (ElectronAsarIntegrity) | present and **equal** to the actual header hash |
| Match | PASS (81 entries listed) |

`EnableEmbeddedAsarIntegrityValidation` is enabled (fuse table above), so the
packaged runtime validates the archive against the embedded declared hash at
load time.

### Bundled native closure architecture

Every PE payload under `resources/` was header-checked; none is a
foreign-architecture binary.

| Payload | PE machine | Arch |
| --- | --- | --- |
| `resources\native\x64\oa-desktop-audio.exe` (built from source, `x86_64-pc-windows-msvc`) | `0x8664` | x64 |
| `resources\app.asar.unpacked\node_modules\@anthropic-ai\claude-agent-sdk-win32-x64\claude.exe` | `0x8664` | x64 |
| Any foreign-architecture PE under `resources\` | none found | — |

## Remaining gap (why #8920 stays open)

This is the unsigned experimental portable only. DIST-07 support still requires,
per ProductSpec §§4-7 and 14.2, the signed per-user NSIS installer with Windows
trust verification (publisher `OpenAgents, Inc.`), plus native clean install,
protocol registration, Start-menu/taskbar identity, uninstall, N-1 update,
interruption/locked-file drain, first launch, and retained-slot rollback on real
Windows 10 22H2 and current Windows 11 x64 hosts — none of which an unsigned
portable ZIP provides. The full five-target signed ReleaseSet convergence and
promotion remain owner-gated under #8917.
