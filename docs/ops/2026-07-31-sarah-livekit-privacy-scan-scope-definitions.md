# Sarah LiveKit eight-scope privacy scan: scope definitions and access

**Date:** 2026-07-31. **Verdict: the scan is now executable.** This document
supersedes the per-scope obstruction table in
[`2026-07-31-sarah-livekit-privacy-scan-executability.md`](2026-07-31-sarah-livekit-privacy-scan-executability.md),
which recorded eight scopes and none executable. All four blockers recorded
there are cleared, and the three undefined scopes are defined here.

That earlier document remains correct as of when it was written, and its
reasoning about why partial remediation buys nothing still holds. What changed
is that every blocker was cleared in one pass rather than one at a time.

## What changed

| Blocker recorded 2026-07-31 | State now | How |
| --- | --- | --- |
| `pods/log` Forbidden | cleared | The automation identity now reads current and previous-container output. Verified by a real `kubectl logs` call, not `auth can-i`. |
| `pods/exec` denied | cleared | Verified by a real `exec` against a named pod. |
| `clouderrorreporting` not enabled | cleared | API enabled, then `roles/errorreporting.viewer` granted. `groupStats` returns `200`. |
| `redis.instances.list` denied, no VPC path | cleared | `roles/redis.viewer` granted, and the read-only TLS path was built and proven. See below. |
| `packaged_clients` undefined | defined | Section 1. |
| `object_storage` undefined | defined | Section 2. |
| `traces` undefined | defined, and it is a finding | Section 3. |

`auth can-i` is not evidence on this cluster. It returned `yes` for `pods/log`
while the real call was still `Forbidden`. Every permission claim in this
document is backed by an actual API call against a named resource.

## Access granted

Three read-only project roles were granted to
`oa-mvp-automation@openagentsgemini.iam.gserviceaccount.com`:

- `roles/errorreporting.viewer` — `crash_artifacts`
- `roles/cloudtrace.user` — `traces`, read and list only
- `roles/redis.viewer` — `redis` instance metadata

`clouderrorreporting.googleapis.com` was enabled on the project. Pod log and
exec access were granted by the concurrent failure-drill lane and verified
here rather than granted twice.

Each role is read-only, project-scoped, and independently revocable. None of
them grants mutation, secret export, or settlement authority. The identity
still cannot read its own IAM policy, so this list is the authoritative record
of what was added.

## The read-only Redis path

The earlier finding recorded that the `redis` scope needed "both the instance
auth secret and a VPC network path from the collecting host. Neither exists."
The first half was wrong in a useful way and the second half is now solved.

There is no instance auth secret to obtain. The instance is provisioned
`auth_enabled = false` with `transit_encryption_mode = "SERVER_AUTHENTICATION"`
(`infra/modules/livekit-gke/main.tf`). Access is gated by network position, not
by a password: an EGRESS allow at priority 700 targets the SFU network tag, and
a deny at priority 800 covers everything else. So the requirement is to collect
from inside the allowed boundary, not to hold a credential.

The path is a short-lived read-only client pod pinned to the SFU node pool with
the SFU nodeSelector and toleration, mounting the existing `livekit-redis-auth`
secret for `ca.crt` and `host`, running `redis-cli --tls` against port 6378, and
returning its dump through pod logs. It issues only `PING`, `DBSIZE`,
`INFO keyspace`, `--scan`, `TYPE`, `TTL`, and the matching per-type read
(`GET`, `HGETALL`, `LRANGE`, `SMEMBERS`, `ZRANGE`). It writes nothing.

This was built and run. It returned `PONG`, `DBSIZE 18`, and a complete dump.
The pod was deleted afterward. Note that the `host` property already carries
the port, so the port must be split off before it is passed to `-h`.

Collecting this scope does require creating one pod in `livekit-system`. That
is a write to the cluster, and it is the reason the scope was previously called
unreachable. It is not a write to Redis, and it is not a change to any shared
firewall, NetworkPolicy, ConfigMap, Secret, or Deployment.

## 1. `packaged_clients`

**Definition: every shipped non-Omega client artifact, currently three.**

- **OpenAgents Desktop** — stable `0.1.0` (GitHub release
  `openagents-desktop-v0.1.0`, ten artifacts across darwin-arm64, darwin-x64,
  linux-x64, linux-arm64) and RC `0.1.1-rc.2` (macOS arm64).
- **OpenAgents mobile, iOS** — the accepted TestFlight build and its signed OTA
  JavaScript bundle on the `openagents-production` channel. Android ships
  nothing; the application id is configured but no Play or AAB release exists.
- **Pylon** — `@openagentsinc/pylon@1.0.5` on npm, plus the signed standalone
  RC binaries published to `updates.openagents.com`.

The former `clients/` applications and `apps/autopilot-desktop` are deleted and
ship nothing. They are additionally locked out at the update feed, which
returns a typed `410` for every legacy desktop route.

**Why the broad reading.** The runbook says "fully unpacked artifacts for every
other shipped client." A narrower "only clients that can reach LiveKit" reading
would make the set empty, because the server admits only
`clientProfile=omega_editor` to `livekit_room_v1` and mobile is deferred. An
empty set cannot be handed to a collector that requires every scope non-empty,
so the narrow reading is not merely stricter, it is unsatisfiable.

The broad reading is also the correct one on the merits, because of what this
scope actually tests. `packaged_clients` is in `CLIENT_SCOPES` but deliberately
not in `RETENTION_SCOPES`. It checks exactly three things: the production
OpenAI key, the `principal.sarah` private identity, and PEM private-key
material. That is a build-hygiene question — did a production secret get baked
into a shipped binary — and it applies to every artifact built in the same
environment regardless of whether that artifact speaks LiveKit. Excluding
Desktop and Pylon because they do not reach the SFU would exclude them from a
check that has nothing to do with reaching the SFU.

One consequence to plan for: neither shipped Desktop artifact and no iOS `.ipa`
is on the collecting host. They must be fetched from the GitHub release, the
update bucket, and npm before the window opens.

## 2. `object_storage`

**Definition: five buckets and one prefix.**

```
gs://openagentsgemini-audio-retention-mvp
gs://openagents-audio-retention-staging-157437760789
gs://openagentsgemini-livekit-deployment-receipts
gs://openagentsgemini-livekit-build-source
gs://openagentsgemini-terraform-state
gs://openagentsgemini-oa-artifacts/private/
```

**The selection rule.** A bucket is in scope if it meets at least one of:

1. it carries the label `service=livekit` — a mechanical selector, not a
   judgment call;
2. repository configuration declares it a write target of the Sarah voice path;
3. an in-repo invariant explicitly claims it is clean of secrets, media, or
   transcripts, which makes it a claim worth testing rather than an assumption
   worth trusting; or
4. it holds private per-owner session or agent payloads on a live production
   write path.

The other twenty buckets are excluded because they are empty, build or CDN or
static artifact stores, frozen pre-LiveKit decommission archives, or unrelated
product lanes, and because none is written by the LiveKit or Sarah runtime.

**Three things this scoping had to get right.**

There is no LiveKit media or recording bucket, and there cannot be one: the
bundle deploys no recording, egress, ingress, SIP, or TURN worker, and nothing
under `infra/livekit/production/` references a bucket at all. The intuitive
target for a media-retention scan does not exist. Object-storage exposure here
is entirely indirect — build source, receipts, state, retained audio, and the
Codex archive.

Prefix-scoping `oa-artifacts` to `private/` is load-bearing rather than
convenient. The excluded `sarah-avatar/` subtree contains a single 16.43 GiB
object, sixty-five times the collector's per-object limit, which would hard-fail
the scope outright; and it holds deliberate TTS and avatar production media that
would trip the raw-media detectors as guaranteed false positives. That subtree
belongs to the stopped avatar pipeline, not the LiveKit voice path.

Terraform state is included specifically because it has a documented prior
incident of the exact leak class being scanned for: a code-signing key was once
inline in state there, and superseded versions were purged. Excluding it as
"just infrastructure" would exclude the one surface with a known history. It is
also three objects and 427 KiB, so the cost of testing the claim is trivial.

**Carry this caveat into the gate.** `audio-retention-mvp` is currently empty.
A clean result over it proves the thirty-day lifecycle is working, not that
production retained audio is clean. The staging twin still holds 8,245 objects,
including synthetic `smoke:` canary owners, and is the substantive target.

Measured cost: about 9,070 objects, about 780 MiB, largest object 150.4 MiB,
which is under the 256 MiB limit.

## 3. `traces`

**Definition: no trace export exists. This is a finding, not a gap.**

Neither `livekit-server` nor `sarah-livekit-agent` emits distributed traces:

- no OpenTelemetry SDK is declared or imported anywhere in the monorepo — the
  208 lockfile hits are a dev-tooling peer-dependency qualifier, never a
  runtime path;
- no collector, sidecar, or DaemonSet is deployed in `oa-livekit-prod`, and the
  `gke-managed-otel` namespace referenced in a GKE argument does not exist;
- no tracing section exists in the LiveKit configuration, in the repository or
  in the live ConfigMap;
- no trace resources exist in any Terraform; and
- the agent affirmatively pins the OpenAI Realtime session's `tracing` field to
  `null` in both the session constructor and the canonical transport
  configuration, validates it per session, and enforces it with a policy test.

That last point is the strongest evidence. The one trace exporter the agent
could reach would ship session traces to the provider's dashboard, and it is
deliberately disabled with a test that fails if the source stops disabling it.
The absence is designed, not accidental.

This is now confirmed at the backend as well as in configuration. With
`roles/cloudtrace.user` granted, a seven-day query returns `{}`.

Do not conflate the ATIF agent-trace system with this scope. The monolith's
`KHALA_CHAT_TRACE_EMIT_ENABLED` governs `agent_traces` rows and the
`/trace/{uuid}` viewer, an application-level transcript projection with its own
owner-scoped redaction model. It emits nothing to Cloud Trace and belongs to
whichever scope covers agent transcripts. A `traces` scope naming it would be
scoping the wrong system.

Residual, and small: Cloud Run can sample inbound request latency to Cloud
Trace as platform behavior independent of workload instrumentation. That would
be span metadata — method, path, latency, status — never media, session
content, or provider payloads, and it does not apply to the GKE workloads at
all.

Because the scope is a proven negative rather than a body of evidence, its
export is the enumeration itself: the backend query, its empty result, and the
configuration proofs above. That satisfies the collector's non-empty
requirement honestly, and it asserts exactly what was checked.

## The remaining step, and one integrity requirement

Every scope is now collectable. What has not been done is the single
same-window collection run, and it must not be run without the following.

**The retention canaries must actually be injected.** The collector requires at
least one canary and compares it against the six runtime persistence scopes,
but it cannot verify the canary was ever introduced into the system. A canary
generated at scan time and never spoken into a live session will never be
found, and its axis will report clean while having tested nothing. Five of the
six axes — the OpenAI key, the Sarah private identity, PEM material, raw media,
and transcript objects and payloads — are meaningful without injection. The
retention-canary axis is not. A run that records `satisfied` on the strength of
an uninjected canary would be asserting a cleanliness nobody measured.

So the order is: inject unique synthetic canaries through a live Sarah LiveKit
session, then collect all eight scopes inside one two-hour window, then scan.
The injection step depends on the headless harness and should be sequenced with
whoever owns it.

Measured collection cost, so the window can be planned rather than discovered:

| Scope | Cost |
| --- | --- |
| `packaged_omega` | ~170 MiB, already unpacked on the collecting host |
| `packaged_clients` | ~1 GiB, must be fetched first |
| `pods` | ~1.8 GiB — 511 MiB per agent pod, 80.6 MiB per SFU pod, `/tmp` is 4 KiB |
| `logs` | bounded by the query window |
| `redis` | ~14 KiB |
| `object_storage` | ~780 MiB across ~9,070 objects |
| `traces` | the enumeration result |
| `crash_artifacts` | Error Reporting groups plus previous-container output |

About 4 GiB total. Fetch the packaged clients before the window opens, since
download time counts against the two hours.

## How the row should read

`privacy_scope_count` on `sarah-livekit-failure` stays `not_observed`. No scan
has run, so nothing has been observed, and the recorder refuses fewer than
eight scopes anyway.

The reason changes, and the change is the point. It is no longer "three
blockers outside what any repository change can fix." It is "all eight scopes
are defined and collectable; the same-window run with injected canaries has not
been performed." That is a scheduling statement, not an obstruction.

It is not `satisfied`, because nothing was scanned. It is not `contradicted`,
because nothing contradicted the row. And this document, like the one it
supersedes, is not a scan result — with one exception, stated plainly: the
`redis` scope was read in full during this work and contained no transcript
payloads, no PEM private-key material, no provider-key patterns, no Nostr key
material, and no media signatures across its eighteen keys. That is one scope,
read once, outside any acceptance window. It admits nothing.
