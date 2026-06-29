# Pylon Local Compute Instruction Packet

Date: 2026-06-05

Issue: `OpenAgentsInc/openagents#162`

Status: guarded agent/user setup instructions for Autopilot Sites and
OpenAgents local-compute flows.

## Authority Boundary

Pylon is optional local compute. A Site agent may suggest Pylon only after a
human owner asks about local compute or grants explicit owner/operator
authority.

Do not run Pylon, register a provider identity, submit artifacts, claim
accepted work, or make payout claims from a generic Site-agent prompt alone.
Pylon/provider actions require the Pylon provider identity and Nexus/Pylon
policy, not generic Site-agent authority.

## Current Readiness Summary

Use this packet together with
`docs/sites/2026-06-05-pylon-v0-2-public-readiness-audit.md`.

Current status:

- `@openagentsinc/pylon@0.1.17+` is the package-managed launcher floor.
- `pylon-v0.1.16+` is the prior paid-training release floor with the same
  narrow guarantees described in the public earning runbook.
- `pylon-v0.1.23` is the latest Pylon GitHub release observed in the readiness
  audit.
- Pylon v0.2 is source-ready for the LDK-compatible payout-target contract, but
  it is not broadly public-ready as a v0.2 release claim.
- Current public release assets inspected for `pylon-v0.1.17` through
  `pylon-v0.1.23` are Darwin ARM64 only.

## Install Commands

Preferred launcher path:

```bash
npx @openagentsinc/pylon
```

Equivalent package-manager paths:

```bash
bunx @openagentsinc/pylon
npm install -g @openagentsinc/pylon && pylon
bun install -g @openagentsinc/pylon && pylon
```

If Pylon is already installed:

```bash
pylon
```

Pinning the prior paid-training floor:

```bash
npx @openagentsinc/pylon --version 0.1.16
```

Use pinning only when the operator intentionally wants that floor. The default
launcher path should resolve the current trusted release for the platform or
fall back to source build when no matching asset exists.

## Readiness Commands

Run these before making any local-compute or earning claim:

```bash
pylon --version
pylon status --json
pylon training status --json
pylon wallet balance --json
pylon wallet history --limit 20 --json
```

Interpretation rules:

- `pylon --version` proves only the local binary version.
- `pylon status --json` proves only local runtime and connectivity state.
- `pylon training status --json` proves only training/runtime readiness fields
  reported by the local node.
- `pylon wallet balance --json` proves only local wallet balance state.
- `pylon wallet history --limit 20 --json` proves only local wallet history
  entries visible to that node.

None of these commands alone prove paid-work eligibility, accepted work,
payment dispatch, confirmation, or settlement.

## Platform Instructions

### macOS

Apple Silicon macOS is the strongest current binary path. The launcher should
resolve current Darwin ARM64 assets when the matching public release asset
exists.

### Linux

Linux users can use the launcher, but current release assets inspected in the
readiness audit do not include current Linux archives. The launcher may fall
back to a source build.

Do not tell Linux users that a current binary install is guaranteed until
current Linux release assets and smokes are retained.

### Windows

For Windows, use WSL Ubuntu by default.

Do not steer normal users to native PowerShell or `cmd` for Pylon unless they
explicitly ask for the native Windows experimental path.

Current native Windows caveat:

- the launcher source maps `win32` to Windows;
- docs describe a `pylon-v<version>-windows-x86_64.zip` asset shape; and
- tests cover Windows archive naming and `.exe` paths;
- but no current native Windows release asset or retained native Windows smoke
  was observed in the readiness audit.

## LDK-Compatible Payout Target

Paid Pylon work requires an LDK-compatible payout target.

Supported target kinds in source:

- `bolt12_offer`
- `bolt11_invoice`
- `bip353_name`
- `lnurl_pay`

BOLT12 is the preferred durable target. BOLT11 should be treated as a
per-payment compatibility target. Spark targets are historical or migration
artifacts only and are not eligible for new paid work claims.

## Earning Caveats

Do not say "run Pylon and get paid" as an unconditional promise.

Keep these states separate:

| State | Meaning |
| --- | --- |
| Online | The Pylon process is connected or recently heartbeating. |
| Eligible | Nexus can assign the node a class of work because version, capability, payout target, and policy gates pass. |
| Assigned | A specific work item or lease has been assigned. |
| Accepted | The authority accepted the completed work outcome. |
| Paid | A payout was dispatched or confirmed against accepted work. |
| Settled | Terminal settlement evidence exists. |

Allowed claim shape:

```text
This Pylon appears online. It is not proof of paid-work eligibility yet.
```

Disallowed claim shape:

```text
This Pylon is online, so it can earn payouts.
```

## Referral Preservation

If a Pylon setup, Site challenge, invite link, or agent-generated prompt
arrives with an OpenAgents referral or source token, preserve the attribution
through the OpenAgents-hosted capture path.

After capture, use the clean canonical URL. Do not leave referral, checkout,
auth, account-result, or payment state in public product URLs.

## Agent Checklist

Before suggesting Pylon:

1. Confirm the human owner asked about local compute or granted explicit local
   compute authority.
2. Link to this packet and the v0.2 readiness audit.
3. State that Pylon is optional.
4. State the current version floor and platform caveats.
5. Ask the human to run the install/readiness commands locally.
6. Treat any command output as private unless the human explicitly chooses to
   share it.
7. Do not request wallet secrets, recovery phrases, raw node entropy,
   preimages, bearer tokens, API keys, or private channel state.
8. Preserve referral attribution through the hosted capture path.
9. Do not claim payment or settlement without receipts.

## Operator Checklist

Before using a Pylon for Sites-adjacent local compute:

1. Confirm owner/operator authority for this machine and this task.
2. Confirm Pylon runtime status with the readiness commands.
3. Confirm the work class is eligible for Pylon/local compute instead of SHC,
   Cloudflare, or a normal Autopilot run.
4. Confirm the work contract is bounded and receiptable.
5. Confirm artifact handling does not expose private customer source, secrets,
   wallet material, local paths, or raw runner logs.
6. Confirm accepted-work and payout claims reference Nexus/Treasury/Pylon
   receipts before showing them publicly.

## Current Public Wording

Use:

```text
Pylon is an optional local-compute path. Use `npx @openagentsinc/pylon` or
`pylon` only with explicit owner/operator approval. Current public instructions
use the 0.1-line launcher and release floor while v0.2 release assets and
platform smokes are still gated.
```

Do not use:

```text
Pylon v0.2 is ready for everyone.
```

Do not use:

```text
Run Pylon and earn money.
```
