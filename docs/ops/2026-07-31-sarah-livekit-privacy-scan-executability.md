# Sarah LiveKit eight-scope privacy scan: executability against rc29

> **Superseded later the same day.** Every blocker below is cleared and every
> undefined scope is defined in
> [`2026-07-31-sarah-livekit-privacy-scan-scope-definitions.md`](2026-07-31-sarah-livekit-privacy-scan-scope-definitions.md).
> The scan is now executable. This document is retained because its reasoning
> was correct when written and because the "partial remediation buys nothing"
> argument still governs how the run must be sequenced. Do not cite the verdict
> below as current state.
>
> One correction worth carrying forward: `kubectl auth can-i` is not evidence on
> this cluster. It returned `yes` for `pods/log` while the real call was still
> `Forbidden`. The instinct recorded below — to verify with a real API call
> against a named resource — was right, and it should stay the standard.

**Date:** 2026-07-31. **Verdict: the scan cannot be executed today.** Four
independent blockers, each verified by a read-only command whose output is
quoted below. Three of the four are outside what any repository change can fix.

This document exists so `privacy_scope_count` on the `sarah-livekit-failure`
release-gate row can be recorded `not_observed` with a reason that names the
exact obstruction per scope, rather than an unexplained absence. It is a
feasibility finding. It is not a scan result, and it admits nothing.

## What the scan is

The collector is `scripts/cloud/livekit-privacy-scan.mjs`, defined in
`scripts/cloud/livekit-privacy-scan-lib.mjs` and documented under **Secret,
log, and retention scan** in
[`2026-07-30-livekit-self-hosted-gcp-runbook.md`](2026-07-30-livekit-self-hosted-gcp-runbook.md).

It compares the real production OpenAI key, the real `principal.sarah` private
identity, and private synthetic retention canaries against read-only exports
from eight named scopes, and additionally detects private-key material, named
transcript objects and payloads, and common retained media formats.

The eight scopes are fixed and ordered in `PRIVACY_SCOPES`:

`packaged_omega`, `packaged_clients`, `pods`, `logs`, `redis`,
`object_storage`, `traces`, `crash_artifacts`.

Two properties make this all-or-nothing:

- every scope must be present, complete, and non-empty; and
- the spread from the earliest `startedAt` to the latest `completedAt` across
  all eight must be at most two hours.

So a single blocked scope fails the entire run. There is no partial scan, and
the runbook is explicit that *"an unavailable backend is a failed gate, not a
zero-finding scan."* The gate recorder agrees independently:
`privacy_scope_count` requires at least eight distinct scopes, no residue, and
`sameWindowComplete`, so seven clean scopes records nothing.

## The production deployment is live

This is not a case of scanning something that is not running. The cluster
`oa-livekit-prod` (us-central1) is `RUNNING` with six nodes; three
`livekit-server` and three `sarah-livekit-agent` pods are `Running` in
`livekit-system` on images matching `infra/livekit/bundle.json`;
`https://livekit.openagents.com/` answers `200`; and
`docs/ops/receipts/livekit/production-sarah-headless-2026-07-31-long-interrupt.json`
records a passing live session against it. The blockers below are credential
and backend gaps, not absence of a target.

## Per-scope verdict

| Scope | Verdict | Obstruction |
| --- | --- | --- |
| `packaged_omega` | **ready** | The rc29 DMG is on disk at 170,083,787 bytes, under the collector's 256 MiB per-object limit. |
| `packaged_clients` | **undefined** | The scope is prose only: *"every other packaged client"*. No document or manifest enumerates which shipped artifacts those are, and the collector takes a directory rather than resolving them. |
| `pods` | **blocked** | The automation service account cannot read pod logs or exec. Verified against a named pod, not merely `auth can-i`. |
| `logs` | **ready** | `gcloud logging read` over `resource.labels.cluster_name="oa-livekit-prod"` returns live entries. |
| `redis` | **blocked** | No `redis.*` IAM, and the scope requires keys and values through a read-only TLS client, which needs both the instance auth secret and a VPC network path from the collecting host. Neither exists. |
| `object_storage` | **undefined** | 26 buckets are listable, several plausibly in scope. Nothing defines which are *"in-scope"*. |
| `traces` | **undefined** | `cloudtrace.googleapis.com` is enabled project-wide, but there is no evidence the LiveKit or Sarah workloads emit traces and no defined export boundary. |
| `crash_artifacts` | **blocked** | `clouderrorreporting.googleapis.com` is not enabled on the project, and the previous-container half of the scope depends on `pods/log`, which is Forbidden. |

### The verifying commands

All read-only, all with
`CLOUDSDK_CONFIG=/Users/christopherdavid/work/.secrets/gcloud-sa-config`.

`pods`, a real API call rather than an optimistic `auth can-i`:

```
$ kubectl -n livekit-system logs livekit-server-<pod> --tail=1
Error from server (Forbidden): pods "livekit-server-<pod>" is forbidden:
  cannot get resource "pods/log" ... requires one of
  ["container.pods.getLogs"] permission(s)

$ kubectl -n livekit-system auth can-i create pods/exec
no - requires one of ["container.pods.create"] permission(s)
```

`redis`:

```
$ gcloud redis instances list --region us-central1 --project openagentsgemini
ERROR: PERMISSION_DENIED: Permission 'redis.instances.list' denied
```

`crash_artifacts`:

```
$ gcloud services list --enabled --project openagentsgemini | grep -iE 'clouderrorreporting|cloudtrace'
cloudtrace.googleapis.com
```

Error Reporting is absent from the enabled set; Cloud Trace is present.

Both required secrets exist and were not read:
`sarah-nostr-identity-secret` and `oa-livekit-prod-openai-api-key` are both
present in Secret Manager.

## What each blocker needs

Three classes, and only the third is a repository change.

1. **Owner or infrastructure decisions.** Granting the automation service
   account `container.pods.getLogs` and pod exec on a production cluster is an
   authority expansion, not a configuration detail, and is the owner's to make.
   Enabling Error Reporting is a project-level API change. A read-only Redis
   path requires the instance auth secret plus a network route that does not
   exist from the collecting host.

2. **A scoping decision.** `packaged_clients`, `object_storage`, and `traces`
   each need someone to name the exact artifact set, bucket set, and export
   boundary. Until then the collector cannot be given a directory that means
   anything, and a scan built on a guessed boundary would assert cleanliness
   over a set nobody agreed to.

3. **Nothing in this repository.** No code change makes the scan runnable. The
   collector is correct; it has nothing complete to read.

Note that granting only the `pods` permissions would not produce a passing
scan. The same-window all-eight requirement means `redis`, `crash_artifacts`,
and the three undefined scopes still fail the run. Any remediation short of all
four blockers buys nothing.

## How this is recorded

`privacy_scope_count` belongs on the `sarah-livekit-failure` row as
`not_observed`, with a reason naming the blocked and undefined scopes and
citing this finding.

It is not `contradicted`: nothing was scanned, so nothing contradicted the row.
It is emphatically not `satisfied` with fewer than eight scopes, which the
recorder refuses anyway. And a feasibility finding is not a scan — this
document establishes why the scan produces nothing, never that the scopes are
clean.
