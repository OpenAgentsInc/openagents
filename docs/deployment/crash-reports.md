# Crash reports

A crash in a room or a page produces a report with a stack of Coax functions.
Reports with the same stack land in one bucket, and a new bucket raises an
alert through the service alerts ([service levels](service-levels.md)). No
outside crash service is used (#13298).

## Read the crashes

```sh
curl -s https://openagents.com/api/playtest/crashes          # production
curl -s https://staging.openagents.com/api/playtest/crashes  # staging
```

The answer lists the 50 buckets heard most recently in 30 days. Each has its
signature, its count, and its newest report in full.

A report is lines of `KEY VALUE` (crash report v1,
`proto/verse/crash-report-v1.json`):

```
crash 1
t 1762990000000
source room
shard 1
release 1afd85d5a2-20261112180000
build 5f0c2a9e7b1d4c3a8e6f00112233445566778899
kind SIGABRT
at 0
trap coax/web/room/room.coax:412: index out of bounds
f coax_trap room.c:328
f coax_index room.c:372
f rm_crash_test coax/web/room/room-stats.coax:168
f main room-host.c:1047
f _start +0xeab0
tick 1762989999900 420 2
sig SIGABRT|rm_crash_test|main
```

- `trap` is the line a Coax trap prints before it stops.
- Each `f` line is a frame, innermost first: the function, then its file and
  line. Functions the compiler inlined have their own lines. A Coax file
  shows its path in the repository; the C host and Coax's generated C show
  only the file's name.
- `tick` lines are the last 16 ticks before the crash: the time, how long the
  tick took in microseconds, and the player count.
- `sig` is the kind and the three innermost Coax functions. The bucket is the
  md5 of the source and this line.

## Rooms

1. `room-crash.c` installs handlers for SIGSEGV, SIGBUS, SIGFPE, SIGILL,
   SIGABRT, SIGTRAP and SIGSYS. A handler writes the dump to
   `/var/lib/oa-room/UNIT/crash.txt` with async-signal-safe calls only, prints
   the frames to the log, and lets the process die of the signal.
2. systemd starts the room again within a second.
3. The new process reads the dump and runs `addr2line -a -f -i` on the binary
   that crashed. `room-crash.coax` builds the report, and the room posts it to
   `POST /api/playtest/room-crash`, signed as its `crash` write
   ([room writes](../protocol/room-write-v1.md)). The dump moves to
   `crash.last.txt`, so it is sent once.

Room binaries are built with `-g1` and Coax's `#line` directives
(`stage.sh`, `compile.sh`), so each address resolves to a Coax file and line.
The code is the same bytes as without `-g1`.

Cloud Run's rooms have no state directory. There a handler only prints the
frames to the log.

## Pages

1. `wasm.sh` runs `wasm-opt -O2 -g`, then moves the function names out of the
   module into `NAME.wasm.syms` (`coax/web/tool.sh wasm-size symbols`). The
   module that ships has the same bytes as before, so no size budget changes.
2. `deploy.sh` ships the names beside the module, as
   `build/NAME.HASH.wasm.syms`.
3. After a trap, and only then, `host.js` fetches the names and sends the
   `crash` analytics event with the module's file (`rel`), the Coax trap's
   line (`trap`) and the eight innermost frames' names (`f0` to `f7`).
4. The service watch folds each such event into a report every minute.

A page gives function names, and the trap's line gives the file and line of
the trap. Frames above it have no line numbers.

## Alerts

`coax/web/api/service-policy.coax` decides them:

| Alert | Fires when | Runbook |
| --- | --- | --- |
| `crash:BUCKET` | A room's bucket got a report in the last hour. One alert for each bucket | Read the report at `/api/playtest/crashes`. If a release did it, roll back with `coax/web/room/vm-deploy.sh rollback` |
| `crash:page` | A page crash that no automated browser sent arrived in the last hour. One alert for all pages, because anyone can post a page's event | Read the report at `/api/playtest/crashes` |

They follow the same path as every service alert: production mails the owner,
and staging reaches only the log, `/status` and the agents' inbox.

## Symbols for each release

| What | Where | Kept |
| --- | --- | --- |
| A room binary with its line table | `/opt/oa-room/releases/NAME/room` on the room VM | The five newest releases |
| A room binary's debug file | `gs://openagentsgemini-oa-symbols/room/BUILDID.debug.gz` | 180 days |
| A page module's function names | `build/NAME.HASH.wasm.syms` on the site | While that release is live |
| The same names | `gs://openagentsgemini-oa-symbols/wasm/NAME.HASH.wasm.syms.gz` | 180 days |

`vm-deploy.sh deploy --staging` uploads the room's debug file, and
`deploy.sh` uploads each release's page names. Neither can stop a deploy.

Resolve an address after its release has left the VM or the site:

```sh
# A room frame "f ? +0x4b310" from a report whose build line is BUILD
coax/web/room/vm-deploy.sh resolve BUILD 4b310

# A page frame "#123" from a report whose release line is everglade2.HASH.wasm
coax/web/symbols.sh page everglade2.HASH.wasm 123
```

## Test the path

```sh
coax/web/room/vm-deploy.sh crash staging   # a Coax trap on purpose in the staging room
```

In a staging page, run `coaxial.trapNextFrame()` in the console. Production
pages don't have it.

Both show up at `https://staging.openagents.com/api/playtest/crashes` within
about a minute. Staging never mails the owner.

## Resources and cost

| Resource | Name | Cost |
| --- | --- | --- |
| Cloud Storage bucket | `openagentsgemini-oa-symbols`, us-central1, Standard, private, objects deleted after 180 days (`coax/web/symbols.sh bucket`) | $0.02 a GB a month; see the measured size below |
| Disk on the room VM | `/var/lib/oa-room/UNIT/crash.txt` and `crash.last.txt`, a few KB a unit | none |
| Database | `verse.crash_buckets` and `verse.crash_reports` (the newest 20 reports a bucket, buckets dropped after 90 days) | none |

No new service, job or schedule: the existing `service-watch` jobs do the
work.
