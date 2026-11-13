# One release: the API, the rooms and the page

This page describes the release set: one command that moves the API, the rooms
and the page in order, under one version, with a canary before the rooms and
one command to move all three back (#13299). The rules live in
`coax/web/release-set-policy.coax`. The command is `coax/web/release-set.coax`.

## Ship

```sh
coax/web/tool.sh release-set ship --staging   # staging's API, room and pages
coax/web/tool.sh release-set ship             # production (run by the owner, or with the owner's word)
```

It ships the checkout's HEAD. The commit must be committed, and for production
it must be on `origin/main`. The parts move in this order, and a part moves
only after the ones before it:

| Step | What runs | It stops when |
| --- | --- | --- |
| The API | `coax/web/api/deploy.sh` (staging, then production). Skipped when the API already runs the commit | The deploy fails, or the API's health names another build |
| The rooms, on staging | The protocol bots play staging's shard as it is (not when that room speaks a wire version behind the bots': it would refuse them all). `coax/web/room/vm-deploy.sh deploy --staging` puts the new room there. The bots play it again | The canary fails (below). Staging goes back to its room |
| The rooms, in production | `vm-deploy.sh canary K --release NAME` puts the release the set staged on one shard. Its windows are read beside the other shards' for the canary's time. Then `vm-deploy.sh deploy --release NAME` moves the rest | The canary fails, or staging no longer runs `NAME` (below). The canary shard goes back; the other shards never moved |
| The page | `coax/web/release.sh --ref COMMIT` | A live shard's pins (engine, rules, world) are not the page's, or the shard does not speak the page's wire version. A page ahead of its rooms strands every player (#13163, #13387) |

The set's version is the commit and the time, for example
`1afd85d5a2.1794518372`. The record of a set (its version, what each part ran
before and after, which parts moved, where it stopped) is written under the
pipeline's folder: `~/.openagents/pipeline/release-sets/ENV/VERSION.txt`.

### A promotion pins its build, and a release holds staging

Two rules keep a release from promoting a build it never tested (#13398). On
2026-11-12 another deploy to staging landed between a release's canary and its
promotion, and all six production shards took that build (#13397).

**A promotion names its release.** `vm-deploy.sh deploy` and `vm-deploy.sh
canary K` take `--release NAME`, the release `deploy --staging` printed. The
release set passes the name it staged. A promotion is refused, and nothing in
production changes, when:

| Case | It says |
| --- | --- |
| No release is named | `no release is named: a promotion names the release it staged (--release NAME)` |
| Staging's unit runs another release by then | `staging does not run the release this promotion staged: someone deployed to staging since`, with both names |
| The release is a seeded bad build | `a seeded bad build: production never takes one` |
| The live page's wire version is outside the range the release speaks | `NAME speaks wire A to B, and the page production serves speaks wire N` |
| The release's wire range can't be read from staging's room | `could not be read` |

The last two hold for every promotion, inside a release set or not. Rooms
always speak the version before their own
([the previous wire](../protocol/everglade-previous-wire.md)), so a release
one wire version ahead of the page passes. A release two versions ahead does
not: ship the version between first. When the live page's version can't be
read, that is not held against the promotion.

The engine, rules and content pins have the same rule: a room seats the build
of the release before its own for one release
([the previous pins](../protocol/everglade-previous-pins.md), #13408). The
promotion does not read those pins. The landing check does
(`coax/remote/check.sh e2 PROD`, line `e2 prev-pins`): a commit whose room
would not seat production's page goes red on `main` before any release.

**A release holds staging's lock.** `release-set ship` takes the lock before
its first step and releases it after its last, whether it ships or stops. The
lock is one line in `/opt/oa-room/staging.lock` on the room VM: a token, when
it ends, the computer and process that took it, and who holds it. While it is
held, these refuse anyone who does not pass the holder's token, and name the
holder:

- `vm-deploy.sh deploy --staging` and `vm-deploy.sh rollback --staging`
- `coax/web/api/deploy.sh` (every API deploy goes to staging first)

```sh
coax/web/room/vm-deploy.sh lock status   # who holds staging, until when, and whether the lock is stale
coax/web/room/vm-deploy.sh lock clear    # remove a stale lock; a live one is refused
coax/web/room/vm-deploy.sh lock take "why" 3600   # hold staging yourself: prints "lock-token TOKEN"
coax/web/room/vm-deploy.sh lock release TOKEN
```

A lock is stale when its time has passed (four hours unless the taker says
otherwise), or when it was taken on this computer and its process is gone. A
stale lock never stops a deploy: the next deploy to staging removes it and
says whose it was. To deploy under your own lock, pass `--lock TOKEN` or set
`OA_STAGING_LOCK`.

The rules are `rs_promote`, `rs_lock` and `rs_rooms_beside` in
`coax/web/release-set-policy.coax`. `coax/web/room/test-vm-deploy.sh` runs
`vm-deploy.sh` and `api/deploy.sh` against a stand-in for the VM and checks
each refusal.

### A page never ships ahead of its rooms' wire

A room speaks a range of wire versions and reports it (`wire=A-B` on the
`build` line of its vitals). A room refuses a page whose version is outside
that range. The page then tells the player to reload, and the reload gets the
same page and the same refusal, so nobody can play until the rooms move.

So a change that raises `WIRE_VERSION` ships only as a release set, the rooms
before the page. Two checks hold that rule (#13387):

- `release-set ship` releases the page only when every live shard speaks the
  page's wire version (and runs its pins).
- `coax/web/release.sh`, the page's release by itself, asks the same question
  before it puts anything on staging or production. It reads staging's shard
  for `--staging-only` and production's shards otherwise. When a shard does
  not speak the page's version, the release stops and names each shard with
  both versions.

```sh
coax/web/tool.sh release-set page-wire             # this checkout's page against production's rooms
coax/web/tool.sh release-set page-wire --staging   # against staging's room
```

The command exits 0 when the page may ship and 1 when it may not. It changes
nothing.

| Case | Result |
| --- | --- |
| Every live shard read speaks the page's version | The page ships |
| A live shard read does not speak it | The release stops |
| No shard can be read, and the page speaks the version production's page speaks | The page ships: the release changes nothing a room could refuse |
| No shard can be read, and the page speaks another version | The release stops |

A release whose wire version did not change is never stopped by this check.
`release.sh` asks the rooms while the build starts, so the check adds no wait.
The rule is `rs_wire_speaks` and `rs_wire_ship` in
`coax/web/release-set-policy.coax`.

### The canary

A canary is compared with the others: in production, the shards still on the
old release, over the same minutes; on staging, which has one shard, the same
shard just before, under the same bots. The canary fails when it:

| Reason | Rule |
| --- | --- |
| `crashed` | A crash bucket names the new release |
| `unhealthy` | None of its windows could be read |
| `slower-tick` | Its tick's p50 or p95 (the middle one of its windows) is over twice the others' and 300 µs more, or it runs under nine in ten of the ticks a second the others run |
| `more-refusals` | It refuses over twice the frames a second the others do, and 1 a second more |
| `fewer-seats` | It seated under nine in ten of what the others seated (bots) |
| `slower-saves` | Its oldest unsaved checkpoint is past 15 s and the others' is not |
| `refuses-the-build` | Its bots' hellos were refused for their version |

Each reason is a regression the release causes. What the other shards do too
never stops a release.

The canary shard is the emptiest one unless `--canary K` names it. All six
production shards run on one VM today, so a canary catches a bad room build,
not a bad host. Rooms on two VMs are #13295.

### The release's own check

`release-set ship --staging --seed-tick-us 20000` ships a seeded bad room: a
build whose every tick is 20 ms longer (`vm-deploy.sh deploy --staging
--seed-tick-us`). The canary must stop it and staging must go back to the room
it ran. A seeded build's release is named `-seeded`, and `vm-deploy.sh` never
puts one on a production shard.

## Roll back

```sh
coax/web/tool.sh release-set rollback --staging
coax/web/tool.sh release-set rollback            # production
```

It moves back what the newest set moved, the page first and the API last. A
part the set skipped or never reached stays where it is:

| Part | How |
| --- | --- |
| The page | Production: the pages kept by the release (`coax/web/rollback.sh STAMP`). Staging keeps no pages, so the commit the API ran before is released there again |
| The rooms | `vm-deploy.sh rollback` (staging: `rollback --staging`): the rollback target, each shard drained first |
| The API | The image the revision before ran is deployed again as a new revision, so the service's traffic stays on its latest and the next deploy takes it as every deploy does |

Migrations stay applied. An API release must keep its schema changes readable
by the revision before it.

### The rooms' rollback target

`/opt/oa-room/previous` is where `vm-deploy.sh rollback` goes. A promotion
makes the release it replaces the target, and says so. Three cases keep the
target right (#13398):

| Case | What happens |
| --- | --- |
| The release production runs was a mistake, and you promote its repair | Run `vm-deploy.sh deploy --release NAME --repair`. The target stays where it was, so a rollback never goes to the mistake |
| A rollback | The release it leaves is marked bad (`/opt/oa-room/bad`). No later promotion makes a bad release the target, and no rollback goes to one |
| You need another target | `vm-deploy.sh previous NAME` sets it. It refuses a release that is bad, seeded, missing or running. `vm-deploy.sh bad NAME` marks one bad by hand |

`vm-deploy.sh releases` lists every link and the bad releases.

## Status

`release-set status` prints the three parts' builds and the shards' pins, and
says whether they are one set. They are one set when the rooms and the page
run the same commit, and the API runs that commit or a later one that
contains it. An API release keeps what earlier commits need, so a newer API
beside the set's rooms and page is still one set, and the line says so. Any
other mix reads `not one set`, and the command exits 1.

## Flags and maintenance

A flag is a name and a whole number that the API holds for itself and the
rooms. An admin sets one with no deploy. Every room reads the whole list on
its control poll, two seconds later (`coax/web/room/room-control.coax`,
`rm_flag`). A flag can carry the time it ends and up to 160 characters of why.

```sh
# as an admin (coax/web/api/admin.sh's key signs the request)
POST /api/admin/flags   maintenance 1 0 Moving the rooms to a new release
POST /api/admin/flags   closed-s3 1
POST /api/admin/flags   clear maintenance
GET  /api/admin/flags
```

| Flag | Read by | Effect |
| --- | --- | --- |
| `maintenance` | The API | While it is not 0, nobody new is seated. The ticket's answer says why, and the page shows `MAINTENANCE · WHY` over its bar and asks again every 30 s. Players already seated play on |
| `closed` | Every room | The shard's directory report says it drains, so the directory places nobody new on it. Seated players play on |
| `closed-sK` | Shard K | The same, for one shard: a shard out of rotation without a drain or a restart |

What a name, a value and the words can be is in
`coax/web/api/service-policy.coax`.
