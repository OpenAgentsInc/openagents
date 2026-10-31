# Pylon guard decisions — 2026-10-30

## Delivered scope

The plan was pushed first as `ecba6b57d8`, before implementation. This slice
implements M0, M1/chapter 28 and the local shadow consumer of M2/chapter 29 in
[the delivery plan](../verse/pylon-guard-decisions.md). Tests ran on macOS 26.4
arm64 using this checkout's Coax compiler. No whole-model CPU run, broad model
mutation, production change or service interruption was needed.

The game consumer is off by default. Only `?guard-ai=shadow` enables it;
`?guard-ai=ambient` stays off. It counts proposals without changing dialogue,
patrols, combat, movement, rewards or replicated state. Actual public-Pylon
shadow latency/quality and ambient application remain separate milestones.

## Focused checks

| Component | Evidence |
| --- | --- |
| `psionic/lib/game/decision.coax` |7 checks; root-only mutation173/173 killed, zero survivors. Debug/release emitted C, each plain and ASan/UBSan, pass through `psionic/tests/check_game_decision.sh --native-only`. |
| `verse/everglade/guard-decision.coax` |48 checks including imported baselines; root-only mutation517/517 killed, zero survivors. `verse/test_guard_ai.sh`:21 lifecycle/transport cases in debug, release and ASan/UBSan. |
| `psionic/lib/fleet/discovery.coax` |4 checks;26/26 mutants killed, zero survivors. |
| `coax/lib/net/relay_poll_budget.coax` |1 check;10/10 mutants killed, zero survivors. `coax/tests/net/relay_poll_budget.sh`: debug/release ASan/UBSan, controls, empty fragments, exact caps, one read, partial input and preserved legacy behavior. |
| Generic HTTP effects |`bun test coax/web/http-requests.test.ts`:14 cases/78 assertions pass, including cancellation/reuse, old body-read completion, replacement generation, deadline once, streamed byte cap, malformed UTF-8 and legacy flags/headers/cookies/beacons. |
| Complete browser host |`bun test coax/web/storage.test.ts coax/web/scene-ready.test.ts`:13 cases/48 assertions pass, muted/headless WebGPU. |

These are 726 selected mutation sites across four small owned policy roots,
with no survivors and no equivalent markings. Imported crypto/model roots were
not mutated again. Later book markers are comments only. The post-review page
snapshot/warp/dialogue changes affect the page adapter, not the qualified
standalone policy.

The new `psionic/check.sh --only game/decision` dispatches before unrelated
oracle setup; the full unit inventory also includes game/decision and
fleet/discovery. The new HTTP module participates in shared golden input
tracking. A pre-existing quickstart-content expectation was reconciled with
its actual dependency on both the quickstart and Pylon board; the unchanged
other seven input tests and corrected selected case pass.

## Actual emitted page

Built only Everglade, with its independently checked policy library:

```shell
coax/web/wasm.sh apps/openagents-web/everglade.coax apps/openagents-web/everglade-face.c /tmp/everglade-guard-qa/everglade.wasm verse/player.coax verse/grid.coax verse/everglade/river.coax verse/everglade/guard-decision.coax
EVERGLADE_GUARD_WASM=/tmp/everglade-guard-qa/everglade.wasm bun test coax/web/everglade-guard.test.ts
```

Wasm build passes without diagnostics. The focused browser harness uses the
actual world Wasm, C face, shipped host and real local pack/WGSL, with a local
synthetic SystemOne edge. It boots headless Chrome with WebGPU and muted audio.
The test fixture restores an ordinary valid kept place near Bram; it does not
bypass game eligibility. All six scenarios pass:

- ordinary and unqualified ambient visits: no guard request;
- a matching pinned response: one dispatch and one later shadow admission;
- malformed and oversized HTTP200: no admission;
- guest401 and quota429: no admission, requests disabled for that visit;
- blur while a response is held: ticket revoked, delayed response cannot apply;
- golden and actual clip-capture modes: no guard request.

The first ordinary/ambient scenario passed in the initial run. The remaining
five passed in a selected 11.77-second run after correcting the synthetic
response's missing required service version. The unchanged passing scenario
was retained instead of repeated. Initial harness setup also corrected its
asset separator and ruleset facts. No product decoder was loosened to fit a
fixture. The screenshot showed the rendered South Gate near Bram with normal
HUD; the shadow result has no rendering hook.

Independent review found that the first page observation occurred before later
same-frame spell/harm mutations. It now runs immediately before the full frame
returns, after those effects and voice edits; pending warps are ineligible.
Hash changes and dialogue entry revoke leases. The final Wasm/browser evidence
uses that corrected ordering. Shared-schema page/policy headers are bridged
with local C type aliases, an equal-size assertion and a data copy; all admission
and effect descriptors remain Coax.

## Warm fleet measurement and red/green cases

[Machine-readable fleet evidence](pylon-guard-fleet-2026-10-30.json) retains
commands, source/artifact SHA-256 values and results. The peer is independent,
locally signed/encrypted and synthetic; there is no original-weight inference.
A retained old binary and the new sanitized binary used the same 5000 ms
discovery setting and repeated-call peer fixture:

| Calls | Old adapter | New adapter |
| --- | --- | --- |
| First warm call |5089ms |77ms |
| Second warm call |5071ms |42ms |

The change removes repeated discovery waiting after the current authenticated
connection has caught up. A bounded ready-input drain observes withdrawals
before selection and again after signing. Expiry, exact identity, generation,
free capacity and attempt deadline are rechecked before publication.

All 10 new discovery cases and 12 affected prior peer regressions pass under
ASan/UBSan. Cold delayed history took 381 ms; caught-up empty history followed
by an online provider took 351 ms. Review caught and reproduced the empty-history
regression first (503 in 1 ms); the fixed adapter retains the bounded discovery
window when no eligible route exists. Withdrawals, unknown EOSE, expiry,
reconnect, retries, late answers and loopback fallback remain qualified.

A sustained PONG peer reproduced a pre-existing lower-level polling loop: the
old candidate hit the test's 12-second HTTP abort. The additive bounded poll
limits each fleet poll to 64 scan/read steps, including controls and empty
fragments, and closes/refuses on exhaustion. The final PONG case refused in 26 ms
with zero jobs. Text-frame and byte floods refused in 34 ms / 33 ms with zero jobs.
Legacy poll behavior remains available to existing callers. Connect/flush
operations retain their own I/O ceilings; this is bounded processing work,
not a hard wall-clock scheduler or public-network latency promise.

## Book and remaining qualification

The lifecycle example's expected transcript was authored first; debug/release
runs match it exactly. The structural checker passes: 93 programs, 372 source-linked listings and
local links. The new actual-Wasm browser suite is explicitly opt-in with the
built artifact path; ordinary browser unit runs skip it rather than consume a
stale or missing realm build. The final host browser bundle passes too.
The opt-in wrapper was checked in both modes: unset skips the suite, and the
selected valid-shadow case passes with the artifact set (10 assertions).
The selector was also exercised with a recording compiler stub: game/decision
reaches only its gate before oracle setup; an unknown game selector exits2.
Source/artifact identities are in the [source manifest](pylon-guard-sources-2026-10-30.json). It also found two existing `sh` fences in chapter16
and one stale source-linked event listing in chapter19; those were mechanically
reconciled with the user's shell/Jai convention and current source.

Real account/public-edge guard calls, held-out appropriateness, calibration,
latency distributions, outcome telemetry and independent-provider failover are
not established by synthetic responses. These precede ambient promotion in
the plan. Local abort revokes application authority and frees browser capacity;
it does not claim that a remote GPU stopped. No default guard activation or
production deployment is part of this slice.
