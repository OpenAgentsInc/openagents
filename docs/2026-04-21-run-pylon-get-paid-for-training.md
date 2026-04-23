# Run Pylon, Complete Hosted Training Work, Get Paid

Published: 2026-04-21

OpenAgents now has a narrow public paid-training loop that works end to end.
A Pylon operator can install a current Pylon release, run one command, stay
online, receive hosted starter training work from Nexus, complete the work, and
receive Bitcoin into the local Pylon wallet when the work is accepted.

The user-facing command is intentionally small:

```bash
pylon
```

There is no CS336-specific opt-in command for normal operators. There is no
operator Nexus bearer token for normal operators. There is no direct GCS
credential for normal operators. Pylon comes online, advertises its local
capabilities and payout target, asks Nexus for available work, executes the
currently hosted starter lane when eligible, and watches the wallet for
accepted-work payment.

This is not the full open-ended training marketplace yet. It is the first
public, production-backed earning path for hosted starter training work. The
current live training work class is bounded CS336 Assignment 1 starter work.
That lane proves the important loop: online Pylon receives work, produces
artifacts through the public-safe signed-artifact path, gets validated, closes
out as accepted work, and receives sats for the accepted outcome.

## What Is Possible Now

Pylon can now run as the default online earning loop. Bare `pylon` initializes
local state, marks the node online, starts its local admin/status loop,
publishes provider presence when possible, and keeps provider and training
intake running while the process stays alive. The terminal UI is separate:
`pylon-tui` or `pylon tui`.

Public paid-training onboarding now has a minimum release:
`pylon-v0.1.10`, exposed through `@openagentsinc/pylon` version `0.1.10`.
That release is the current public release that has the pieces needed for this
claim: the bare `pylon` earning loop, npm bootstrap behavior that launches the
earning loop instead of the TUI, public signed-artifact transfer, accepted-work
payout projection, validator intake enabled by default, worker-first and
validator-second role claims, failed retained-runtime lease retirement,
nonfatal scheduler-error handling, default local Spark payout destination
creation in the long-lived serve path, retained snapshot reuse for validator
replay retries, Autopilot proof projection fixes, and terminal closeout
reporting before slow artifact publication can wedge the loop.

Nexus can now offer hosted starter work to online paid-training-capable Pylons
without asking the user to pick a course, enter a private credential, or run a
one-off assignment command. The production starter lane targets online Pylons
by `min_pylon_version=0.1.10`, skips exhausted or sealed starter runs, and does
not require the provider's build digest to match the Nexus service build.

The payout rule is also concrete. Pylon gets paid for accepted homework work.
Periodic placeholder payouts and liveness payouts are not part of this claim
and should stay disabled for it. A payment counts as proof only when it is tied
to an accepted outcome id for the completed training window.

Admins can pace the amount of work offered to online Pylons. Nexus exposes a
cron-safe homework dispatch endpoint that can create fresh starter runs at a
controlled interval, cap spend per call, and intentionally duplicate starter
work across intervals when that is useful for proving the earning loop or
metering early demand.

## How To Use It As A Pylon Operator

If `pylon` is already installed, check that it is new enough:

```bash
pylon --version
```

For paid hosted starter training, use `pylon-v0.1.10` or a newer release with
the same paid-training guarantees. Older versions may still bring up local
Gemma inference, but they are not sufficient proof for the hosted starter
earning claim.

If Pylon is not installed, use the npm bootstrap lane when `npm` is available:

```bash
npx @openagentsinc/pylon
```

That launcher resolves the matching Pylon release, verifies the published
checksum, caches the binary locally, runs the basic smoke path, and starts the
installed `pylon` binary by default. To pin the first paid-training release
explicitly:

```bash
npx @openagentsinc/pylon --version 0.1.10
```

After install, the normal provider command is:

```bash
pylon
```

Keep that process online. Pylon only receives hosted work while it is running
and eligible. When work is available, Nexus assigns the currently hosted
starter lane automatically. When the work is accepted, the Pylon wallet should
show a completed receive for the accepted outcome.

Useful inspection commands:

```bash
pylon status --json
pylon training status --json
pylon wallet balance --json
pylon wallet history --limit 20 --json
```

The current training path still needs the local training runtime to be
discoverable. In practice that means a compatible Psionic runtime checkout must
be reachable by Pylon until that runtime is bundled more tightly. Pylon checks
`OPENAGENTS_PSIONIC_REPO`, sibling checkout paths, common home-directory paths,
and worktree paths. If detection fails, `pylon training status` and
`pylon doctor` report the exact runtime discovery problem.

Do not set `GOOGLE_APPLICATION_CREDENTIALS`,
`OPENAGENTS_PYLON_TRAINING_GCS_BEARER_TOKEN`, or an operator Nexus bearer
token for the public path. Public artifact transfer should go through
Nexus-brokered signed URLs.

## How To Pace Work As A Nexus Admin

The public operator does not run an admin command. Admins can seed and meter
starter work behind the scenes with the homework dispatch endpoint:

```bash
curl -X POST "$NEXUS_BASE_URL/v1/admin/homework/cs336-a1/dispatch" \
  -H "Authorization: Bearer $NEXUS_CONTROL_ADMIN_BEARER_TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "run_count": 3,
    "max_contributors_per_run": 1,
    "amount_sats": 7,
    "total_budget_sats": 21,
    "run_slug_prefix": "cron.hourly",
    "reuse_existing_run": false
  }'
```

This endpoint is safe to put behind cron when the goal is to govern the pace of
available starter work and payouts. By default it creates fresh CS336 A1
homework runs, so the same starter assignment can be offered again across
intervals. `run_count` controls how many runs to create, `max_contributors_per_run`
controls how many contributors each run admits, `amount_sats` controls the
accepted-work payout per contribution, and `total_budget_sats` caps spend for
the call.

The endpoint does not pay anyone at launch time. It only creates work. Treasury
pays after a contribution closes out as accepted work.

## The Production Proof

The proof receipt is checked in at:

```text
docs/reports/nexus/20260421-223232-issue-4413-public-pylon-proof.json
```

That proof used the public `@openagentsinc/pylon` package at version `0.1.7`,
resolved the `pylon-v0.1.7` release asset without using a cached binary, and
ran the worker with the bare command `pylon`. The worker did not use a
CS336-specific command, a direct GCS credential, or an operator-only Nexus
credential.

Production Nexus was running:

```text
us-central1-docker.pkg.dev/openagentsgemini/openagents-nexus/nexus-relay:2a7986b42d77
```

The worker Pylon completed four accepted homework outcomes and received four
completed 25-sat wallet receives, for 100 sats total in the local Pylon wallet.
The accepted-work payout records were confirmed and settled by treasury, and
the final treasury status showed no accepted-work pending payout count and no
accepted-work attention payout count.

One operational detail matters for repeatability. The successful public proof
kept Pylon state isolated with `OPENAGENTS_PYLON_HOME`, but used the normal
user `HOME` so the installed Rust toolchain and sibling Psionic checkout were
discoverable. A fully synthetic `HOME` hid `rustup` state and failed before the
real hosted-work path could prove anything.

## What This Does Not Claim Yet

This does not claim arbitrary public training jobs are live. The current paid
training lane is bounded hosted starter work.

This does not claim raw GPU rental is live. Pylon advertises capability
envelopes and executes specific work classes; it is not a generic accelerator
exchange.

This does not require an OpenAgents web login. Account linking is optional and
only needed when an operator wants dashboard visibility.

This does not mean every machine will earn immediately. Pylon must be online,
the local runtime prerequisites must be discoverable, Nexus must have starter
work available, and treasury must have enough wallet balance to pay accepted
work.

This does not count placeholder payments as success. The user-facing proof is
accepted homework work followed by a wallet receive tied to that accepted
outcome.

## Troubleshooting

If `pylon` starts but receives no work, first check:

```bash
pylon status --json
pylon training status --json
pylon doctor
```

Common causes are straightforward: the Pylon release is older than `0.1.10`, the
node is not actually online, the local Psionic runtime cannot be found, Nexus
has no currently available hosted starter work, or production Nexus is not
running the hosted-starter fix set that matches the public Pylon release.

If work completes but the wallet does not show payment, inspect the accepted
outcome and payout projection rather than looking for unrelated wallet
activity:

```bash
pylon training status --json
pylon wallet history --limit 20 --json
```

The status should distinguish accepted work with payout pending from payout
confirmation failure, treasury balance failure, artifact authorization failure,
and ordinary runtime failure.

The short version is now true: install a current Pylon, run `pylon`, keep it
online, and accepted hosted starter training work can pay sats into the local
Pylon wallet.
