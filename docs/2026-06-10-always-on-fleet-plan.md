# Always-On Fleet Plan: Machines That Stay Online

Date: 2026-06-10. Owner directive: the blocker audit
(`docs/2026-06-10-oldest-open-issues-blocker-audit.md`) showed eight of
the eleven oldest issues converge on one missing event — a Pylon that
stays online. The directive is to gain the ability to spin up machines
that DO stay online, using whatever works: local machines, Tailnet
remotes, SHC, gcloud. This is the plan. Windows/WSL is deliberately out
of scope by the same directive (strongly deprioritized; removed from
the registry blocker and the platform matrix in the same change).

## What "online" means, precisely

The dispatch gate (and the Artanis administrator tick, live since
tonight) considers a Pylon eligible when ALL of:

1. registered (`pylon_api_registrations` row),
2. fresh heartbeat (the tick uses a 10-minute window),
3. executor capability declared (rc2 auto-declares at `go-online`),
4. wallet readiness reported (rc2 auto-claims tip readiness with it).

So "a machine that stays online" = a supervised process that runs the
Pylon runtime, heartbeats forever, and restarts itself after crashes,
reboots, and sleep. Everything else is already automatic in rc2.

## The ladder (cheapest first, each rung independent)

### Rung 0 — this Mac, tonight (zero new infrastructure)

The operator's own machine, supervised by launchd so macOS restarts it
on crash and login:

- `~/Library/LaunchAgents/com.openagents.pylon.plist` with
  `KeepAlive: true`, `RunAtLoad: true`, running the packaged pylon (or
  `bun src/index.ts` from the checkout) with `PYLON_OPENAGENTS_BASE_URL`
  and `OPENAGENTS_AGENT_TOKEN` in the environment.
- One-time: `pylon go-online` (declares capabilities) and
  `pylon wallet report-readiness` (reports + auto-claims tip readiness).
- Sleep is the enemy on laptops: `caffeinate -s` in the plist command
  or `sudo pmset -a sleep 0` on desktops. The iMac Pro (below) is the
  better permanent host for exactly this reason.
- Acceptance: `pylonsOnlineNow >= 1` on `/api/public/pylon-stats` for
  24 unbroken hours, and the admin tick's first autonomous dispatch
  recorded in `artanis_admin_tick_decisions`.
- **Executed 2026-06-11 ~01:00 UTC.** Operational notes from the live
  bring-up, for every future host:
  - Use a dedicated `PYLON_HOME` per identity; the shared default home
    hits the registration-ownership 401 when another agent's token
    registered it first.
  - `pylon provider go-online` is required once per home (writes
    `lifecycle: online` and auto-declares the executor/NIP-90/labor
    capabilities); without it admission denies with
    `blocker.assignment.lifecycle_offline`.
  - The supervised loop is `presence heartbeat` + `assignment
    run-no-spend` every 60s (`run-no-spend` is the full
    poll→accept→execute→closeout cycle).
  - Within 25 minutes of bring-up the full autonomous span completed
    twice: mind-dispatched assignments `…011429`/`…011629` executed,
    worker-replayed byte-identically (digest `f2995c4e…`), and accepted
    to `accepted_work` on the digest predicate — zero humans.

### Rung 1 — Tailnet remotes (hardware we already own)

Per `TAILSCALE_SSH_RUNBOOK.md`: `imac-pro-bertha` (desktop, never
sleeps — the natural standing Pylon), `macbook-pro-m2`, and `archlinux`
(Linux coverage). For each: SSH in, install the packaged pylon, same
go-online + report-readiness, then supervise — launchd on the Macs,
`systemd --user` unit with `Restart=always` on archlinux (the Comunero
systemd lesson generalizes: a unit file is what finally kept their
wallet daemon answering invoice requests).

- Acceptance: 2–3 distinct `pylonRef`s online simultaneously; the
  fleet survives one deliberate `kill -9` per host (supervisor brings
  it back inside the heartbeat window).

### Rung 2 — SHC dispatch lane (the worker's own compute control)

The worker already carries an SHC control-plane config
(`SHC_CONTROL_API_URL`, `SHC_CONTROL_API_BEARER_TOKEN`,
`SHC_DISPATCH_MODE` with `shc_primary_only` and
`shc_primary_cloudflare_container_backup_gcloud_reference` modes, plus
a runner callback token). Extend the SHC runner image to boot a Pylon
alongside its runner duties: the same supervised process, registered
with an `shc.` pylonRef namespace. This gives programmatic
spin-up/teardown from the worker itself — the first rung where
**Artanis can request capacity** rather than a human provisioning it.

- Acceptance: one SHC-hosted Pylon registered and heartbeating,
  started and stopped through the control API.

### Rung 3 — gcloud burst capacity (paid, on demand)

A small always-free-tier or e2-micro instance with a cloud-init that
installs bun + the packaged pylon, writes the systemd unit, and joins
with a `gcloud.` pylonRef. The setup script now lives in
`apps/pylon/deploy/gcloud/setup-pylon.sh`, using IAP/SSH plus the existing
`apps/pylon/scripts/install-cloud-node.sh` installer so spinning N more is one
command once a compute-capable GCP credential is active. Used
when the lane needs guaranteed capacity (training windows, demos)
rather than as the default — owned hardware first, rented second,
matching the economics gates already in the training promises.

- Acceptance: scripted create → online → dispatched → destroyed cycle
  with the cost recorded. For #6089's proven-engine lane, use the script's
  optional `--accelerator type=count` path to create a GPU host, then install
  and verify vLLM/SGLang separately before emitting serving benchmark refs.
- 2026-06-23 live evidence: `pylon-gcloud-khala-6089-check` is online as a CPU
  GCloud Pylon, and the existing stopped L4 host
  `gswarm508-clean2-20260325044551-contrib` was repurposed with old startup
  metadata cleared, booted into the kernel with matching NVIDIA modules, and
  brought online as `gcloud.gswarm508-clean2-20260325044551-contrib`.

### Rung 4 — supervision as a platform feature (close the loop)

Once rungs 0–1 exist, the watching side: the capacity-funnel snapshots
already record online counts every tick. Add an Artanis responder-style
alert — when `pylonsOnlineNow` drops below the standing floor (initially
1), Artanis posts to its status topic and (rung 2+) requests an SHC
replacement. The fleet keeps itself awake; dark-capacity accounting
stays honest because every gap is a recorded event, not a silent zero.

## Sequencing and ownership

| Step | Action | Owner | When |
| --- | --- | --- | --- |
| 1 | Local launchd Pylon on this Mac, go-online, verify dispatch | agent (done tonight if env permits) | now |
| 2 | imac-pro-bertha standing Pylon over Tailnet | agent | next session with Tailnet access |
| 3 | archlinux systemd Pylon (Linux evidence too) | agent | same pass as 2 |
| 4 | SHC runner image boots a Pylon | agent + SHC config owner | after 1–2 prove the shape |
| 5 | gcloud script in apps/pylon/deploy/gcloud | agent | when burst capacity is wanted |
| 6 | floor-watch alert in the Artanis tick | agent | after 24h of rung-0 uptime |

## What this unblocks (from the blocker audit)

The first standing Pylon: the admin tick's first autonomous
dispatch→verify→accept span (#4701/#4697 machinery, deployed and
waiting), movement on #4641/#4642/#4652 stream counters, the
evolution-loop promise's unattended streak, and the responder's
external-contributor flow gains a live device to talk about. Rungs 1–3
turn "a machine" into "a fleet with a floor."

## Boundaries

- Standing Pylons run with the operator's identity and wallet on owned
  hardware; SHC/gcloud Pylons use dedicated refs and never carry
  personal wallet mnemonics.
- Paid capacity (rung 3) is bounded and recorded; owned hardware is
  the default. No capacity claims in public copy beyond live
  `pylon-stats` counters — the no-overclaim gates extend unchanged.
- Windows/WSL: out of scope, not on this ladder, per the owner
  directive recorded above.
