# Omega documentation

- Status: active
- Owner: OpenAgents
- Date: 2026-07-24
- Audience: product, engineering, release, and assurance teams

Omega is the Zed-based OpenAgents Desktop application.
It is the primary desktop destination.

Read these documents in this order:

1. Read the [Omega roadmap](./ROADMAP.md).
2. Read the
   [release-readiness brand audit](./2026-07-24-release-readiness-brand-audit.md).
3. Read the
   [identity-first onboarding roadmap](./2026-07-23-identity-first-onboarding-roadmap.md).
4. Read the
   [historical Rust and WGPUI harvest](./2026-07-24-historical-rust-wgpui-gpui-harvest.md).
5. Read the
   [Omega 3D avatar and Verse harvest audit](./2026-07-24-omega-3d-avatar-verse-harvest-audit.md).
6. Read the
   [Grok Build Rust harvest](./2026-07-24-grok-build-rust-harvest.md).
7. Read the
   [Full Auto port audit](./2026-07-24-full-auto-port-audit.md).
8. Read the
   [Full Auto contract freeze](./2026-07-24-full-auto-contract-freeze.md)
   (`OMEGA-FA-00`).
9. Read the
   [omega-effectd extract](./2026-07-24-omega-effectd-extract.md)
   (`OMEGA-FA-01`).
10. Read the
   [omega-effectd supervisor](./2026-07-24-omega-effectd-supervisor.md)
   (`OMEGA-FA-02`).
11. Read the
    [omega-effectd host bridge](./2026-07-24-omega-effectd-host-bridge.md).
12. Read the
    [Agent Computer and Omega completion plan](./2026-07-24-agent-computer-omega-completion-plan.md).
13. Read the
    [HE-02 harness-environment release](./2026-07-24-he02-harness-environment-release.md).
14. Read the
    [Sarah workroom MVP specification](./2026-07-24-sarah-workroom-mvp-spec.md)
    (`OMEGA-SW-00` through `OMEGA-SW-07`, `SARAH-NR-00` through `SARAH-NR-09`).
15. Read the
    [Sarah Nostr record contract](./2026-07-24-sarah-nostr-record-contract.md)
    (`SARAH-NR-00`).
16. Read the
    [Sarah Nostr identity contract](./2026-07-24-sarah-nostr-identity-contract.md)
    (`SARAH-NR-04`).
17. Read the
    [Sarah NIP-AE companion profile](./2026-07-24-sarah-nip-ae-companion-profile.md)
    (`SARAH-NR-07a`).
18. Read the
    [Sarah Nostr migration and cutover](./2026-07-24-sarah-nostr-cutover.md)
    (`SARAH-NR-08`).
19. Read the
    [Sarah Nostr journey proof](./2026-07-24-sarah-nostr-journey-proof.md)
    (`SARAH-NR-09`).
20. Read the [accepted Omega plan](../sol/2026-07-23-omega-zed-primary-surface-accepted-plan.md).
21. Read the [master roadmap](../sol/MASTER_ROADMAP.md).
22. Read the [Desktop release contract](../deploy/openagents-desktop-cross-platform-release.md).

The Omega roadmap owns the implementation order.
The release-readiness audit owns the current brand and package gap.
The identity-first roadmap owns the first native product journey.
The historical Rust harvest owns the boundary between reusable OpenAgents
product behavior and framework code that must not move into native GPUI.
The 3D avatar and Verse harvest owns the separate path from historical Verse,
Three.js, React Three Fiber, and Ruins of Atlantis into a Nostr-primary native
Omega Avatar Stage.

The Grok Build Rust harvest owns the boundary between direct Rust candidates,
behavior and test ports, and terminal or runtime systems that must not move.

The Full Auto port audit owns the Desktop-to-Omega Full Auto port plan.
The Full Auto contract freeze owns the admitted lifecycle, digests, redaction
map, and first-port cuts for `OMEGA-FA-00`.
The Agent Computer completion plan owns closeout of openagents `#9190` and
`#9193` plus Omega cloud-capacity integration.

The Sarah workroom MVP specification owns the proposed first workroom slice.
Part 1 puts one native Sarah conversation pane on the current OpenAgents API
record. Part 2 moves the Sarah runtime to Nostr on an owned relay, per the
owner direction of 2026-07-24.
The Sarah Nostr record contract freezes kinds `44300` and `44301`, the
conversation identifier, causal links, fixtures, the §7 projection map, and
the §21 boundary for `SARAH-NR-00`.
The Sarah Nostr identity contract freezes Secret Manager custody, the sealed
signer, NIP-OA/AA attestation, and lifecycle for `principal.sarah`
(`SARAH-NR-04`).

The Sarah NIP-AE companion profile freezes kind `30174` memory engrams
(`SARAH-NR-07a`).
It freezes NIP-44 encryption to the owner and HMAC-blinded `d` tags.
It freezes companion body fields.
It states that the graph index is derived and never authority.

The Sarah Nostr cutover note owns the `shadow` / `cutover` / `retirement`
stage machine, `SARAH_NOSTR_RECORD_MODE`, drift comparison, and
export/rollback for `SARAH-NR-08`. Production default stays `khala`.
The Sarah Nostr journey proof owns the automated receipt harness and the
residual live install checklist for `SARAH-NR-09`.

Part 3 is the v2 roadmap for the semi-public community workroom. Outside
developers point their own compute at bounded work there. They earn experience
points, and Sarah arbitrates. The v2 room does not pay in its first version.
The build order is Nostr first, so no client is built on the current record
and then replaced. The specification is a proposal, not an admitted packet
ledger.

The accepted plan owns the product and repository boundary.
The master roadmap owns priority across OpenAgents programs.
Current code, tests, and receipts own implementation truth.

The immediate target is Omega `v0.2.0-rc1`.
This target is a branded bootstrap prerelease.
It is not a Desktop feature-parity or primary-cutover claim.

The first product slice is identity-first onboarding.
It adds identity first to the existing GPUI onboarding structure.
It preserves the current Theme and Agent Setup sections.
