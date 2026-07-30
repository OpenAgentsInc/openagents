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
   [OpenAgents Mobile adaptation audit for Omega](./2026-07-24-openagents-mobile-omega-adaptation-audit.md).
8. Read the
   [Full Auto port audit](./2026-07-24-full-auto-port-audit.md).
9. Read the
   [Full Auto contract freeze](./2026-07-24-full-auto-contract-freeze.md)
   (`OMEGA-FA-00`).
10. Read the
    [omega-effectd extract](./2026-07-24-omega-effectd-extract.md)
    (`OMEGA-FA-01`).
11. Read the
    [omega-effectd supervisor](./2026-07-24-omega-effectd-supervisor.md)
    (`OMEGA-FA-02`).
12. Read the
    [omega-effectd host bridge](./2026-07-24-omega-effectd-host-bridge.md).
13. Read the
    [Agent Computer and Omega completion plan](./2026-07-24-agent-computer-omega-completion-plan.md).
14. Read the
    [HE-02 harness-environment release](./2026-07-24-he02-harness-environment-release.md).
15. Read the
    [Sarah workroom MVP specification](./2026-07-24-sarah-workroom-mvp-spec.md)
    (`OMEGA-SW-00` through `OMEGA-SW-07`, `SARAH-NR-00` through `SARAH-NR-09`,
    `SARAH-CW-00` through `SARAH-CW-09`).
16. Read the
    [Sarah Nostr record contract](./2026-07-24-sarah-nostr-record-contract.md)
    (`SARAH-NR-00`).
17. Read the
    [Sarah Nostr identity contract](./2026-07-24-sarah-nostr-identity-contract.md)
    (`SARAH-NR-04`).
18. Read the
    [owned Nostr relay deploy runbook](../ops/2026-07-24-owned-nostr-relay-deploy.md)
    (`SARAH-NR-03`, Option A host in `nostr-effect`).
19. Read the
    [Sarah NIP-AE companion profile](./2026-07-24-sarah-nip-ae-companion-profile.md)
    (`SARAH-NR-07a`).
20. Read the
    [Sarah Nostr migration and cutover](./2026-07-24-sarah-nostr-cutover.md)
    (`SARAH-NR-08`).
21. Read the
    [Sarah Nostr journey proof](./2026-07-24-sarah-nostr-journey-proof.md)
    (`SARAH-NR-09`).
22. Read the
    [community workroom contract freeze](./2026-07-24-community-workroom-contract.md)
    (`SARAH-CW-00`).
23. Read the
    [Sarah community journey proof](./2026-07-24-sarah-community-journey-proof.md)
    (`SARAH-CW-09`).
24. Read the
    [NIP adoption candidates](./2026-07-24-nip-adoption-candidates.md).
25. Read the [accepted Omega plan](../sol/2026-07-23-omega-zed-primary-surface-accepted-plan.md).
26. Read the [master roadmap](../sol/MASTER_ROADMAP.md).
27. Read the [Desktop release contract](../deploy/openagents-desktop-cross-platform-release.md).
28. Read the
    [Omega open issues unified completion plan](./2026-07-25-omega-open-issues-unified-completion-plan.md).
29. Read the
    [Omega Agent implementation roadmap](./2026-07-25-omega-agent-roadmap.md)
    (`OMEGA-AGENT`, epic omega#73).
30. Read the
    [Omega master delegation plan](./2026-07-25-omega-master-delegation-plan.md)
    (order and parallel lanes for all open omega issues).
31. Read the
    [Omega Agent shape record](./2026-07-25-omega-agent-shape-record.md)
    (`OMEGA-AGENT-00`, omega#74).
32. Read the
    [Omega Agent cloud-coupling severability trace](./2026-07-25-omega-agent-cloud-severability-trace.md)
    (`OMEGA-AGENT-00`, omega#74).
33. Read the
    [Omega Agent slim-agent audit](../omega-agent/2026-07-27-slim-agent-audit.md).
34. Read the
    [Omega Agent slim-agent specification](../omega-agent/2026-07-27-slim-agent-spec.md)
    (proposed, not admitted).
35. Read the
    [Omega cloud build and application update audit](./2026-07-27-omega-cloud-build-and-update-audit.md).
36. Read the
    [Omega network sniffer specification](./sniffer/README.md)
    (`OMEGA-SNIFF-01` through `OMEGA-SNIFF-08`).
37. Read the
    [Episode 263 Omega alpha release gap analysis](./2026-07-29-episode-263-alpha-release-gap-analysis.md)
    before claiming or releasing the three-mode alpha described by the
    owner-locked Episode 263 script.
38. Read the
    [Omega Nostr authentication and onboarding target](./2026-07-30-omega-nostr-authentication-and-onboarding.md)
    for the source-pinned path from background identity provisioning to a full
    Rust-first account, signer, recovery, relay-authentication, and
    interoperability product.

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

The OpenAgents Mobile adaptation audit owns the product recommendation for an
Omega mobile controller. It keeps one OpenAgents store app, makes Omega a
first-class host, and makes Omega issue `#31` the first mobile parity target.
It defines a Nostr-primary thin-whole workroom before generic ACP, Git, or
terminal control. It also separates closed source packets from physical mobile
parity.

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

The owned Nostr relay deploy runbook records Option A hosting from
`nostr-effect`. It records the Node pin, Cloud Run steps, Cloud SQL steps,
DNS for `relay.openagents.com`, and the load-proof harness exit
(`SARAH-NR-03`).

Part 3 is the v2 roadmap for the semi-public community workroom. Outside
developers point their own compute at bounded work there. They earn experience
points, and Sarah arbitrates. The v2 room does not pay in its first version.
The build order is Nostr first, so no client is built on the current record
and then replaced. The specification is a proposal, not an admitted packet
ledger.

The community workroom contract freeze (`SARAH-CW-00`) locks the v2 room laws.
It freezes group identity, membership, grants, experience, rank, and
settlement. It freezes the two-room rule and the authority table. Canonical
and negative fixtures live under `fixtures/sarah-community-workroom/`.

The Omega Agent shape record owns the shape decision for the first-party
agent. It also owns the rejected alternatives and the open owner question
about the Omega Nostr identity. The severability trace owns the statement of
what the agent is when no external service answers. The ProductSpec at
`specs/omega/omega-agent.product-spec.md` owns the intent for both.

The cloud build and application update audit owns the recommendation to move
Omega production builds off the active workstation. It also owns the remote
macOS runner decision, disk controls, and the proposed owned update path.

The network sniffer specification owns the proposed capture boundary for any
independent local application. It defines verified application identity, the
durable file, exact fidelity labels, sensitive data rules, and the Omega Agent
inspection tool.

The Episode 263 alpha release gap analysis compares the owner-locked spoken
promise with the current installed product shape. It owns the no-go verdict,
the restoration packets for Direct Agent, Omega Agent, Sarah, tester channels,
and Omega downloads, and the installed-candidate acceptance matrix. It does
not itself admit the public claim or promote a build.

The Omega Nostr authentication and onboarding target reconciles the removed
first-run identity wizard with the current background-created identity. It
compares the current product with Buzz and Armada, then proposes progressive
activation, multiple signer types, account switching, bounded hydration,
device grants, and NIP-29/Buzz/Armada entry without moving root-key custody out
of the Rust service. It is a proposal and does not admit those packets.

The accepted plan owns the product and repository boundary.
The master roadmap owns priority across OpenAgents programs.
Current code, tests, and receipts own implementation truth.

The immediate target is Omega `v0.2.0-rc1`.
This target is a branded bootstrap prerelease.
It is not a Desktop feature-parity or primary-cutover claim.

The first product slice is identity-first onboarding.
It adds identity first to the existing GPUI onboarding structure.
It preserves the current Theme and Agent Setup sections.
