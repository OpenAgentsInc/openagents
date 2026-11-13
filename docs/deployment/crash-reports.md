# Crash reports

A crash in a room or a page produces a report with a stack of Coax functions.
A crash in the iPhone or Mac app produces a report with a stack of offsets
into the app's binary. Reports with the same stack land in one bucket, and a new bucket raises an
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
   line (`trap`) and the eight innermost frames (`f0` to `f7`). A frame is
   its function's name, then ` +0x` and its code offset when the browser's
   stack gave one.
4. The service watch folds each such event into a report every minute.

A page's report is crash report v2 (`proto/verse/crash-report-v2.json`,
#13379). Its first line is `crash 2`, and each frame carries its code offset:

```
f page_crash_test +0x2b7e1
```

- The offset is the frame's byte offset in the module's file
  (`NAME.HASH.wasm`), as Chrome, Firefox and Safari print it after
  `wasm-function[N]`.
- The signature, and so the bucket, is the names alone. A v1 bucket keeps
  its v2 reports.
- The API reads v1 and v2. Rooms still write v1: their frames already carry a
  file and line.

The trap's own line gives the file and line of the trap. For a frame above
it, resolve the offset against the module (`wasm-objdump -d NAME.HASH.wasm`
shows the instruction at that offset, in the function the frame names). A
line number needs the module's DWARF, which no release keeps yet: the
sidecar is not measured yet (#13379); the report already carries
what it resolves.

## The iPhone and Mac apps

The system keeps a diagnostic of each crash and hands it to the app on a
later launch (MetricKit). The app sends it only after you say yes.

1. `apps/shared/host/apple/CrashReports.swift` listens for the diagnostics
   in both apps. For each crash it writes a dump and keeps it in a folder of
   its own: `Application Support/crash` on the iPhone,
   `~/.openagents/desktop/crash` on the Mac. At most 8 dumps wait.
2. The first dump raises one question, "OpenAgents quit unexpectedly", with
   **Send** and **Don't Send**. The answer is kept in the file `consent`.
   After **Don't Send** the app drops every dump and keeps no more. Delete
   the file to be asked again.
3. After **Send**, the app posts each dump to `POST /api/playtest/app-crash`.
   The route needs no login, because an app can crash before one. It takes
   16 KB at most and 6 posts a minute from one address.
4. `room-crash.coax` (`crash_app_report`) makes the report from the dump, and
   the API keeps it by bucket like a room's.

A dump holds the app's version and build, the system's version, the signal,
the termination reason, and the crashed thread's frames. It holds no account,
no device name and nothing typed or said.

An app's report is crash report v3 (`proto/verse/crash-report-v3.json`):

```
crash 3
t 1762990000000
source app
app ios
os iPhone OS 18.1 (22B83)
release 1.4 (212)
build 70b89f2716343580a69557cdb41d7743
kind SIGSEGV
msg Namespace SIGNAL, Code 11 Segmentation fault: 11
f libswiftCore.dylib +0x1b24
f OpenAgents +0x285b8
f OpenAgents +0x1000
sig SIGSEGV|OpenAgents+0x285b8|OpenAgents+0x1000
```

- `t` is the end of the day MetricKit reported the crash in. MetricKit gives
  no closer time.
- `build` is the UUID of the app's own binary. It names the dSYM that
  resolves the app's frames.
- Each `f` line is a binary and the frame's offset in it. No frame has a
  name: MetricKit gives none.
- `sig` is the kind and the three innermost frames in the app's own binary.
  Offsets change with every build, so one bug in two builds lands in two
  buckets.

Resolve a frame in the app's own binary on a Mac with Xcode:

```sh
coax/web/symbols.sh app-frame 70b89f2716343580a69557cdb41d7743 285b8
```

`apps/openagents-ios/release.sh` keeps each TestFlight build's dSYMs
(`coax/web/symbols.sh app ARCHIVE`). The Mac app's development build has no
dSYM, so its frames stay offsets.

Limits, stated so no one assumes more:

- Not seen on a device. `apps/shared/host/apple/crash-reports-check.sh`
  checks the dump against a diagnostic in MetricKit's documented shape, and
  the API suite checks the route. A real diagnostic needs a TestFlight build
  and a crash on a phone; it arrives up to a day later.
- Whether the system hands diagnostics to the Mac's ad hoc signed
  development build is not known.
- A Mac check that launches the app sets `OA_CRASH_REPORTS_OFF=1`, so the
  question can't stop it. `--shot` runs never start the reporter.
- Android (`ApplicationExitInfo`) is not built.

## Alerts

`coax/web/api/service-policy.coax` decides them:

| Alert | Fires when | Runbook |
| --- | --- | --- |
| `crash:BUCKET` | A room's bucket got a report in the last hour. One alert for each bucket | Read the report at `/api/playtest/crashes`. If a release did it, roll back with `coax/web/room/vm-deploy.sh rollback` |
| `crash:page` | A page crash that no automated browser sent arrived in the last hour. One alert for all pages, because anyone can post a page's event | Read the report at `/api/playtest/crashes` |
| `crash:app` | An app's crash report arrived in the last hour. One alert for both apps, because anyone can post a dump | Read the report at `/api/playtest/crashes`, then resolve its frames with `coax/web/symbols.sh app-frame` |

They follow the same path as every service alert: production mails the owner,
and staging reaches only the log, `/status` and the agents' inbox.

## A GitHub issue for each production bucket

While a `crash:` alert is open on production, the green-main watcher
(`coax/remote/pipeline-lib.sh` `pl_crash_issues`, run each minute by
`pl_service_watch`) files one GitHub issue for each bucket the alert covers
(#13379):

- `crash:BUCKET` covers that room bucket.
- `crash:page` covers every page bucket heard in the last hour.
- `crash:app` covers every app bucket heard in the last hour.

The issue's title is `Production crash (bucket XXXXXXXXXXXX): SIGNATURE`, and its
body is the bucket's newest report from `GET /api/playtest/crashes`.

A bucket never gets two open issues:

1. The watcher looks for an open issue whose title names the bucket. If it
   finds one, it files nothing.
2. If GitHub can't be asked, it files nothing and asks again the next minute.
3. It remembers each bucket it handled in `~/.openagents/pipeline/crash-issues`,
   so GitHub is asked once for each bucket.
4. When a bucket's alert opens again after its issue was closed, the bucket
   gets a new issue.

Staging files no issues. `OA_PIPELINE_ISSUES=0` turns them off.
`coax/remote/test-after.sh` (case 13) checks these rules with stand-ins for the
API and GitHub.

## Symbols for each release

| What | Where | Kept |
| --- | --- | --- |
| A room binary with its line table | `/opt/oa-room/releases/NAME/room` on the room VM | The five newest releases |
| A room binary's debug file | `gs://openagentsgemini-oa-symbols/room/BUILDID.debug.gz` | 180 days |
| A page module's function names | `build/NAME.HASH.wasm.syms` on the site | While that release is live |
| The same names | `gs://openagentsgemini-oa-symbols/wasm/NAME.HASH.wasm.syms.gz` | 180 days |
| An app build's dSYM | `gs://openagentsgemini-oa-symbols/app/UUID.dSYM.zip` | 180 days |

`vm-deploy.sh deploy --staging` uploads the room's debug file, and
`deploy.sh` uploads each release's page names. Neither can stop a deploy.

Resolve an address after its release has left the VM or the site:

```sh
# A room frame "f ? +0x182a16" from a report whose build line is BUILD
coax/web/room/vm-deploy.sh resolve BUILD 182a16
# 0x0000000000182a16
# coax_index
# ././room.c:376
# rm_crash_test
# ./coax/web/room/room-stats.coax:168

# A page frame "#123 +0x2b7e1" from a report whose release line is everglade2.HASH.wasm
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
| Cloud Storage bucket | `openagentsgemini-oa-symbols`, us-central1, Standard, private, objects deleted after 180 days (`coax/web/symbols.sh bucket`) | Under $0.10 a month. Storage is $0.02 a GB a month. A room release adds 564 KB (measured, 2026-11-12) and a full page release about 250 KB, so 30 releases a day for 180 days is about 4 GB |
| Disk on the room VM | `/var/lib/oa-room/UNIT/crash.txt` and `crash.last.txt`, a few KB a unit | none |
| Database | `verse.crash_buckets` and `verse.crash_reports` (the newest 20 reports a bucket, buckets dropped after 90 days) | none |

No new service, job or schedule: the existing `service-watch` jobs do the
work.
