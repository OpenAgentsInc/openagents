# Pylon Release Freeze Network Readiness Closeout

Status: release-freeze guard for OpenAgents issue #4555.

## Public State Checked

Issue #4555 opened at `2026-06-08T02:05:01Z`.

The current public release state checked during this closeout is:

| Surface | Current public state | Public timestamp |
| --- | --- | --- |
| GitHub release | `pylon-v0.2.4`, marked latest | published `2026-06-07T20:11:14Z` |
| npm package | `@openagentsinc/pylon@0.2.5`, `latest` dist-tag | published `2026-06-08T03:50:29Z` |

That means the npm `latest` baseline moved after #4555 was opened. Do not hide
that fact and do not delete existing artifacts as part of this closeout. Treat
`pylon-v0.2.4` plus `@openagentsinc/pylon@0.2.5` as the current public baseline
for the freeze.

## Freeze Rule

Do not publish another public Pylon GitHub release, npm package version, npm
`latest` dist-tag move, or broad download/earning announcement until the live
network path is proven from the new issue set:

- OpenAgents #4563: real SHC Harbor Terminal-Bench smoke with Probe closeout
  bundles.
- OpenAgents #4564: public `benchmark-cloud` runner executing real Probe tasks.
- Psionic #1093: live Omega/Pylon closeout imports into the GEPA coordinator.
- Omega #513: Probe GEPA route scorecards connected to Coding on Autopilot
  outcomes.
- Probe #188: cross-repo tracker for the live Probe GEPA network smoke and
  product evidence tranche.

Omega #511 and #512 are now retained launch-gate evidence for the Artanis
public report. They do not by themselves authorize a new Pylon public release
or broad earning copy. They prove bounded status operation and retained
GEPA/Pylon smoke evidence. The next release needs live worker evidence.

## Next Release Preconditions

Before any newer public Pylon release or npm `latest` move:

- run a real SHC Harbor Terminal-Bench smoke with Probe;
- preserve `probe-closeout.json`, verifier refs, artifact manifests, proof
  bundles, resource receipts, route scorecards, and failure classifications;
- wire an Omega assignment lease to a real Pylon worker in `unpaid_smoke` mode
  first;
- prove worker accept, progress refs, artifact/proof submission, accepted or
  rejected closeout, and Psionic import without payout language;
- connect Psionic's GEPA coordinator to live Omega/Pylon imports while keeping
  the deterministic evaluator fallback;
- make the `benchmark-cloud` runner execute real Probe tasks, enforce allowed
  refs, sandbox constraints, tool menus, event refs, and normalized closeout
  bundles under failure and timeout;
- add a real Stage 0 campaign receipt bundle with live SHC or live Pylon
  assignment ids, closeout refs, and verifier artifacts;
- promote any Stage 1 candidate only to `shadow`, not active runtime;
- connect route scorecards to Coding on Autopilot workroom evidence;
- publish Artanis summaries only through the Omega/operator authority path;
- keep paid-work settlement out of scope until no-spend batches are boring and
  operator accounting is stable;
- start LoRA/Qwen/MLX training only after GEPA produces clean traces.

## Public Copy Boundary

Allowed:

- "The current public Pylon baseline is `pylon-v0.2.4` plus
  `@openagentsinc/pylon@0.2.5`."
- "The next release is frozen behind live Probe/Pylon/Psionic network evidence."
- "Artanis/Omega has bounded GEPA/Pylon status evidence, not a public
  Terminal-Bench score."

Not allowed:

- "Pylon v0.2 is generally available for broad earning."
- "Download Pylon now to earn bitcoin from Probe benchmark work."
- "GEPA/Pylon benchmark settlement is live."
- "A retained smoke is a public Terminal-Bench score."

## Verification Commands

Commands run for this closeout:

```bash
gh issue view 4555 --repo OpenAgentsInc/openagents \
  --json createdAt,updatedAt,number,title,state,url

gh release list --repo OpenAgentsInc/openagents --limit 15 \
  --json tagName,name,isLatest,isDraft,isPrerelease,publishedAt,createdAt

npm view @openagentsinc/pylon dist-tags version time --json
```

This is a docs and release-policy closeout. No release command was run and no
package publication was attempted in this closeout.
