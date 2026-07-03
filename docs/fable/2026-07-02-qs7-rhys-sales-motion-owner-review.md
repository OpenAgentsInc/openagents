# QS7 Rhys Sales Motion Owner Review Packet

Date: 2026-07-02
Status: owner-gated readiness packet; no external PR opened
Source issue: OpenAgentsInc/openagents#8067
FleetRun: `fleet_run.qa8034.fixed.20260703T014152Z`
Pinned base: `main@1422b4a8440fd16bf1505cd052583f9bc4bed28e`

This document is the in-repo QS7 closeout packet for the bounded public
checkout. It does not claim that a Swarm Audit ran against
`RhysSullivan/executor`; it records the exact owner gate and the evidence shape
that must exist before the outward-facing demo PR is opened.

Context note (2026-07-03): Episode 246 (`docs/transcripts/246.md`)
acknowledges this thread on camera — the owner cites Rhys's public QA-agent
ask as the trigger for productizing QA Swarm ("maybe for you, but
definitely a bunch of other people are going to want that also"), which
raises the value of landing this motion well once the owner gate clears.

Refresh note (2026-07-03): the product prerequisites that were missing when
this packet first landed are now in `main`: QS2 share URLs (#8062), QS8
chill-evals variant comparison (#8068), and QS9 third-party target adapters
(#8069). This narrows the remaining blocker to the target-specific
`RhysSullivan/executor` run package and the explicit owner sign-off on the
external PR body/media. No live executor audit receipts are present in this
checkout.

## Source Contract

QS7 comes from `docs/fable/2026-07-02-qa-swarm-product-plan.md` section 6:
run the Swarm Audit package against `RhysSullivan/executor` and open the demo PR
Rhys asked for, including auto-attached webm plus terminal video, a distilled
`*.e2e.test.ts`, a CONFIRMED/REFUTED verdict, a share URL, and a chill-evals
comparison of MCP variants. The same section marks QS7 as dependent on QS2 and
QS8 and owner-gated before the PR goes out.

The requirements contract is
`docs/feature-requests/2026-06-24-autonomous-qa-e2e-from-computer-use.md`: the
reviewer should be able to evaluate the work by reading the committed e2e test,
watching the run output, and opening the shareable trace/evidence surface
without running anything locally.

## Honest Current State

| Acceptance item | State in this checkout | Evidence / blocker |
| --- | --- | --- |
| Audit run against executor produces full artifact set | Blocked, not run | QS2, QS8, and QS9 have landed, but no target-specific executor run receipt, media artifact, distilled test, verdict, or owner-approved external target scope exists in this checkout. |
| PR drafted with auto-attached media, distilled test, verdict, share URL | Draft only | Template below; every media/test/share placeholder must be replaced by dereferenceable receipts from a real run before it leaves this repo. |
| Chill-evals variant comparison included | Product support landed, target rows missing | Comparison matrix below; real variant rows require the executor audit to run across the chosen MCP/config variants. |
| Owner sign-off recorded before PR goes public | Recorded as required, not granted | `NEEDS_OWNER.md` now carries the QS7 owner gate. |

No external repository content was modified. No public comment or PR was opened
against `RhysSullivan/executor`.

## Required Artifact Manifest

The outward-facing packet is sendable only when every row has a dereferenceable
receipt:

| Artifact | Required receipt shape | Public-safety rule |
| --- | --- | --- |
| Browser webm | `video.qa_swarm.executor.<run>.<browser>` | Must show only target app/browser state intended for the demo. |
| Terminal video | `video.qa_swarm.executor.<run>.<terminal>` | Must redact local paths, tokens, env values, and raw provider payloads. |
| Distilled test | `test.qa_swarm.executor.<scenario>.e2e` | Committed in the external PR; should be readable as the verification contract. |
| Verdict | `artifact.qa_swarm.executor.<run>.verdict` | CONFIRMED, REFUTED, or INCONCLUSIVE from observed run output only. |
| Share URL | `qa-run.executor.<run>` rendered at `/qa/{runRef}` or equivalent | Public projection only; private target details stay opaque. |
| Chill-evals comparison | `trace.compare.qa_swarm.executor.<run>` | Variant axis and outcome deltas only; no raw prompts or credentials. |

## Draft External PR Body

Do not send this until the owner approves the final artifact set.

````markdown
## QA Swarm audit

This PR is a demo run of OpenAgents QA Swarm against `executor`: a real browser
and terminal drove the scenario, the session was distilled into a committed e2e
test, and the artifacts below are the review surface.

Verdict: `<CONFIRMED | REFUTED | INCONCLUSIVE>`
Share URL: `<openagents.com/qa/...>`

### Artifacts

- Browser recording: `<attached webm>`
- Terminal recording: `<attached terminal video>`
- Distilled test: `<path/to/*.e2e.test.ts>`
- Trace: `<openagents.com/trace/...>`
- Chill-evals comparison: `<openagents.com/trace/compare/...>`

### Chill-evals variants

| Variant | MCP/config axis | Verdict | Runtime | Notes |
| --- | --- | --- | --- | --- |
| baseline | `<current executor setup>` | `<result>` | `<duration>` | `<observed behavior>` |
| variant-a | `<MCP/config change>` | `<result>` | `<duration>` | `<delta>` |
| variant-b | `<MCP/config change>` | `<result>` | `<duration>` | `<delta>` |

### Verification

```sh
<exact command that passes in this PR>
```

The test source and recordings are the proof surface; reviewers should not need
to run the project locally to understand the result.
````

## Chill-Evals Comparison Schema

QS8 should make the comparison first-class. QS7 only needs to fill this
public-safe shape once the real executor run exists:

| Field | Meaning |
| --- | --- |
| `runRef` | Stable QA Swarm run ref for the executor audit. |
| `variantRef` | Public-safe variant id, not a raw config dump. |
| `mcpProfileRef` | Opaque MCP/config profile receipt. |
| `scenarioRef` | Distilled scenario/test receipt. |
| `verdict` | CONFIRMED, REFUTED, or INCONCLUSIVE. |
| `durationMs` | Observed wall-clock runtime for that variant. |
| `traceRef` | Public trace or trace-compare ref. |
| `videoRefs` | Public-safe browser and terminal media refs. |
| `deltaSummary` | Human-readable behavior/latency/verdict delta. |

## Owner Gate

The owner must approve all of the following before the public PR is opened:

- The target branch and scenario scope for `RhysSullivan/executor`.
- The exact external PR body.
- The media attachments after redaction review.
- The public share URL and comparison URL.
- The verdict wording, especially any REFUTED or INCONCLUSIVE result.

Until that happens, the correct status for QS7 is:

```text
NEEDS-OWNER: QS7 Rhys sales motion is ready for owner review only after real
executor run receipts exist. Do not open the external PR from this packet.
```
