# Archil against our Computers stack: use, supplement, or build

This assessment asks a narrow question: given what OpenAgents already runs for
cloud computers, what would Archil add, and should we use it, build our own
version, or skip it? It builds on the [Archil research](research.md) and on a
hands-on trial on 2026-10-18.

**Short answer:** don't replace anything. Add Archil as one more driver behind
the existing `computer-driver` trait, next to `oa-boat` and SPACE, and qualify
it with one measurement: warm fork to first `cargo build`. Don't build our own
distributed filesystem. If we build anything of our own, it's the sandbox half
(Firecracker with memory snapshots), which the SPACE work already started.

## What we already have

### The Computers API is mostly built

The Computers API is deployed for admin and named-agent accounts since
2026-10-16 ([contract](../../sdk/computers.md),
[service](../../cloud/computers.md),
[delivery plan](../../sdk/delivery-plan.md)):

- Create, list, stop, wake, delete, and capabilities (#11801); commands with
  idempotency keys and cancellation (#11802); files up to 4 MiB (#11803); a
  21-operation OpenAPI subset (#11804); the Rust client (#11805); web chats
  (#11806); placement `auto`, `our-cloud`, and `fast-sandbox` (#11809).
- Every backend implements one trait, `crates/computer-driver`, with four
  declared profiles: Core, Suspend, Snapshot, and Fork. `admit()` refuses a
  profile a driver doesn't declare, and `crates/computer-conformance` tests
  each profile.
- Not in the contract: ports, previews, interactive terminals, and stdin.
  Public Snapshot and Fork stay refused "unless the exact provider operation
  and ownership semantics are implemented and qualified".

This matters for the Archil question. The product surface, accounts, grants,
billing hooks, and SDK already exist and are ours. Archil can only ever be a
backend under them.

### We already built our own Box once

"Box" was Ascii's sandbox product, since renamed Boat (boat.dev). We have used
it three ways ([Boat SDK plan](../../cloud/2026-10-02-boat-sdk-plan.md),
[oa-boat](../../cloud/oa-boat.md)):

| Backend | What it is | Status |
| --- | --- | --- |
| Hosted Boat | `crates/boat` SDK against boat.dev | Deprecated after its API returned 502s (#11256); opt-in with `BOAT_HOSTED=1` |
| `coder-box` | A 59-operation Box-compatible server on QEMU/KVM, private repository | Paused: 116.6 s cold, 27.5 s warm |
| `oa-boat` | Our Boat-compatible API on Cloud Run; each sandbox is a whole GCE VM from the daily `oa-coder-host` image | Production default |

`oa-boat` declares all four profiles: stop keeps the boot disk, resume starts
the same VM, and fork copies the disk through a snapshot. It measures:

- Create to ready: 34 to 65 s, and 1 to 4.5 minutes in a busy zone.
- Warm image fork: ready in 33 s, first `cargo build` in 93 s, $0.0104 total,
  "no lazy restore to wait for".
- Chat computer: 77 s to first turn, 52 s to resume.
- `large` (n2d-standard-8): $0.338/h, $0.172/h on spot. A stopped 200 GB disk
  costs $0.027/h.

Its known gaps are n2d capacity shortfalls in us-central1 (#11600), a single
in-memory service instance, and no memory preservation.

### SPACE is our Firecracker spike

The optional SPACE driver (Perplexity's Firecracker) measured 2.0 s p50 cold
create to first command and 319 ms p50 from snapshot to ready, and kept a
1 GiB buffer across same-host suspend and wake. Cross-host restore and worker
loss are untested, so `oa-boat` stays the default ([space](../../cloud/space.md)).

## The lesson that decides this: lazy restore

Boat failed us on exactly the workload we care about. Its snapshots restored
lazily over FUSE, and a warm Cargo target turned that into a stall:

- A 108 GB template took 76.4 s to fork, then 701 s to the first build.
- Pruned to 33 GB, restore took 72 s on the quietest fork and 830 s on the
  slowest; a typical fork reached its first build in 3 to 9 minutes.
- Truncated modification times forced 53-crate rebuilds (#10251, #10274).

`oa-boat` won because a GCE disk from an image has no lazy restore to wait
for. Archil is also lazy by design: data not in a client or server cache is
fetched on access. Archil's server-side SSD tier and subtree ownership should
behave better than Boat's restore, but that's a claim, not a measurement. The
pilot must test this exact case before anything else.

## What Archil would add that we don't have

1. **Memory-preserving pause and resume.** `oa-boat` keeps disks only. An
   Archil pause keeps processes, so a long Claude Code or Foreman turn can
   survive an idle stop. SPACE showed this on one host only.
2. **Fork of a running machine.** A fork resumes from the source's CPU,
   memory, and disk state. We have no equivalent.
3. **Seconds-scale starts without our own Firecracker fleet.** In the trial,
   sandboxes reached running in seconds, against 34 to 65 s for `oa-boat`.
4. **Filesystem branches billed copy-on-write**, for many attempts on one warm
   workspace.

It would not add accounts, grants, the API, billing, or the SDK. We have
those.

## What argues against it now

- **Vendor risk, again.** Hosted Boat's outage is why `oa-boat` exists.
  Archil is a two-year-old company with $18M raised, and its docs disagree
  with its product. In one hour on 2026-10-18 we hit:
  - API keys scoped to one region;
  - sandboxes refused in gcp-us-central1 ("Sandboxes are not enabled on this
    controlplane"), so compute is AWS-only;
  - attaching a healthy disk to a sandbox failing at boot on every image and
    path;
  - background processes killed when the starting command returned;
  - the free-plan firewall blocking rustup, with apt offering Rust 1.63.
- **Price per hour.** At published rates, 8 vCPU and 32 GiB cost $1.08/h,
  about 3.2 times `large` on `oa-boat` and 6 times spot. Per-second billing
  and pause narrow this only for bursty or idle-heavy work.
- **Branches can't be deleted yet.** Branch-per-task storage would only grow.
- **Branches share disk credentials.** Isolation needs one disk per account,
  which multiplies disks.
- **Location.** Archil compute is AWS-only; our database and services are in
  GCP us-central1. Control traffic is small, but moving workspaces between
  the two costs egress on both sides.
- **Credential custody.** A Claude login inside a third-party sandbox carries
  the same restriction we recorded for Boat: the owner's own plan only.

## Could we build our own Archil?

Archil is two products, and the answer differs for each.

| Half | What it takes | Our position |
| --- | --- | --- |
| Distributed filesystem: subtree ownership, multi-zone SSD tier, write-ahead log on object storage | Years of storage engineering by an ex-EFS team; the core engine isn't open source | Don't build. GCE disks and images cover our single-writer workspaces, and Loro rooms on `coder-sync` hold shared session state |
| Sandboxes: Firecracker, memory snapshots, fork of running state | Firecracker on nested-virtualization or metal GCE hosts, snapshots to GCS, disk snapshots for fork | Feasible. We ran Firecracker on a nested-virtualization GCE host before the reset (about 1 s boot), and the SPACE driver is closed on code |

So a do-it-ourselves Archil would be the SPACE path finished: our own
Firecracker driver, in our cloud, behind the same trait. That's the only part
where owning it pays off, and only if memory pause and resume turn out to
matter in practice.

## Recommendation

1. **Add an Archil driver, off by default.** Put an `archil` driver in
   `crates/computer-driver` behind a cargo feature, like `space`. Declare
   Core, Suspend, Snapshot, and Fork, and run `computer-conformance` against
   it. Use the Rust SDK being ported in the trial sandbox, vendored as
   `crates/archil` with Archil's MIT notice.
2. **Run one decisive measurement.** Fork a sandbox holding the openagents
   checkout with a warm target, and time fork to ready and to the first
   `cargo build`. The bar is `oa-boat`: 33 s plus 93 s. If Archil can't beat
   that, stop.
3. **Then test what only Archil offers.** Pause a sandbox mid Claude Code
   turn, resume it, and confirm the turn continues. Fork a running sandbox
   and run two attempts from one warm state. Record cost per issue next to
   the GCE pool's $0.0017 one-file issue.
4. **Keep `oa-boat` the default.** Archil goes behind placement
   `fast-sandbox` or an explicit choice, never the only path, so another
   vendor outage costs us a fallback rather than a rebuild.
5. **Decide build or buy after the pilot.** If memory pause and resume
   matter and Archil's price or reliability doesn't hold up, finish our own
   Firecracker driver instead.

## Trial notes (2026-10-18)

- Claude Code 2.1.302 ran headlessly in an Archil sandbox (aws-us-east-1,
  4 vCPU, 8 GiB, 1 TB Archil-backed root disk), signed in with our Claude
  Code token, and answered a smoke prompt.
- Sandboxes from the `rust:bookworm` Docker Hub image get a current Rust
  toolchain; the default image's apt Rust is 1.63.
- Long tasks must start through the process API (`sandbox.run` and then
  disconnect), not a backgrounded shell command.
- The agent's workspace and session history live on the sandbox root disk,
  because attaching a separate disk failed.
