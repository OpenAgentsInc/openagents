# Everglade multiplayer launch runbook (2026-11-04)

This runbook takes Everglade multiplayer from tonight's deploy hold to a
launch with real players. It covers the deploy order, the capacity change for
#12717, the production checks to run after each step, the rollback for each
step, and the steps only the owner can do.

Epic: #12718. Spec: `docs/verse/2026-11-03-everglade-multiplayer-spec.md`.
All times are Chicago time.

## Before you start

- Run everything from a clean worktree of `origin/main`, never from the main
  checkout:

  ```sh
  cd ~/work/openagents
  git fetch -q origin
  git worktree add ../openagents-launch origin/main
  cd ../openagents-launch
  ```

- Every `gcloud` call uses the automation account:

  ```sh
  export CLOUDSDK_CONFIG=$HOME/work/.secrets/gcloud-sa-config
  ```

- Never print the seat-ticket key or a session cookie. The tools that need
  the key read it from Secret Manager into their own environment.
- One person runs each step. The lead runs `release.sh`.

### What production runs at the time of writing (2026-11-03, 20:30)

| Service | Live revision | Built from |
| --- | --- | --- |
| `oa-api` | `oa-api-00106-rll` | `978ef4f4c1` (includes migration 0118) |
| `oa-room` (shard 1) | `oa-room-00037-4kj` | `1fee391fc9` |
| `oa-room-2` to `oa-room-6` | `oa-room-K-00016-*` (listed under rollback) | `1fee391fc9` |
| Pages | release `978ef4f4c1` | `978ef4f4c1` |

Check these again before step 1, since another agent may have shipped since:

```sh
gcloud run services describe oa-api --region us-central1 --project openagentsgemini \
  --format='value(status.latestReadyRevisionName,spec.template.spec.containers[0].env)' \
  | tr ';' '\n' | grep -E 'oa-api-|OA_BUILD_SHA'
for k in 1 2 3 4 5 6; do s=oa-room; [ $k -gt 1 ] && s=oa-room-$k
  gcloud run services describe $s --region us-central1 --project openagentsgemini \
    --format='value(metadata.name,status.latestReadyRevisionName,spec.template.spec.containers[0].image)'
done
```

Write the revisions down. They are your rollback targets.

### What ships tomorrow

- **oa-room:** everything since `1fee391fc9` in `coax/web/room/` and in the
  page code the room links (`apps/openagents-web/`). This includes:
  - `4a3156c965`: events, levels and the Warlock's new spells.
  - `86808a350c`: a player's silent socket gives way to their new one.
  - `6a846432d2`: 256 players in 16 rooms hold, and a host stall no longer
    leaves seats 300 ms behind.
- **Pages:** everything since `978ef4f4c1`. This includes the abilities'
  batch 3 page half (`e73b4503a9`, `1fee391fc9`), the BLOCK smoke probes
  (`f696d556cb`), and the room-lost and rooms-full messages (`efdc754452`).
- **API:** nothing since `978ef4f4c1` at the time of writing. Migration 0118
  is already live. Step 1 is a no-op unless the check below finds new API
  commits or you take the capacity change (step 4).

## Step 1: API

### Check what would ship

```sh
LIVE=978ef4f4c1   # OA_BUILD_SHA from the check above
git log --oneline $LIVE..origin/main -- coax/web/api
git diff --name-only $LIVE origin/main -- coax/web/api/migrations
```

If both are empty, skip to step 2.

### Check that every new migration is additive

The API applies each migration it doesn't find in `schema_migrations`, in
name order, when it starts. Read each new file in full, then run:

```sh
for f in $(git diff --name-only $LIVE origin/main -- coax/web/api/migrations); do
  echo "== $f"; grep -n -iE 'drop |truncate|delete from|rename|alter .* type|not null' "$f" | grep -v '^\s*--'
done
```

Additive means it only adds tables, columns (`ADD COLUMN IF NOT EXISTS`, with
no `NOT NULL` lacking a default), indexes, or `CREATE OR REPLACE FUNCTION`.
Stop and ask the owner before you deploy anything that drops, renames,
truncates or deletes.

### Deploy

```sh
coax/web/api/deploy.sh
```

This deploys staging, runs staging's smoke, deploys production with the same
image, and runs production's smoke. It stops on the first failure.

### Checks after step 1

1. **Smoke:** `deploy.sh` ran it. It passes when the script ends without an
   error.
2. **Directory:** this prints `{"realm":"everglade"` followed by the
   instances:

   ```sh
   curl -s https://openagents.com/api/playtest/rooms/everglade | head -c 300
   ```

3. **Fan-out:**

   ```sh
   coax/web/tool.sh coax/web/room/fanout-check.coax https://openagents.com 17 5
   ```

   It passes when it prints the holding line and exits 0. That means all 17
   guests sat, instance 1 filled to 16 before instance 2 opened, the rooms
   list shows each instance on the shard its bots reached, and the list is
   empty once the bots leave.

### Rollback for step 1

```sh
gcloud run services update-traffic oa-api --region us-central1 --project openagentsgemini \
  --to-revisions oa-api-00106-rll=100
```

Use the revision you wrote down. An additive migration stays in the database
after a rollback, and the old API ignores what it doesn't use. Don't write a
reverse migration under time pressure.

## Step 2: oa-room on all six shards

```sh
git pull -q --rebase origin main
coax/web/room/deploy.sh
```

`deploy.sh` builds one image and deploys it to `oa-room` and `oa-room-2` to
`oa-room-6`. It then checks each shard's health and the load balancer rules,
and runs its own checks through `wss://openagents.com`, retrying for up to
5 minutes while the rule reaches every edge. Those checks are the room load
(4 players), `relay-check` (normal and slow), `sim-check`, `bridge-check`, the
32-player everglade load, the rooms list, every shard's health, `fanout-check`
with 17 guests, and the relay's p95. It ends with
`room: deployed IMAGE behind wss://openagents.com/api/room/`. Any other ending
is a failure.

Every page connected to a shard reconnects when its revision changes. Since
`efdc754452`, a page says LOST THE ROOM · REJOINING, then BACK IN THE ROOM, but
that message arrives only with step 3's page release. Deploy oa-room when few
players are on.

### Checks after step 2, in order

Export the seat-ticket key once for the checks that sign their own tickets:

```sh
export OA_ROOM_TICKET_KEY="$(gcloud secrets versions access latest --secret oa-room-ticket-key --project openagentsgemini)"
```

| # | Check | Command | Passes when |
| --- | --- | --- | --- |
| 1 | Smoke | `coax/web/room/everglade-proof.sh smoke` | Both pair runs (desktop, then phone) end in `holds`. Smoothness (#12731) is reported separately. Its result doesn't block this step. |
| 2 | Latency | `coax/web/room/everglade-proof.sh latency` | The round trip's p95 is under 250 ms (spec acceptance 1). The last measured p95 was about 145 ms. |
| 3 | Load | `coax/web/room/everglade-proof.sh load --players 64 --seconds 60 --shards 6` | Every socket opens. D arrives at 28 Hz or more in every room. Every seat sees every other. No ACK goes back. Input-to-ACK p95 is within 50 ms of the ping's p95. Every shard's everglade tick p95 is under 5 ms (spec acceptance 7). The proof ends with `everglade-proof: holds`. |
| 4 | Fan-out | `coax/web/tool.sh coax/web/room/fanout-check.coax https://openagents.com 17 5` | As in step 1. |
| 5 | Abilities | `coax/web/tool.sh coax/web/room/abilities-check.coax wss://openagents.com` | It prints `lifts`. Both sockets hear A's Reverse Gravity cast and get the server's cylinder. The server lifts A over 3 m within 3 s. B's own cylinder stands beside A's. A's second press ends only A's, and B's E ends B's. |
| 6 | Bridge | `coax/web/tool.sh coax/web/room/bridge-check.coax wss://openagents.com` | It prints `holds`. Both hear the same bridge pieces break. A walking over the broken deck is in the river on both sockets. After R, A walks over dry. |
| 7 | Shared world | `coax/web/tool.sh coax/web/room/sim-check.coax wss://openagents.com` | It prints `shares`. A's wolves, B's hits and the kill reach both sockets. The pack stays when A leaves. WANTED goes to B, not A. Breaks match up to the rebuild. Late joiner C has the broken town within 1 s. R is refused while the spell is fresh, then rebuilds. |

If `sim-check` fails only on `breaks_up` (A and B counting a different number
of breaks), run it once more. Tonight's investigation found no dropped frames.
The likely cause is the end-of-test window, where A closes 500 ms after the
rebuild while B listens for 800 ms more. A second failure is a real failure.

`everglade-pair` against production still runs the old page until step 3.
Steps 7 and 8b of the pair check skip or fall back where the old page lacks the
new probes. That is expected before step 3.

### Rollback for step 2

Put every shard back on the revision you wrote down. At the time of writing:

```sh
for p in oa-room:oa-room-00037-4kj oa-room-2:oa-room-2-00016-kbs oa-room-3:oa-room-3-00016-h8z \
         oa-room-4:oa-room-4-00016-kgd oa-room-5:oa-room-5-00016-trd oa-room-6:oa-room-6-00016-tw4; do
  gcloud run services update-traffic "${p%%:*}" --region us-central1 --project openagentsgemini \
    --to-revisions "${p#*:}=100" &
done; wait
```

Then run checks 4, 5 and 7 again. `deploy.sh` changes the load balancer only
by adding rules, so the rollback doesn't need to touch the URL map.

If step 2 fails, roll back the shards and hold step 3. The new pages expect the
new room server's events and level frames.

## Step 3: the page release

The lead runs this:

```sh
coax/web/release.sh
```

`release.sh` builds `origin/main` once, puts it on staging, runs `smoke.sh`
and the goldens' start check on staging and production, promotes the same
bytes, then runs the two checks again on production. It rolls back on its own
if production gets a new failure. It holds a lock across machines, so only one
release runs at a time.

### Checks after step 3, in order

| # | Check | Command | Passes when |
| --- | --- | --- | --- |
| 1 | Site smoke | `coax/web/smoke.sh https://openagents.com` | It exits 0. Every page and file answers with the right status, type and caching. |
| 2 | Pair, desktop | `coax/web/tool.sh everglade-pair` | It ends in `holds`. That covers steps 1 to 8 of the pair check: both join and see each other with names; the shared pack; B's kill and the same WANTED; move latency; the jump; Reverse Gravity cast with key 5 and ended; INVITE seating a third page in A's instance; and BLOCK hiding A at once and again after B reloads. Smoothness is reported, not judged here. |
| 3 | Pair, phone | `coax/web/tool.sh everglade-pair --phone` | The same checks as desktop at 390x844, DPR 3, touch. |
| 4 | Latency | `coax/web/room/everglade-proof.sh latency` | As in step 2. |
| 5 | Load | `coax/web/room/everglade-proof.sh load --players 64 --seconds 60 --shards 6` | As in step 2. |
| 6 | Abilities | the `abilities-check` command from step 2 | `lifts` |
| 7 | Events, START NOW and STOP | Owner only. See below. | See below. |

Then check by hand in one headless pair, or on two of your own devices:

- A Levitate (key 7) rises while held and hovers when let go.
- X sinks.
- Spell modes (Elementalism, Create or Destroy Water) look the same on both
  pages.
- Ice from Ray of Frost is slippery for both players.
- The Wind Wall icon counts up to 12 (`n/12`) and Wall of Stone up to 4. The
  count turns red when your oldest wall is replaced.

When steps 2 and 3 hold, post the evidence on #12733 and close it.

### What a local `everglade-pair` run can and can't show

A local run (a local page server plus a local `bin/room`, joined with
`?relay=ws://localhost:PORT`) has no API. So it has no seat tickets, no names,
no player ids, no directory and no blocks. Since `820b53996e`, the pair
check skips the steps that need the API on a local relay instead of failing
them:

| Local failure (2026-11-03) | Real? | Why |
| --- | --- | --- |
| Nameplate names are `-` | No, local only | The room takes a name only from the API's seat ticket and never from the page's hello, so on a local relay everyone is `-`. Now skipped on `?relay=`. Production still checks it. |
| INVITE link doesn't seat C | No, harness only | The harness built C's link without `?relay=`, so C looked for the API and never joined. The link now keeps `?relay=`, and C sits in A's instance locally. |
| BLOCK doesn't hide A | No, local only | A nameplate opens the pane on a player id, and only a seat ticket gives one. Now skipped on `?relay=`. |
| Reverse Gravity has no cylinder and doesn't lift A | No | A fresh build of `main` passes it locally on desktop (A lifted to 317 cm) and on phone (462 cm), and production passed it on `978ef4f4c1`. The failing run served page builds it didn't rebuild (`--build true`) and added its own delay in the harness script. |
| Move latency misses the later trials | Yes, harness bug | The published world puts a crate about 1.5 m ahead of the spawn. Since `29a442405b` the room server lays it too, so walking into it stops the player on both sides, and later trials moved nobody. On production this starts the moment step 2 ships. Both pages now step right off the spawn before the trials. |

So after step 2, use the pair check from `main` (the harness runs from this
checkout, not from the release). An older harness reports false move-latency
misses.

### Rollback for step 3

```sh
coax/web/rollback.sh            # lists the kept releases
coax/web/rollback.sh <time>     # puts back the pages kept at <time>
```

Use the release `release.sh` printed as the one before this release. The
pages name only content-named files, so restoring the pages is the whole
rollback.

## Step 4: capacity, 3 rooms a shard (#12717)

### Choose: 3 rooms a shard, not 8 shards

| Option | Players | Added cost a month | Work |
| --- | --- | --- | --- |
| Today: 6 shards, 2 rooms each | 192 | $0 | none |
| **6 shards, 3 rooms each** | **288** | **$0** | one row change |
| 8 shards, 2 rooms each | 256 | up to about $100 | `deploy.sh shards=8`, two `room_shards` rows, load.coax's 6-shard limit, two new backends |

Cost basis: each shard is Cloud Run with 1 vCPU and 512 MiB,
`--no-cpu-throttling` and min 0 instances. It bills only while players are on
it, at about $0.068 an hour, or about $50 a month if it never sleeps.
Placement fills the lowest shard first. Three rooms a shard runs the same
services longer and uses fewer of them, so it never costs more than two rooms
a shard. Eight shards would add at most about $100 a month and lower
capacity, so they don't fit under the $1,500 GCP cap as well. Choose 3 rooms
a shard.

Headroom:
- After tonight's fixes, one local process ran 16 full rooms (256 players)
  with ACK p95 under 40 ms and 10.9 s of tick work a minute.
- On production, 2 full rooms tick at p95 3.9 ms on a shard (`f0d97831c8`).
- Memory is about 10 MB a room locally, against 512 MiB.

### Gate: one shard with three full rooms on production

Run this after step 2. Every room it opens goes to shard 1, on the old path
with tickets the tool signs itself:

```sh
coax/web/tool.sh coax/web/room/load.coax everglade wss://openagents.com 48 60
curl -s https://openagents.com/api/room/s1/health
```

Apply the change only if all of these hold:
- The load prints `holds`.
- Shard 1's everglade tick p95 is under 5 ms while the load runs.
- Memory stays under 400 MiB. The `load` step of `everglade-proof.sh` reads
  it from Cloud Monitoring.

If p95 is between 5 and 15 ms, the change still fits the 33 ms frame. Apply it
only with the owner's yes, since spec acceptance 7 sets 5 ms.

### Apply

Placement reads `verse.room_shards.rooms_cap` on every pick, so the change
takes effect without a room server deploy. Ship it as a migration, the way
0117 set it to 2. Add
`coax/web/api/migrations/0119_room_shard_cap_three.sql`:

```sql
-- Everglade instances on oa-room shards (#12717): three instances a shard. After the shared-seat
-- copy and the stall's owed ticks (6a846432d2), a shard ticks three full rooms well under the
-- 33 ms frame (gate: docs/verse/2026-11-04-everglade-launch-runbook.md, step 4). Six shards then
-- hold 18 instances (288 players). Placement reads rooms_cap each pick. Safe to run twice.
UPDATE verse.room_shards SET rooms_cap = 3 WHERE shard BETWEEN 1 AND 6 AND rooms_cap = 2;
```

In the same commit, change the comments that say "2 instances each" or "two
rooms a shard" in `coax/web/room/deploy.sh`, `coax/web/room/load.coax` and
`coax/web/room/everglade-proof.sh` to three. Then:

```sh
git add coax/web/api/migrations/0119_room_shard_cap_three.sql coax/web/room/deploy.sh \
  coax/web/room/load.coax coax/web/room/everglade-proof.sh
git commit -m "Everglade: three instances a shard (#12717)"
git pull -q --rebase origin main && git push -q origin HEAD:main
coax/web/api/deploy.sh
```

### Checks after step 4

1. **Fan-out across a shard's third room:**

   ```sh
   coax/web/tool.sh coax/web/room/fanout-check.coax https://openagents.com 40 10
   ```

   It passes when 40 guests fill instances 1, 2 and 3 at 16, 16 and 8, and
   the rooms list puts all three on shard 1.
2. **Load:** run the `everglade-proof.sh load` command from step 2 again. Its
   own packing still puts two rooms on a shard, so it checks that nothing
   regressed.

### Rollback for step 4

Add `0120_room_shard_cap_two.sql`, then deploy the API:

```sql
UPDATE verse.room_shards SET rooms_cap = 2 WHERE shard BETWEEN 1 AND 6 AND rooms_cap = 3;
```

Instances already open on a third slot stay open until they empty. New ones
go to the next shard with room.

## Owner-only steps

These are in `NEEDS_OWNER.md` at the workspace root. Only the first two
matter for launch.

1. **Admin live-edit check (blocks only the pair check's step 9).** No agent
   identity is an Everglade admin. Do one of these, which takes about 2
   minutes:
   - Make the production test account an admin of the realm.
   - Or log in on openagents.com as an admin and save the `oa_session` cookie
     to `~/work/.secrets/everglade-admin-session.env` as
     `OA_EVERGLADE_ADMIN_SESSION=...`. Never paste it into chat.

   Then tell an agent that the smoke may publish one test edit to Everglade.
   The agent runs this from the openagents root:

   ```sh
   OA_EVERGLADE_ADMIN_SESSION=... coax/web/tool.sh everglade-pair --allow-public-edit
   ```

   It passes when the second page hears `e edit` and shows the placed thing
   within 2 s. The check doesn't undo its edit, so afterwards press
   **ORIGINAL** twice in **EDIT**, or ask the agent to remove the test thing.
2. **Events START NOW and STOP (needs an admin, after step 3).**
   1. On a computer signed in as an Everglade admin, open
      **openagents.com/worlds/everglade**.
   2. On a second device signed out (a guest), open the same page.
   3. On the admin page, open **EVENTS** (an admin's button in the top bar) and press
      **START NOW**. It passes when the admin page says THE RAID STARTS IN
      EVERY ROOM WITHIN 2 S, and both pages show the event banner within
      2 s.
   4. Press **STOP**. It passes when the admin page says THE RAID STOPS IN
      EVERY ROOM WITHIN 2 S, and the banner leaves both pages within 2 s.

   Leave the switches as they were before the test. The room logic has unit
   tests; this is the only production check of the admin path.
3. Not blocking the launch, still open for Everglade:
   - Approve the painted look at dawn and dusk, and check the pond woods on
     your phone.
   - Get two Quaternius packs for the townspeople and cats.
   - Check the Everglade link preview in iMessage and Slack.
   - Check the phone tier's look and speed (#10908).
   - Play Everglade natively on the 4080's screen.

## Known gaps at launch

- Smoothness while walking (#12731) belongs to the netcode work. The pair
  check reports it and doesn't judge it.
- REPORT and KICK in the moderation pane have no automated production check
  (#12714). REPORT files a real queue entry, and KICK needs an admin.
- Not yet run on the server: the 60 s regrow, the admin's realm edits, the
  rain's rise on the river, Water Breathing and the boats (spec section 13,
  item 5).
