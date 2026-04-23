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

Pylon can now run as the default user-facing earning dashboard. Bare
interactive `pylon` opens a minimal homework-focused TUI that initializes local
state, starts and supervises the worker process, marks the node online, exposes
local admin/status endpoints, publishes provider presence when possible, and
keeps provider and training intake running while the window stays open.

Public paid-training onboarding now has a minimum release:
`pylon-v0.1.11`, exposed through `@openagentsinc/pylon` version `0.1.11`.
That release is the current public release that has the pieces needed for this
claim: the minimal TUI-managed earning worker, npm bootstrap behavior that
launches that TUI after smoke checks, no automatic Gemma diagnostics or model
downloads during homework onboarding, public signed-artifact transfer, accepted-work
payout projection, validator intake enabled by default, worker-first and
validator-second role claims, failed retained-runtime lease retirement,
nonfatal scheduler-error handling, default local Spark payout destination
creation in the long-lived serve path, retained snapshot reuse for validator
replay retries, Autopilot proof projection fixes, and terminal closeout
reporting before slow artifact publication can wedge the loop.

Nexus can now offer hosted starter work to online paid-training-capable Pylons
without asking the user to pick a course, enter a private credential, or run a
one-off assignment command. The production starter lane targets online Pylons
by `min_pylon_version=0.1.11`, skips exhausted or sealed starter runs, and does
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

For paid hosted starter training, use `pylon-v0.1.11` or a newer release with
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
npx @openagentsinc/pylon --version 0.1.11
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

The release receipt for the current user-path floor is checked in at:

```text
docs/reports/nexus/20260423-072712-pylon-v0.1.11-release.json
```

That receipt proves the `pylon-v0.1.11` release asset, npm bootstrap behavior,
TUI-managed worker lifecycle, no default Gemma model download, and production
homework lease intake to released/sealed local closeout state. It is the proof
that a normal user can run `pylon` and get the simplified dashboard that keeps
the worker online. It is not the full accepted-work payout proof because the
smoke had training-relay publication retries pending after local closeout.

The primary fresh npm-installed accepted-work payout proof remains checked in
at:

```text
docs/reports/nexus/20260423-050434-pylon-v0.1.10-release.json
```

That receipt proves both publication and runtime behavior for
`pylon-v0.1.10`. The release was cut from
`8b814d800b6f4291892a1bcc835fb34a2b91fee1`, published as the GitHub release
tag `pylon-v0.1.10`, and published to npm as `@openagentsinc/pylon@0.1.10`.
The Mac release asset used for the proof was
`pylon-v0.1.10-darwin-arm64.tar.gz` with archive SHA-256
`a63a9ca8fa32dd05d9815f5087c19faa9a70f250b38a29d193274f07e9149e5d`.

The release was not accepted just because a package existed. It had a
source-gate pass, a release-asset smoke pass, and a production earning pass.
Before release, the focused source checks covered payout destination creation,
retained artifact replay, the Pylon build, the Autopilot build, Autopilot unit
tests, and the Tauri homework handshake control smoke. After release, the npm
bootstrap resolved the `pylon-v0.1.10` release asset without using a cached
binary, verified the published checksum, installed `pylon` and `pylon-tui`,
confirmed both help surfaces, and ran a long-lived `pylon` process that created
a local Spark payout destination and reported the default online earning loop.

The fresh production earning proof then used `@openagentsinc/pylon@0.1.10`,
the release asset from `pylon-v0.1.10`, an isolated Pylon home, a normal user
`HOME` so Rust and Psionic discovery worked, and a separate admin-triggered
hosted homework run. The worker reported to Nexus as:

```text
release id: openagents.pylon@0.1.10
build version: 0.1.10
build digest: sha256:7b1a2e79255ac893cdd6581a771b85293fb8485bde5892f7648ab4f79e6e1d84
```

The dispatch and payout proof recorded:

```text
network id: trainnet.cs336.a1.pylon-0.1.10-release.20260423T050920Z
training run id: run.cs336.a1.pylon010-prod-20260423051030_20260423051031_fa7d95a2_0001.20260423051031.46951e63
window id: window.cs336.a1.pylon010-prod-20260423051030_20260423051031_fa7d95a2_0001.20260423051031.46951e63.0001
latest closeout status: rewarded
featured window status: reconciled
accepted contributors: 1
payout eligible: true
payout receipt id: 019db8c1-6639-7751-a717-cee14dd2012e
payout reconciliation status: settled
treasury payout class: accepted_work
amount: 25 sats
worker wallet balance: 0 sats -> 25 sats
wallet receive status: completed
```

That is the current proof behind the user-facing claim. A normal operator can
install the current package, run `pylon`, stay online, receive hosted starter
training work when an admin dispatches it, and receive sats after the work is
accepted.

Earlier production receipts still matter as history. The first public package
proof for this path is
`docs/reports/nexus/20260421-223232-issue-4413-public-pylon-proof.json`, which
used `@openagentsinc/pylon@0.1.7` and proved the bare `pylon` command could earn
across four accepted homework outcomes. A later npm release-asset proof is
`docs/reports/nexus/20260422-035746-pylon-npm-e2e-fb60b91678ca.json`, which
used a newer npm-installed worker and a separate validator. Those receipts
explain the path to the current release, but `0.1.11` is the minimum public
release to recommend now.

One operational detail still matters for repeatability. The successful public
proof kept Pylon state isolated with `OPENAGENTS_PYLON_HOME`, but used the
normal user `HOME` so the installed Rust toolchain and sibling Psionic checkout
were discoverable. A fully synthetic `HOME` hid `rustup` state and failed before
the real hosted-work path could prove anything.

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

Common causes are straightforward: the Pylon release is older than `0.1.11`, the
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
