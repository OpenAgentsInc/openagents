# Pylon Field handoff

Status as of October 7, 2026. Work on the Pylon Field stopped here on the
owner's instruction. This page says what landed, what remains, and how to
pick it up. The plan is [Compute in the Verse](verse-compute.md); the guide
to running a pylon is [Run a Pylon, and use one](pylon.md).

October 8 coordination checkpoint: P1 remainder #10921, P2 #10922,
P3 #10923, P4 #10924, and umbrella #10925 remain open. The issue and claim
audit prepares their briefs, but this run stops new assignments at
89 percent shared usage. Recheck `gh issue view N` and
`openagents issue status N` before claiming. The existing 4080 provider
and Psionic services are retained. P1's acceptance gaps below still apply;
an existing `pylon ask` receipt does not establish Alice's pool routing.

## What landed

| Commit | Issue | What |
| --- | --- | --- |
| `ea29ab9812` | #10920 (P0) | The Pylon Field and the Wellspring in Everglade's north woods, drawn from this computer's lease table through `ComputeSource`. |
| `737f94a17c` | #10921 (P1) | Psionic's serving path imported into `crates/psionic`. |
| `a6eb9acadc` | #10921 (P1) | `crates/pylon`: beacons, free NIP-CJ jobs on Psionic, receipts, pool aggregates, `RelayField`, and `openagents pylon`. |
| `8b99221ed5` | #10921 (P1) | The owner's decision that pylon traffic uses the production relay. |
| `aece703aec` | #10921 (P1) | The relay source in Verse, the beam for this computer's pylon jobs, and the visual polish. |

`aece703aec` adds:

- `pylon::field::Live` and `RelayField::watch`: one background
  subscription to a pool's beacons, receipts, and aggregates that verifies
  each record, holds bounded state, and reconnects.
- `pylon::inflight`: while `openagents pylon ask` waits for an answer, it
  leaves a mark under `~/.openagents/compute/inflight/` that names the
  pylon, never the prompt.
- `nostr::pylon::parse_aggregate`, which verifies a `30201` event without
  recomputing it.
- `zones::everglade::compute::relay::RelaySource` (feature `pylon-relay`,
  on in Verse's `desktop` feature) and `compute::Merged`. The desktop draws
  this computer's pylon beside the relay's pylons.
- The beam to Alice's station shows while her studio seat works or one of
  this computer's pylon jobs is in flight, with a fork from the serving
  pylon.
- Visual polish: carved rune bands that light one per busy slot, crystal
  tips, a stream of light to the basin while a pylon serves, a wider
  basin (`BASIN` is 2.6 m) on a stepped plinth with shafts of light,
  rising motes, ripple rings, and caustic light on its wall and the
  standing stones, a real lamp that lights the stones and ground at night,
  and a continuous ribbon for the beam. The field draws at most 320 glow
  quads.

## What's open

Issue #10921 stays open. Its acceptance and where each item stands:

- A second machine's pylon appears in another client's field within 60
  seconds: met. The RTX 4080's beacon reaches this Mac's field through the
  subscription in seconds.
- A free job from Alice produces a receipt that the next aggregate counts:
  partly met. `openagents pylon ask` produces a receipt that the aggregate
  counts, and the beam shows while it runs. Alice's own studio work doesn't
  route to the pool yet; that needs a Coder delegate target for the pylon.
- A tampered receipt or aggregate is refused in a test: met
  (`nostr::pylon` tests and `pylon::field` tests).
- Captures `pylon-field-two-machines.png` and `wellspring-live.png`: met.
  See [Capture paths](#capture-paths).

October 8: `openagents host share on|off|status`, the `pylon` lease
resource at `background` priority with draining while the owner's work
needs the computer, and NIP-OA owner tags on beacons (`openagents pylon
link`) landed, and `openagents pylon route on` sends Alice's and the
crew's day plans to the pool as free jobs, falling back to their own
model. The web and phone builds leave the field dormant.

## Run and test

Use the build lease for every Cargo command:

```sh
openagents lease build --keep-target-dir -- cargo test -p pylon
openagents lease build --keep-target-dir -- \
  cargo test -p verse-zone-everglade --features pylon-relay -- compute:: world_tree
```

Send a job to the pool, and see what the relay shows:

```sh
openagents pylon status
openagents pylon ask "Name three planets."
openagents pylon pool --publish
```

Render the live field offscreen, with this computer's real lease table and
the relay's pylons. `VERSE_CAPTURE_COMPUTE_WAIT` waits for a busy relay
pylon (`busy`), a job in flight (`job`), or both; send jobs from another
shell while it waits:

```sh
openagents lease build --keep-target-dir -- \
  cargo build --release -p verse --example everglade_capture
VERSE_CAPTURE_COMPUTE=live VERSE_CAPTURE_COMPUTE_WAIT=busy,job \
  "$CARGO_TARGET_DIR/release/examples/everglade_capture" field.png pylons
```

Add `VERSE_TOWN_HOUR=22` for night, and use the `wellspring` view for the
basin. `VERSE_CAPTURE_COMPUTE=demo` draws the labeled DEMO pool.
`VERSE_PYLON_RELAY` names another relay for Verse, or `off`.

### The RTX 4080 pylon

`coderos-4080` runs two transient user units, `pylon-psionic` and
`pylon-provider`, from `~/work/pylon-p1` on the box. Check them, and stop
them:

```sh
ssh coderos-4080 'systemctl --user status pylon-psionic pylon-provider'
ssh coderos-4080 'PYLON_DIR=~/work/pylon-p1/run PYLON_TARGET=~/work/pylon-p1/target ~/work/pylon-p1/openagents/scripts/pylon-psionic.sh stop'
```

Stopping publishes an offline beacon. A reboot also stops them, since no
unit file is installed.

## Interfaces

- `zones::everglade::compute::ComputeSource`: `fn sample(&mut self, now:
  u64) -> Sample`. A `Sample` holds `PylonSample` records, the capacity
  book's wells, the pool's job rate, the pool's name, `in_flight` (the
  pylon IDs running this computer's jobs), and `verified` (the pool's
  aggregate recomputed). Install a source with
  `WorldRuntime::set_compute_source`. `Merged` combines sources, the first
  source's pylons taking the first sites.
- `pylon::field::RelayField`: `poll()` fetches once; `watch(&live, &stop)`
  subscribes until `stop` is set. `Live::offer` takes relay events,
  `Live::pylons(now)` returns verified pylons with stale ones as
  `unknown`, `Live::rate(now)` gives jobs a minute (from a valid
  aggregate, else from receipts), and `Live::aggregate(now)` returns the
  aggregate and whether it recomputed.
- `pylon::inflight`: `Mark::new(home, &job)` and `read(home, now)`.

## Known issues

- A job on the 4080 takes about 2 seconds, and a beacon republishes at most
  every 10 seconds, so a beacon often shows a pylon idle while it serves.
  The beam and fork come from the client's own marks, so they are exact
  for this computer's jobs.
- An aggregate seldom recomputes in Verse: the relay keeps only the newest
  beacon, so once the pylon republishes, the counted beacon is gone. The
  rim's runes stay dark, which is honest but rarely lit.
- Two beacons sampled in the same second don't replace each other in
  `BeaconBook`.
- A pool rate below one job a minute draws a ripple rarely.
- On sloped ground the basin's plinth shows a wedge on its downhill side.
- Generated faces dim at night by a fixed factor rather than being lit.

## Owner steps

None new. `NEEDS_OWNER.md` keeps the note on how to stop the 4080 pylon.

## Capture paths

In `/private/tmp/claude-501/pylons/live/`, taken while this Mac sent jobs
to the 4080 with the `pylon ask` client:

- `pylon-field-two-machines.png` and `pylon-field-two-machines-night.png`:
  this Mac's spire and the 4080's obelisk, 1 of 2 slots busy, with the beam
  and its fork.
- `wellspring-live.png` and `wellspring-live-night.png`: the basin with the
  pool's rate from the published aggregate.
- `demo-pylons.png`, `demo-pylons-night.png`, and
  `demo-wellspring-night.png`: the labeled DEMO pool.

Each capture's `.log` records what the relay showed. P0's captures are in
`/private/tmp/claude-501/pylons/`.
