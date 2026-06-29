# Artanis Pylon v0.2 Launch Readiness

Date: 2026-06-06

Status: implemented in #398 / `ARTANIS-013`, with launch communication
packaging added in #413 / `ARTANIS-027` and release-parity evidence added in
#419 / `ARTANIS-033`.

## Purpose

Artanis needs a public-safe way to summarize Pylon v0.2 readiness for Episode
232 without overstating what is actually ready.

This contract turns the existing Pylon readiness audit and local-compute packet
into a typed Artanis-administered checklist and Forum launch/update template.
#419 adds a stricter release-parity projection so source-level v0.2 support
does not get mistaken for a shipped v0.2 release.

## Implementation

Code lives in:

- `workers/api/src/artanis-pylon-v02-readiness.ts`
- `workers/api/src/artanis-pylon-v02-readiness.test.ts`
- `workers/api/src/artanis-pylon-v02-launch-communications.ts`
- `workers/api/src/artanis-pylon-v02-launch-communications.test.ts`
- `workers/api/src/artanis-pylon-v02-release-parity.ts`
- `workers/api/src/artanis-pylon-v02-release-parity.test.ts`

Source docs:

- `docs/sites/2026-06-05-pylon-v0-2-public-readiness-audit.md`
- `docs/sites/2026-06-05-pylon-local-compute-instruction-packet.md`
- `docs/pylon/2026-06-06-payout-target-admission-projection.md`
- `docs/pylon/2026-06-06-ldk-readiness-projections.md`

## Readiness Stages

The projection keeps these states separate:

- source-ready;
- release-ready;
- platform-ready;
- eligible;
- accepted;
- paid;
- settled.

The #419 release-parity projection adds explicit blockers for release tag,
release assets, package version, runtime smoke, platform smoke, eligibility
telemetry, payment target registration, accepted-work proof, paid-work
receipts, and settlement receipts.

Current seeded state:

| Stage | Current projection |
| --- | --- |
| Source-ready | Verified at source-contract level for the LDK-compatible payout target model. |
| Release-ready | Blocked until a v0.2 or explicitly documented release line, assets, and checksums are retained. |
| Platform-ready | Blocked until Linux, WSL Ubuntu, and native Windows smokes/assets are retained. |
| Eligible | Planned only; online does not mean eligible, and LDK-compatible target registration is required. |
| Accepted | Prohibited until accepted-work receipts exist. |
| Paid | Prohibited until public paid-work receipts exist. |
| Settled | Prohibited until public settlement receipt chains exist. |

Current direct release-parity answer:

```text
Pylon v0.2 source support exists. Pylon v0.2 has not shipped.
```

## Platform Guidance

Current public guidance is:

- Apple Silicon macOS is the strongest current binary path.
- Linux may require source build until current Linux assets and smokes are
  retained.
- Windows users should prefer WSL Ubuntu for now.
- Native Windows remains experimental until a current native Windows asset and
  retained smoke exist.

## Forum Template

The Artanis Forum launch/update template and #413 communication package include:

- setup packet ref;
- v0.2 public readiness audit ref;
- readiness command refs for version, status, training status, balance, and
  history checks;
- resource-mode caveats for background, overnight, and dedicated operation;
- explicit separation between online, eligible, assigned, accepted, paid, and
  settled;
- no request for credentials or local node material in public posts.

The #413 package also wires the canonical Pylon release work-log topic into
`/api/public/artanis/report` as `pylonLaunchCommunication` and into `/artanis`
as a compact Pylon launch section.

The template intentionally does not say:

- Pylon v0.2 is publicly released;
- Pylon v0.2 is ready for everyone;
- run Pylon and earn money;
- online means eligible;
- accepted means paid;
- paid means settled.

## Authority Boundary

This readiness contract is public copy and evidence discipline only.

It does not:

- publish a release;
- create release assets;
- register a Pylon provider;
- mutate Nexus;
- register or disclose payout targets;
- dispatch payouts;
- settle work;
- grant Artanis spend or provider mutation authority.

## Verification

Coverage lives in `workers/api/src/artanis-pylon-v02-readiness.test.ts`.
Launch communication coverage lives in
`workers/api/src/artanis-pylon-v02-launch-communications.test.ts`.

The tests cover:

- all required readiness stages;
- source-ready versus release/platform readiness;
- setup refs, readiness command refs, resource-mode caveats, and safe Forum
  copy;
- macOS, Linux, WSL Ubuntu, and native Windows platform guidance;
- rejection of broad public-ready and unconditional earnings copy;
- rejection of paid or settled readiness without public receipt chains;
- missing stage and missing platform guidance rejection.
