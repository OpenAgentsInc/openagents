# QS7 Rhys Sales Motion Owner Review Packet

Date: 2026-07-02
Status: owner-gated evidence packet; no external PR opened
Source issue: OpenAgentsInc/openagents#8067
FleetRun: `fleet_run.qa8034.fixed.20260703T014152Z`
Pinned base: `main@e5473b5657678104fe376087adc03d01b55b22d5`

This document is the in-repo QS7 closeout packet for the bounded public
checkout. It now records a live, read-only executor public-home audit receipt
bundle plus the exact owner gate that still applies before the outward-facing
demo PR is opened.

Context note (2026-07-03): Episode 246 (`docs/transcripts/246.md`)
acknowledges this thread on camera — the owner cites Rhys's public QA-agent
ask as the trigger for productizing QA Swarm ("maybe for you, but
definitely a bunch of other people are going to want that also"), which
raises the value of landing this motion well once the owner gate clears.

Refresh note (2026-07-03): the product prerequisites that were missing when
this packet first landed are now in `main`: QS2 share URLs (#8062), QS8
chill-evals variant comparison (#8068), and QS9 third-party target adapters
(#8069). This change adds the target-specific executor receipt bundle. The
remaining blocker is explicit owner sign-off on the external PR body/media and
where the media should be publicly attached.

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
| Audit run against executor produces full artifact set | Complete for the read-only public-home scenario | Browser video, trace, screenshot, result verdict, terminal asciicast, terminal snapshots, committed e2e test, chill-eval output, and share projection are fingerprinted below. |
| PR drafted with auto-attached media, distilled test, verdict, share URL | Drafted, not opened | Concrete body below uses public refs and sha256 fingerprints. Media upload/attachment location remains owner-gated. |
| Chill-evals variant comparison included | Complete for local/fixture no-spend comparison | Baseline public-home scenario passed; deliberately false candidate failed honestly. This proves the comparison harness and public page behavior, not a decision-grade hosted lane. |
| Owner sign-off recorded before PR goes public | Recorded as required, not granted | `NEEDS_OWNER.md` carries the QS7 owner gate. |

No external repository content was modified. No public comment or PR was opened
against `RhysSullivan/executor`.

## Live Receipt Manifest

Run token: `qs7-executor-20260703T151831Z`

The artifacts remain unpublished until the owner approves attachment/publication
location. Public docs record only refs, statuses, and sha256 fingerprints.

| Artifact | Receipt ref | Status | sha256 |
| --- | --- | --- | --- |
| Browser recording | `video.qa_swarm.executor.qs7-executor-20260703T151831Z.browser` | pass | `a2979cb587778eaa8dd0b4f79293d14b9e1a3305c16cdbd02befc2f451ce18b7` |
| Browser trace | `trace.qa_swarm.executor.qs7-executor-20260703T151831Z.browser` | pass | `975be4a44509c0e40387e7f5055212cf84fba0978aac1bb715d447df1905a088` |
| Browser verdict | `artifact.qa_swarm.executor.qs7-executor-20260703T151831Z.verdict` | CONFIRMED | `38fbc5839dfb93024269da79c9da21fb3b41b54ea71b2db62aa6ee3615d27f3a` |
| Browser screenshot | `artifact.qa_swarm.executor.qs7-executor-20260703T151831Z.screenshot` | pass | `dc9007c282c19f1c00a1ca779ef8b79c784ce638d15866d9e38f4a89dbb50c94` |
| Terminal recording | `video.qa_swarm.executor.qs7-executor-20260703T151831Z.terminal` | pass | `927b06fc70491ae2e8a74a4a9cc89ec2fa28264ef3bdb87a09b73864e32f3577` |
| Terminal snapshots | `artifact.qa_swarm.executor.qs7-executor-20260703T151831Z.terminal_snapshots` | pass | `7bfb17be2f9e54f947c5989537c19bbfd2e64d77ef7c3407e595722b1527a13e` |
| Terminal verdict | `artifact.qa_swarm.executor.qs7-executor-20260703T151831Z.terminal_verdict` | pass | `25d7bf42d3fa91e80bf89f739b78995b8cae69b0d101171312ee971fcf0fff9b` |
| Distilled e2e test | `test.qa_swarm.executor.public_home.e2e` | committed | `21db143076ac4ded24d7a5e722e00845d0e3ca43723238fefea84c5a72d1abe5` |
| Chill-eval result | `trace.compare.qa_swarm.executor.qs7-executor-public-home-20260703` | baseline pass, candidate fail | `2895add8e89518d944d69a3ed4ee8d0d649eadfae84a6bdd2d18c33c8bc395cd` |
| Chill-eval baseline video | `video.qa_swarm.executor.qs7-executor-public-home-20260703.baseline` | pass | `34b479e53b55e150cfaae3e1268480dc7283ec12eb521e9474c4ae3cf8f0a5dd` |
| Chill-eval candidate video | `video.qa_swarm.executor.qs7-executor-public-home-20260703.candidate` | fail | `20700889cb3fa952c6285c40e0ed93dfcce5684706cfef962d8a8423cebc9dec` |
| Share projection | `qa-run.executor.qs7-public-home.20260703` | warning: live tiers skipped | `99f79dec1f80dedda8202b3f474085bc03e1f91b0d7d2a20dd2645487e4702c6` |

Share URL staged for owner review:

```text
https://openagents.com/qa/qa-run.executor.qs7-public-home.20260703
```

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
and terminal drove the public read-only scenario, the session was distilled into
a reviewable e2e test, and the artifacts below are the review surface.

Verdict: `CONFIRMED`
Share URL: https://openagents.com/qa/qa-run.executor.qs7-public-home.20260703

### Artifacts

- Browser recording: `video.qa_swarm.executor.qs7-executor-20260703T151831Z.browser`
- Terminal recording: `video.qa_swarm.executor.qs7-executor-20260703T151831Z.terminal`
- Distilled test: `test.qa_swarm.executor.public_home.e2e`
- Trace: `trace.qa_swarm.executor.qs7-executor-20260703T151831Z.browser`
- Chill-evals comparison: `trace.compare.qa_swarm.executor.qs7-executor-public-home-20260703`

### Chill-evals variants

| Variant | MCP/config axis | Verdict | Runtime | Notes |
| --- | --- | --- | --- | --- |
| baseline | `executor-public:baseline` | `CONFIRMED` | `1176ms` | Public landing page rendered expected hero, MCP gateway, and Codex copy. |
| candidate | `executor-public:impossible-copy` | `REFUTED` | `1009ms` | False copy assertion failed honestly; pass-rate delta `-100%`. |

### Verification

```sh
TARGET_URL=https://executor.sh ARTIFACT_DIR=./runs/executor-public-home \
  bun test apps/qa-runner/generated/executor-public-home.e2e.test.ts

bun run --cwd apps/qa-runner evals -- \
  --scenario executor-public-home \
  --url https://executor.sh \
  --name executor-public-prod \
  --out ./runs/qs7-executor/chill-eval \
  --id qs7-executor-public-home-20260703 \
  --reps 1 \
  --md
```

The test source and recordings are the proof surface; reviewers should not need
to run the project locally to understand the result.
````

## Chill-Evals Comparison

QS8 made the comparison first-class. QS7 fills the public-safe shape with the
executor public-home scenario:

| Field | Meaning |
| --- | --- |
| `runRef` | `qa-run.executor.qs7-public-home.20260703` |
| `variantRef` | `baseline`, `candidate` |
| `mcpProfileRef` | `executor-public:baseline`, `executor-public:impossible-copy` |
| `scenarioRef` | `test.qa_swarm.executor.public_home.e2e` |
| `verdict` | Baseline `CONFIRMED`; candidate `REFUTED` |
| `durationMs` | Baseline `1176`; candidate `1009` |
| `traceRef` | `trace.compare.qa_swarm.executor.qs7-executor-public-home-20260703` |
| `videoRefs` | Baseline/candidate video refs in the live receipt manifest |
| `deltaSummary` | Candidate pass-rate delta `-100%`; candidate failure is the deliberate impossible-copy assertion. |

## Owner Gate

The owner must approve all of the following before the public PR is opened:

- The target branch and scenario scope for `RhysSullivan/executor`.
- The exact external PR body.
- The media attachments after redaction review.
- The public share URL and comparison URL.
- The verdict wording, especially any REFUTED or INCONCLUSIVE result.

Until that happens, the correct status for QS7 is:

```text
NEEDS-OWNER: QS7 executor receipts exist and the public PR body is drafted.
Do not open the external PR or attach media until the owner approves this packet.
```
