# Build hosts: why builds keep failing, what machines we have, and how to get dev boxes

Status: audit, 2026-11-13, 9:30 AM CST. Written after a morning in which the
owner's Mac hard-reset once and became unusable once, no commit on `main`
finished its check for hours, and every agent built on the Mac.

## The short version

1. **The build hosts were switched off by a spend cap that was measuring
   the wrong number.** The cap said we had spent $85 in 24 hours against a
   $75 limit. The real figure was at most about $25. With six hosts the pool
   can't spend more than $39.60 in a day at all, so a $75 cap can only ever
   trip by mistake.
2. **When the cap trips, the work doesn't stop. It moves to the Mac.** That
   is by design, and it is the wrong design for a machine that also runs
   five agents, three dev servers and the owner's browser.
3. **The hosts have no GPU, so every headless browser runs on the Mac
   anyway.** That is where this morning's runaway (590 headless browsers)
   happened.
4. **We already own a usable dev box:** `coderos-4080`, a Linux machine with
   a GPU on the tailnet. The build hosts can become dev boxes too, with a
   small amount of setup, but spot machines are the wrong kind for a session
   that must survive.

Fixing the meter is a few lines and should happen today. Dev boxes are a
day's work. Both are listed at the end.

## What machines we have

### Ours, on the tailnet

| Machine | What it is | State | Used for |
|---|---|---|---|
| `macbook-pro-m5` | M5 Max, 18 cores, 128 GB, 1.8 TB (73 GB free) | On | Everything: the owner's work, every agent session, dev servers, headless browsers, releases and deploys, and builds whenever the hosts are off |
| `coderos-4080` | Linux, RTX 4080 | On | The pipeline's watcher (`after.sh --watch`), Coder runs, decision models |
| `zephyr` | Windows | Offline, seen 1 day ago | Not part of any build path |
| `claude-render-lab`, `devin-oa`, `node` | Linux | Offline 10 to 46 days | Nothing |

### Cloud (GCP `openagentsgemini`, us-central1)

| Machine | Type | Pricing | State | Role |
|---|---|---|---|---|
| `oa-build-1` to `oa-build-6` | `c3d-highcpu-30` (30 vCPU, 59 GiB), 200 GB disk each | Spot, $0.275 an hour running, $20 a month each for the disk | Start on demand, stop after 15 idle minutes | Remote builds and the green-main checks |
| `coax-publish-linux` | `n2d-standard-16` | On demand | Stopped | Publishing Coax |
| `psionic-l4-elephant` | `g2-standard-8` (L4 GPU) | Spot | Stopped | Psionic (paused) |
| `psionic-rtx6000-pylon` | `g4-standard-48` | Spot | Stopped | Psionic (paused) |
| `oct30-att-tdx-clef` | `c3-standard-8` | On demand | Stopped | Clef (paused) |
| `oa-room-1`, `oa-relay-1`, `oa-pay-1`, `oa-lnurl-1`, `oa-lnd-1`, `oa-lnd-tower-1`, `oa-bitcoind`, `immortal-public-regtest-1` | `e2` and `n2` small | On demand | Running | Production services, not build machines |

The build pool is six spot hosts with three job slots each: 18 slots. Hosts
`oa-build-7` to `-10` existed during the 11-12 sprint and are gone.

## What went wrong today

In order:

- **About 6:30 AM.** The Mac hard-reset. The logs stop mid-activity with no
  shutdown and no panic report.
- **7:56 AM.** The Mac became unusable: load average 250, 3,375 processes.
  A check (`drive-hit-check`) relaunched itself without limit: 347 copies
  and about 590 headless Chrome processes. The bug landed on `main` early
  this morning (68f3b21806) and the check runs in the pre-push hook, so it
  is the likely cause of the 6:30 reset too. Fixed in 7f71f9f758.
- **All morning.** `remote.sh status` showed all six hosts `TERMINATED` and
  "about $85.50 over the last 24 hours (the most: $75)". Every agent's
  pre-push hook printed "coax test skipped: no build host has a free slot".
  Agents reported building on the Mac one module at a time.
- **All morning.** The after-landing check started a run for each new head
  of `main` and each ended "unfinished" within seconds. `main-green` stayed
  at c2ac7652b5 while `main` moved 28 commits ahead (#13461).
- **9:16 AM.** The cap was raised to $150 for the day (333ef646af). Five
  hosts started within a minute and one combined run on the head began.

## Why: five causes

### 1. The cap measures a ledger that overcounts by about three and a half times

`remote.sh` never asks GCP what the hosts cost. It keeps a ledger and adds
it up (`spent`, `under_budget`). The ledger is wrong in three ways:

- **Each job is charged the whole host.** A host runs three jobs at once.
  Each job's wall time is recorded at the host's full price, so three
  parallel jobs cost three hosts. In the last 24 hours the ledgers hold
  about 140 job-hours, but the hosts were busy for only 78.4 hours once
  overlapping jobs on the same host are merged.
- **The Mac's ledger is added more than once.** Each machine's ledger is
  copied to a shared bucket under an id made from its hostname. The Mac's
  hostname has changed (`ChristohersMBP2`, `Christophers-MacBook-Pro-2`,
  and today's), so the bucket holds old copies of the same ledger under
  other names, and `spent` adds them as if they were other machines. The
  script reported $85.57; the same rows with duplicates removed come to
  $54.24.
- **Every start is charged 15 idle minutes** whether or not the host sat
  idle.

| How you count the last 24 hours | Figure |
|---|---|
| What the script reported | $85.57 |
| The same ledger rows, duplicates removed | $54.24 |
| Hours the hosts were really busy (78.4) plus every idle tail (11.5), at $0.275 | $24.73 at most |
| The most six hosts can cost in a day, running flat out | $39.60 |

The doc says the estimate "errs high" on purpose. Erring high is fine for
a report. It is not fine for a switch that turns every host off.

### 2. At the cap, heavy work falls back to the Mac

`docs/metaverse/remote-builds.md`, "When the Mac runs the job": when the
spend estimate reaches the cap, when no host can start, or when a host
can't run the job, `remote.sh run` runs the command on the Mac. So a tripped
cap doesn't save money so much as move compiles, test suites and API builds
(each with its own Postgres) onto the machine that has the least room.
Callers that pass `--no-fallback` (the pipeline) fail instead, which is why
the after-landing runs ended "unfinished".

### 3. Nothing says "the hosts are off"

The only signals were one line in the pre-push hook ("skipped: no build
host has a free slot") and an issue filed by the watcher. Neither names the
cause or what to do. Five agents each worked round it separately for hours.
Nobody, including the coordinator, looked at `remote.sh status` until the
owner asked.

### 4. The hosts can't run a browser that counts

The hosts have no GPU. Shots, goldens, frame times, `coax drive` and every
acceptance run stay on the Mac by design, because software WebGPU doesn't
match what the goldens were measured on. The doc notes that a
`g2-standard-8` spot host with an L4 could take them and that it isn't
built. So the most dangerous kind of job, a tool that starts browsers and
helpers, always runs on the owner's machine. This morning's runaway could
not have happened on a host we could simply delete.

### 5. Everything else also runs on the Mac

At 9:30 AM the Mac was running five agent sessions, their worktrees
(each with its own compiler build and page build), three dev servers, two
local APIs with Postgres, headless browsers, an API deploy and the build
host client. The disk has 73 GB free of 1.8 TB. The hosts being off was the
trigger; the standing problem is that the Mac is the only machine that can
do several kinds of work.

Other things that make the pool less dependable, from the ledger:

- **Jobs that never ran:** 44 on 11-09, 12 on 11-10, 106 on 11-12, 2 so far
  today ("failed to run").
- **Spot hosts** can be taken away mid-job. The pipeline retries a check on
  another host, which costs time.
- **A cold host** takes about 4 minutes to create and about 1 minute to
  start, and it stops after 15 idle minutes, so a quiet half hour means the
  next push waits for a boot.

## What it actually costs

- **Running:** $0.275 an hour for each host that is up. Yesterday and today
  that was about 90 host-hours in 24 hours, about $25.
- **Standing:** six 200 GB disks, $120 a month, whether or not anything
  runs.
- **The ledger's 30-day total is $336.** For the reasons above the real
  compute figure is lower, probably under half. The billing export is the
  only exact number; this audit did not read it.

The pool is cheap. The spend cap has cost far more in lost work than the
hosts have cost in money.

## Dev boxes: doing real Claude Code work off the Mac

What a dev box needs that a build host doesn't:

| Need | Build host today | What it takes |
|---|---|---|
| A session that survives | Spot: can vanish at any time. Stops itself after 15 idle minutes | Either an on-demand machine, or spot with every session in `tmux` and all work pushed often |
| A real checkout | A blob-less clone used as a job target | A normal clone with worktrees |
| Tools | The Nix toolchain and Bun | Also `claude`, `gh`, and the repo's hooks |
| Logins | None: the VM has no account and no scopes | A Claude Code login, a GitHub token scoped to the repo, and nothing else. No production secrets, no deploy rights |
| A browser that counts | None | A GPU |
| A way in | SSH by one key from the Mac | The same, or the tailnet |

Three ways to get there, cheapest first:

1. **Use `coderos-4080` now.** It is on, on the tailnet, has a GPU and
   already runs the pipeline's watcher. It can hold Claude Code sessions in
   `tmux` today. Its limits are known: don't delete its release target
   directory, and keep heavy work to two jobs at once. Cost: nothing.
2. **Add one GPU host for browsers.** A `g2-standard-8` spot machine with
   an L4, started on demand like the build hosts. Shots, `coax drive`,
   acceptance runs and goldens for agents move there. Goldens need one
   re-measure on that GPU. The doc's figure is about $0.30 an hour. One
   such machine already exists, stopped (`psionic-l4-elephant`); a new one
   for this purpose is cleaner than reusing it. This is the change that
   takes headless browsers off the Mac.
3. **Make one or two build hosts into standing dev boxes.** Same machine
   type, a normal clone, `tmux`, a Claude login. The choice is the pricing:

   | Pricing | An hour | 12 hours a day | Always on | Catch |
   |---|---|---|---|---|
   | Spot | $0.275 | about $100 a month | about $200 a month | Can be taken away mid-session; work must be pushed often |
   | On demand | $1.12 | about $400 a month | about $810 a month | None |

   Spot with `tmux` and frequent pushes is the sensible start. The agents
   already work in small pushed steps.

With that in place the Mac's job is to hold the conversation and the
browser the owner looks at. Agents on a dev box build locally there (30
cores, warm caches) and never fall back to the Mac.

GCP stays under the $1,500 a month rule in every option above. The dev box
options need the owner's yes before any machine is created.

## What to change

Today, no new resources:

1. **Meter host-hours, not job-hours.** Charge each host for the time it is
   up, once. Merging overlapping jobs per host is enough, and it is what
   the figures in this audit used.
2. **Give each machine's ledger a stable id** that doesn't change with the
   hostname, and remove the stale copies from the bucket.
3. **Make the cap follow the pool.** With six hosts the ceiling is $39.60 a
   day. Either drop the dollar cap and keep the host count as the one
   control, or set the cap from hosts × rate × 24 so it can't trip early.
   Then put the default back from today's $150.
4. **Never fall back to the Mac without saying so.** When hosts are off,
   `remote.sh run` should stop with one line that names the cause and the
   command to fix it. Running on the Mac should be something a caller asks
   for (`--local`), not something that happens.
5. **Say it where people look.** `remote.sh status` in the pre-push output
   when a check is skipped, and the watcher's issue should name "the pool
   is off: the cap" instead of "the hosts could not finish".
6. **Guard the Mac.** A process-count guard beside the existing file-table
   guard, so a runaway is stopped at a few hundred processes instead of
   3,000.

Next, with the owner's yes:

7. Set up `coderos-4080` as the first dev box: a clone, `tmux`, a Claude
   login, and a one-line way to start a session there.
8. One GPU spot host for headless browsers, and move agents' shots and
   drive runs to it.
9. One spot dev box from the build pool, and measure a day of real work on
   it before adding a second.

## What this audit did not check

- The GCP billing export. Dollar figures are computed from the ledgers and
  the list prices in `remote-builds.md`.
- How often spot hosts were taken away. The ledger doesn't record it.
- The 6:30 AM reset's cause. The runaway check fits, and the logs can't
  confirm it.
- Whether `coderos-4080` has the disk and memory for several agent
  sessions at once.
