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
| The rooms, on staging | The protocol bots play staging's shard as it is. `coax/web/room/vm-deploy.sh deploy --staging` puts the new room there. The bots play it again | The canary fails (below). Staging goes back to its room |
| The rooms, in production | `vm-deploy.sh canary K` puts the new room on one shard. Its windows are read beside the other shards' for the canary's time. Then `vm-deploy.sh deploy` moves the rest | The canary fails. The canary shard goes back; the other shards never moved |
| The page | `coax/web/release.sh --ref COMMIT` | A live shard's pins (engine, rules, world) are not the page's, or the shard does not speak the page's wire version. A page ahead of its rooms strands every player (#13163, #13387) |

The set's version is the commit and the time, for example
`1afd85d5a2.1794518372`. The record of a set (its version, what each part ran
before and after, which parts moved, where it stopped) is written under the
pipeline's folder: `~/.openagents/pipeline/release-sets/ENV/VERSION.txt`.

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
| The rooms | `vm-deploy.sh rollback` (staging: `rollback --staging`): the release before, each shard drained first |
| The API | The image the revision before ran is deployed again as a new revision, so the service's traffic stays on its latest and the next deploy takes it as every deploy does |

Migrations stay applied. An API release must keep its schema changes readable
by the revision before it.

`release-set status` prints the three parts' builds and the shards' pins, and
says whether they are one set.

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
