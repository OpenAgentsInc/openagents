# 2026-07-19 Codex Desktop Git Review OOM Incident After-Action

## Document control

| Field | Value |
| --- | --- |
| Status | Final forensic report |
| Incident date | 2026-07-19 |
| Report date | 2026-07-19 |
| Incident class | Local desktop application crash |
| Impact level | High local impact |
| Recurrence | Confirmed |
| Primary incident | `A9CDA8DE-CFD8-4D25-AA4A-CAEC90166CB1` |
| Affected product | Codex Desktop |
| Affected build | `26.715.31925 (5551)` |
| Affected subsystem | Git review-summary worker |
| Direct cause | Node and V8 fatal out-of-memory abort |
| Trigger | A large review-summary Git scan of the umbrella work repository |
| Report scope | Evidence, cause, impact, containment, and corrective actions |
| Change scope | Documentation only |

## Executive summary

Codex Desktop crashed because its internal Git worker reached a fatal Node and
V8 memory limit. The fatal handler called `abort()`. macOS then reported
`EXC_CRASH (SIGABRT)` and `Abort trap: 6`.

The crash did not start on the main user-interface thread. Thread 54 had the
name `git`. Its stack contains `node::OOMErrorHandler` directly above
`abort()`. The lower stack also contains the Node worker loop and message port.

The app log gives the probable workload that caused the memory failure. A
review-summary worker scanned the umbrella `~/work` Git repository. The worker
used `git diff --no-index` and `git hash-object` for untracked files.

The affected app session produced 3,120 review-summary Git log records. The
records contain 1,764 diff operations and 1,303 hash operations. The worker
aborted 871 records after start and 2,249 records before start.

The records also show 139 operations with a duration of at least 80 seconds.
All 139 long operations used the umbrella repository. The maximum duration was
90,534 milliseconds.

The log field `outputLimitMaxBytes` had the value `null` in all 3,120 applicable
records. This field does not prove an unlimited OS pipe. It does
show that the log did not identify a per-command byte limit.

A post-crash repository snapshot found 121,277 untracked files and 121,713
total Git status entries. Two untracked trees contained 120,855 files. Those
two trees represented 99.652 percent of all untracked files.

The incident is recurrent. The local crash archive contains three crashes with
the same fatal signature across two Codex Desktop builds. Each crash used
thread 54, the name `git`, `SIGABRT`, and `node::OOMErrorHandler`.

The immediate technical cause has high confidence. The workload trigger also
has high confidence. The exact final allocation has medium confidence because
the crash report has no heap snapshot or peak resident-memory value.

## Final conclusion

The app terminated itself after a fatal memory allocation failure in its
internal Git review worker. The oversized and dirty umbrella repository drove
an unbounded or insufficiently bounded review workload.

The product failure is larger than the dirty repository. A desktop review
feature must not terminate the whole app for any valid repository state. The
worker needs hard work limits, queue control, a circuit breaker, and crash
isolation.

## Source data notice

This report treats code, commands, paths, timestamps, identifiers, hashes, and
crash fields as source data. The report does not change those values.

The report changes private absolute paths to `~/work` or `~`. This change keeps
the technical path relation and removes the local account name.

The report does not include raw prompts, raw model events, raw shell output, or
file contents from private work trees. It includes only public-safe command
metadata and aggregate measurements.

The local evidence files remain outside Git. The evidence inventory gives their
SHA-256 hashes so an authorized investigator can identify the same files.

## Question under review

This report answers these questions:

1. What terminated Codex Desktop?
2. Which thread caused the termination?
3. Which workload preceded the termination?
4. Why did that workload become large?
5. Did the same failure occur before?
6. Which facts remain unknown?
7. Which controls can prevent another crash?

## Scope

### In scope

- The macOS crash report for the 2026-07-19 12:22:56 local crash.
- The Codex Desktop logs for process 67605.
- The Git state of the umbrella repository after the crash.
- Two earlier crash reports with the same OOM signature.
- One different crash report that provides a comparison case.
- Immediate containment actions.
- Product and repository corrective actions.
- A test plan for a permanent product fix.

### Out of scope

- A change to Codex Desktop source code.
- A change to the umbrella repository ignore rules.
- Removal of local files or work trees.
- A destructive reproduction of the crash.
- A heap dump from the terminated process.
- A claim about the exact V8 allocation that failed.
- A claim about total system memory at the crash time.

## Incident classification

| Dimension | Classification | Basis |
| --- | --- | --- |
| Availability | Local application outage | The app process terminated. |
| Data integrity | No confirmed repository loss | The evidence shows read-only Git commands. |
| Confidentiality | No confirmed disclosure | The crash stayed on the local host. |
| Safety | No physical safety impact | The incident affected a desktop software process. |
| Scope | One operator host | The evidence covers one Mac. |
| Duration | Immediate termination | `SIGABRT` ended the process. |
| Recurrence | Recurrent | Three local reports have the same OOM signature. |
| Detection | Reactive | macOS created a report after process termination. |
| Root class | Resource exhaustion | V8 called the Node OOM handler. |

## Affected environment

| Field | Source value |
| --- | --- |
| Process | `ChatGPT [67605]` |
| Executable | `/Applications/ChatGPT.app/Contents/MacOS/ChatGPT` |
| Bundle identifier | `com.openai.codex` |
| Product version | `26.715.31925` |
| Build | `5551` |
| Architecture | `ARM-64 (Native)` |
| Process role | `Background` |
| Parent process | `launchd [1]` |
| Hardware model | `Mac17,6` |
| OS | `macOS 26.4 (25E246)` |
| Release type | `User` |
| System Integrity Protection | `enabled` |
| Codex Framework version | `150.0.7871.124` |
| App launch time | `2026-07-19 08:05:24.9303 -0500` |
| Crash time | `2026-07-19 12:22:56.9429 -0500` |
| App session age | 4 hours, 17 minutes, 32.0126 seconds |
| Host uptime field | `54000 seconds` |
| Incident identifier | `A9CDA8DE-CFD8-4D25-AA4A-CAEC90166CB1` |
| Crash Reporter Key | `66C55955-03C2-232D-9DA2-25C745D3CBED` |
| Sleep/Wake UUID | `05D51AF1-A29C-469D-9C52-C77502A3E073` |

The executable name is `ChatGPT`, but the bundle identifier is
`com.openai.codex`. This report uses the product name Codex Desktop.

## User-visible impact

The app process terminated without a normal close sequence. All app windows
and in-process workers lost their current process state.

The main app log shows activity for two concurrent turns before the crash. The
last main-process log record occurred approximately 6.8 seconds before the
crash.

The evidence does not prove loss of committed repository data. The Git commands
near the crash used diff and hash operations. These commands normally read
repository state.

The evidence also does not prove that every in-flight model event reached local
durable state. The report has no transaction-level close receipt for each
active turn.

## Detection

macOS detected the process abort and wrote an Apple crash report. The report
contains the fatal signal, thread state, stack, binary images, and incident
identifier.

Codex Desktop wrote separate main-process and Git-worker logs. The Git-worker
log ended before the crash and did not contain a final OOM text record.

No source shows a proactive alert for excessive repository size. No source
shows a user notice before the app terminated.

## Time convention

The Apple crash report uses local Central Daylight Time with offset `-0500`.
The Codex Desktop logs use UTC with suffix `Z`.

This report gives local time first in the timeline. It gives UTC in a separate
column when the app log supplies UTC.

## Detailed timeline

| Local time | UTC time | Event | Evidence |
| --- | --- | --- | --- |
| 08:05:24.930 | 13:05:24.930 | Codex Desktop process 67605 starts. | Apple crash report |
| 08:06:58.816 | 13:06:58.816 | First review-summary record appears in the Git-worker log. | Git-worker log |
| 08:06:58.816 | 13:06:58.816 | A root diff record ends after 81,673 ms with `aborted`. | Git-worker log |
| 08:11:37.439 | 13:11:37.439 | A root diff record ends after 89,804 ms. | Git-worker log |
| 08:13:08.734 | 13:13:08.734 | A root diff record ends after 88,056 ms. | Git-worker log |
| 08:14:42.938 | 13:14:42.938 | A root diff record ends after 87,185 ms. | Git-worker log |
| 08:16:15.692 | 13:16:15.692 | A root diff record ends after 89,510 ms. | Git-worker log |
| 08:17:48.350 | 13:17:48.350 | A root diff record ends after 89,514 ms. | Git-worker log |
| 08:19:21.138 | 13:19:21.138 | A root diff record ends after 89,550 ms. | Git-worker log |
| 08:20:53.894 | 13:20:53.894 | A root diff record ends after 89,601 ms. | Git-worker log |
| 08:22:26.611 | 13:22:26.611 | A root diff record ends after 89,561 ms. | Git-worker log |
| 08:23:59.484 | 13:23:59.484 | A root diff record ends after 86,702 ms. | Git-worker log |
| 08:25:31.740 | 13:25:31.740 | A root diff record ends after 89,269 ms. | Git-worker log |
| 08:27:04.731 | 13:27:04.731 | A root diff record ends after 89,338 ms. | Git-worker log |
| 08:30:10.379 | 13:30:10.379 | A root diff record ends after 89,735 ms. | Git-worker log |
| 08:31:42.828 | 13:31:42.828 | A root diff record ends after 89,448 ms. | Git-worker log |
| 08:33:15.666 | 13:33:15.666 | A root diff record ends after 89,534 ms. | Git-worker log |
| 08:34:48.535 | 13:34:48.535 | A root diff record ends after 89,572 ms. | Git-worker log |
| 08:36:21.822 | 13:36:21.822 | A root diff record ends after 89,562 ms. | Git-worker log |
| 11:59:29.827 | 16:59:29.827 | A root diff record ends after 82,571 ms. | Git-worker log |
| 12:01:09.936 | 17:01:09.936 | A root diff record ends after 89,313 ms. | Git-worker log |
| 12:02:42.836 | 17:02:42.836 | A root diff record ends after 89,318 ms. | Git-worker log |
| 12:04:15.633 | 17:04:15.633 | A root diff record ends after 89,308 ms. | Git-worker log |
| 12:05:48.630 | 17:05:48.630 | A root diff record ends after 89,365 ms. | Git-worker log |
| 12:07:21.487 | 17:07:21.487 | A root diff record ends after 89,456 ms. | Git-worker log |
| 12:08:54.351 | 17:08:54.351 | A root diff record ends after 89,487 ms. | Git-worker log |
| 12:10:27.809 | 17:10:27.809 | A root diff record ends after 89,478 ms. | Git-worker log |
| 12:12:02.753 | 17:12:02.753 | A root diff record ends after 88,684 ms. | Git-worker log |
| 12:13:35.555 | 17:13:35.555 | A root diff record ends after 89,120 ms. | Git-worker log |
| 12:16:41.508 | 17:16:41.508 | A root diff record ends after 89,269 ms. | Git-worker log |
| 12:18:14.811 | 17:18:14.811 | A root diff record ends after 89,659 ms. | Git-worker log |
| 12:18:17.561 | 17:18:17.561 | The worker aborts more diff and hash records. | Git-worker log |
| 12:19:47.673 | 17:19:47.673 | A root diff record ends after 89,305 ms. | Git-worker log |
| 12:19:47.674 | 17:19:47.674 | A hash request names an untracked session-fix file. | Git-worker log |
| 12:19:50.520 | 17:19:50.520 | A new root diff group ends after 2,078 ms. | Git-worker log |
| 12:21:20.686 | 17:21:20.686 | A root diff record ends after 89,397 ms. | Git-worker log |
| 12:21:20.686 | 17:21:20.686 | A hash request names an untracked test file. | Git-worker log |
| 12:21:23.483 | 17:21:23.483 | The last Git-worker records abort another group. | Git-worker log |
| 12:22:50.141 | 17:22:50.141 | The main app writes its last log record. | Main app log |
| 12:22:56.943 | 17:22:56.943 | Thread 54 enters the OOM handler and aborts. | Apple crash report |
| 12:22:59.000 | 17:22:59.000 | macOS writes the report header timestamp. | Apple crash report |

The timeline shows two review bursts. The first burst starts soon after app
launch. The second visible burst continues until 93.46 seconds before the
fatal abort.

The log does not show the command that owned the final allocation. The regular
approximately 90-second pattern supports a continued review scan.

## Crash signal analysis

### Signal chain

| Step | Source value | Interpretation |
| --- | --- | --- |
| 1 | `Triggered by Thread: 54 git` | The internal Git worker caused the crash. |
| 2 | `EXC_CRASH (SIGABRT)` | The process received an abort signal. |
| 3 | `Namespace SIGNAL, Code 6` | macOS classified the termination as signal 6. |
| 4 | `Abort trap: 6` | The process did not complete a normal exit. |
| 5 | `abort() called` | The process called the C abort function. |
| 6 | `node::OOMErrorHandler` | Node handled a fatal V8 memory failure. |

This chain is direct evidence of an internal fatal OOM path. It is not an
inference from a generic memory notice.

### Stable top stack

The top frames are source data from the Apple report:

```text
Thread 54 Crashed:: git
0   libsystem_kernel.dylib  __pthread_kill + 8
1   libsystem_pthread.dylib pthread_kill + 296
2   libsystem_c.dylib       abort + 148
3   Codex Framework         node::OOMErrorHandler(...) + 344
4   Codex Framework         v8::ToExternalPointerTag(...) + 468
5   Codex Framework         v8::ToExternalPointerTag(...) + 352
6   Codex Framework         v8::ExternalMemoryAccounter::Update(...) + 9024
```

Frame 3 establishes the fatal memory path. Frame 6 suggests that V8 external
memory records took part in the path. It does not identify the final object.

### Worker-loop frames

The lower stable frames contain this source data:

```text
node::InternalCallbackScope::Close()
node::InternalMakeCallback(...)
node::AsyncWrap::MakeCallback(...)
node::worker::MessagePort::OnMessage(...)
uv__io_poll
uv_run
node::SpinEventLoopInternal(...)
node::worker::Worker::Run()
```

These frames show a Node worker that handled an asynchronous message. The
worker did not run as a separate protected process.

### Main-thread state

Thread 0 had the name `CrBrowserMain`. It waited in the AppKit event loop at
the crash time.

The main-thread stack starts with `mach_msg2_trap` and continues through the
CoreFoundation and AppKit event loops. The main thread did not call `abort()`.

This evidence excludes a direct main-thread user-interface exception as the
cause of this incident.

### Deep symbol caution

The deep stack contains unrelated names such as
`rust_png$cxxbridge1$194$Reader$next_interlaced_row`. It also contains unknown
JIT addresses.

The Codex Framework binary has limited public symbols. Address-based symbol
names can map to a nearby exported symbol instead of the private function.

This report does not use the unrelated deep names as cause evidence. It uses
the stable signal chain, OOM handler, thread name, and app logs.

## Interpretation of the OOM handler

`node::OOMErrorHandler` means that Node received a fatal V8 memory error. V8
uses this path when it cannot satisfy a required allocation or memory limit.

The handler normally cannot recover the isolate safely. It therefore calls a
fatal termination path. In this incident, that path reached `abort()`.

This fact does not mean that macOS exhausted all physical RAM. A V8 isolate can
reach its heap or external-memory limit while the host still has free memory.

The Apple report does not contain a peak resident-set value, a heap snapshot,
or a `vmSummary` field. The report therefore cannot prove total process memory.

## Git-worker log analysis

### Log identity

The affected process produced three desktop log files:

| Log | Size | Lines | Role in this report |
| --- | ---: | ---: | --- |
| `...-67605-t0-...log` | 1,731,236 bytes | 4,445 | Main app events |
| `...-67605-t1-...log` | 1,984,189 bytes | 3,405 | Git-worker events |
| `...-67605-t2-...log` | 0 bytes | 0 | No usable evidence |

The `t1` file contains `[git]` records. The crash report names the fatal thread
`git`. This relation makes the `t1` file the primary workload source.

### Review-summary aggregate

| Metric | Measured value |
| --- | ---: |
| Applicable log records | 3,120 |
| First applicable timestamp | `2026-07-19T13:06:58.816Z` |
| Last applicable timestamp | `2026-07-19T17:21:23.483Z` |
| Maximum duration | 90,534 ms |
| Records at or above 80,000 ms | 139 |
| Records with `outputLimitMaxBytes=null` | 3,120 |
| Records with `failureReason=aborted` | 871 |
| Records with `failureReason=abortedBeforeStart` | 2,249 |

The 3,120 value counts log records, not proven unique user requests. A command
can produce more than one lifecycle record.

### Command mix

| Subcommand | Applicable records | Share |
| --- | ---: | ---: |
| `diff` | 1,764 | 56.54 percent |
| `hash-object` | 1,303 | 41.76 percent |
| `rev-parse` | 51 | 1.63 percent |
| `status` | 2 | 0.06 percent |

The mix shows repeated content comparison and content hash work. Status
discovery alone does not explain the workload.

### Failure mix

| Failure reason | Records | Interpretation |
| --- | ---: | --- |
| `aborted` | 871 | The command started before cancellation. |
| `abortedBeforeStart` | 2,249 | The queue canceled work before command start. |

The high `abortedBeforeStart` count shows a large queue or rapid cancellation
cycle. It also shows that command cancellation did not stop new work creation.

### Repository relation

Of the 3,120 records, 863 records explicitly name the umbrella `~/work`
repository. Another 2,226 records do not include a `cwd` field. Small counts
name other work trees.

All 139 records with duration at or above 80 seconds explicitly name the
umbrella repository. This is the strongest log-to-repository relation.

### Review request identity

The applicable records contain these source-data fields:

```text
requestKind=review-summary
source=review_model
hostId=local
isRemote=false
```

These values show that a local review-summary feature requested the Git work.
They do not show a user command that directly requested 121,277 files.

### Representative command

The repeated diff command has this source-data shape:

```text
git -c diff.mnemonicPrefix=false \
  -c diff.noprefix=false \
  -c core.quotePath=false \
  -c core.hooksPath=/dev/null \
  -c core.fsmonitor= \
  diff --no-ext-diff --no-textconv --color=never \
  --src-prefix=a/ --dst-prefix=b/ \
  --no-index --raw --numstat -z -- /dev/null -
```

For an untracked file, the worker also used a path in place of standard input.
It used `git hash-object` for other files.

### Adjacent path samples

The last scan groups name untracked files under these normalized trees:

- `.khala-pylon-burndown-ramp/.../stdout.jsonl`
- `.khala-real-port-ramp/.../stderr.log`
- `openagents-session-fix/docs/transcripts/141.md`
- `openagents-session-fix/packages/.../multi-origin-view.test.ts`

The sampled files had sizes from 224 bytes to 9,692 bytes. One large file did
not cause the sampled work. The file count and work rate are more important.

## Umbrella repository snapshot

The investigation measured the umbrella repository after the crash. The root
commit was `a3197eacdb4c58997e4a7fd7202d2cf054036db3` on branch `main`.

The remote was `AtlantisPleb/workspace`. This snapshot is not the OpenAgents
monorepo state. The umbrella repository contains many child repositories and
local work trees.

### Git counts

| Measurement | Count |
| --- | ---: |
| Tracked paths | 2,144 |
| Modified tracked paths from `git ls-files -m` | 435 |
| Deleted tracked paths | 419 |
| Untracked paths | 121,277 |
| Total status records | 121,713 |

The modified count can include deleted paths. These values therefore describe
separate Git queries and do not form independent sum terms.

### Untracked path groups

| Top-level tree | Untracked files | Share of all untracked files |
| --- | ---: | ---: |
| `pnpm-khala-8845-baseline` | 113,819 | 93.850 percent |
| `openagents-session-fix` | 7,036 | 5.802 percent |
| `openagents-worktrees` | 154 | 0.127 percent |
| `.khala-continual-learning-ramp` | 50 | 0.041 percent |
| `.khala-longgen-ramp` | 31 | 0.026 percent |
| `.oa-image-attachments` | 29 | 0.024 percent |
| `.khala-real-port-ramp` | 27 | 0.022 percent |
| `.khala-cl-saturation-ramp` | 17 | 0.014 percent |
| `packages` | 10 | 0.008 percent |
| `.pnpm-store` | 5 | 0.004 percent |
| `.khala-pylon-burndown-ramp` | 4 | 0.003 percent |
| Other trees | 95 | 0.078 percent |

The first two trees contain 120,855 files. They contain 99.652 percent of all
untracked files.

### Disk use

| Tree | Measured disk use |
| --- | ---: |
| `pnpm-khala-8845-baseline` | 6,526,280 KiB, or 6.224 GiB |
| `openagents-session-fix` | 195,388 KiB, or 190.809 MiB |
| `tmp` | 144,868 KiB, or 141.473 MiB |

The investigation stopped a slow size query for `openagents-worktrees`. The
file-count result was sufficient for this report.

### Git object store

| `git count-objects -vH` field | Value |
| --- | ---: |
| Loose object count | 5,355 |
| Loose object size | 3.54 GiB |
| Packed objects | 286,255 |
| Pack count | 6 |
| Pack size | 6.29 GiB |
| Packable loose objects | 1,047 |
| Garbage files | 4 |
| Garbage size | 33.39 MiB |

Git also reported four `tmp_obj_*` files in the object store. Interrupted hash
work can leave temporary objects, but this report cannot assign their origin.

The object-store size adds repository cost. The untracked file count has the
stronger direct relation to the review-summary command pattern.

### Ignore-rule check

The investigation tested the four adjacent sample paths with
`git check-ignore`. Git returned `not_ignored` for each sample.

This result explains why the root repository exposed these files to status and
review discovery. It does not prove that every untracked path lacked an ignore
rule for every command form.

## Recurrence analysis

### Same-signature crashes

| Local crash time | Build | PID | Fatal thread | Signal | OOM handler | Session age |
| --- | --- | ---: | --- | --- | --- | --- |
| 2026-07-17 18:35:55 | `26.715.31251 (5538)` | 3746 | `54 git` | `SIGABRT` | Present | 6 h 17 m 16 s |
| 2026-07-19 05:20:03 | `26.715.31925 (5551)` | 17908 | `54 git` | `SIGABRT` | Present | 4 h 34 m 6 s |
| 2026-07-19 12:22:56 | `26.715.31925 (5551)` | 67605 | `54 git` | `SIGABRT` | Present | 4 h 17 m 32 s |

The signature persisted across builds 5538 and 5551. Two failures occurred on
build 5551 within approximately seven hours.

### Prior Git-worker log volume

| Crash | Review-summary log records |
| --- | ---: |
| 2026-07-17 18:35:55 | 4,588 |
| 2026-07-19 05:20:03 | 4,040 |
| 2026-07-19 12:22:56 | 3,120 |

The prior logs also show repeated diff and hash work under `~/work`. The
2026-07-19 05:20 session includes another root diff with a 89,490 ms duration.

The 2026-07-17 session includes untracked attachment paths in review-summary
work. It ends with the same fatal OOM signature.

### Different crash class

The archive also contains a 2026-07-17 12:13:34 crash on build 5488. That crash
used `CrBrowserMain`, thread 0, `EXC_BREAKPOINT`, and `SIGTRAP`.

The different crash does not contain `node::OOMErrorHandler`. This report
excludes it from the Git-worker OOM incident class.

## Causal model

### Direct cause

V8 reported a fatal memory failure to Node. Node called its OOM handler. The
handler called `abort()`, and signal 6 terminated Codex Desktop.

### Workload trigger

The local review-summary feature generated a large set of Git diff and hash
operations. All long operations used the umbrella repository.

### Environmental factor

The umbrella repository exposed 121,277 untracked files. Two generated or
duplicate trees supplied 99.652 percent of those files.

### Product root cause

The Git review subsystem did not keep its work within a safe resource bound for
this repository state. Cancellation created many `abortedBeforeStart` records,
but it did not prevent new scan work.

The worker shared the app process. Its fatal V8 memory path therefore ended the
whole desktop application.

### Control failure

The product did not stop or degrade the review after repeated approximately
90-second diff failures. It also did not warn the user about the excessive
change set before the crash.

## Five-why analysis

### Why 1: Why did the application exit?

The application received `SIGABRT` after its own process called `abort()`.

### Why 2: Why did the process call `abort()`?

The Node fatal OOM handler called the abort path after a V8 memory failure.

### Why 3: Why did the Git worker use excessive memory?

The worker created and handled thousands of review-summary Git records for a
large dirty repository.

### Why 4: Why was the review set large?

The umbrella repository exposed 121,277 untracked files, child work trees, and
generated artifacts to the scan.

### Why 5: Why did a valid repository state terminate the app?

The review subsystem lacked effective resource limits and process isolation for
the observed workload.

## Factors that increased the risk

### F1. The selected repository had umbrella scope

The selected root contained many independent child repositories, work trees,
generated artifacts, package stores, and temporary data.

### F2. Two untracked trees dominated the file count

The largest tree had 113,819 files. The second tree had 7,036 files.

### F3. The worker used per-file diff and hash work

The log mix contains 3,067 diff or hash records. This work scales with file
count and file content.

### F4. The queue showed heavy cancellation

The worker canceled 2,249 records before start. This count indicates excess
work creation or rapid review invalidation.

### F5. Long operations repeated

The log contains 139 records at or above 80 seconds. Their maximum duration was
90,534 milliseconds.

### F6. The records did not show a byte limit

All applicable records contain `outputLimitMaxBytes=null`. A hard byte budget was
not visible in the diagnostic fields.

### F7. The process had a long active session

The app had operated for more than four hours. A leak or retained review state
could increase risk, but the evidence does not prove a leak.

### F8. The worker failure had process scope

The Node worker lived in the app process. A fatal isolate error therefore had
application-wide impact.

### F9. The feature had no visible circuit breaker

The log shows repeated failures over hours. No source shows a permanent stop
after the failure threshold.

### F10. The app had concurrent work

The main log shows two active turn streams near the crash. Other work can add
memory pressure, but the fatal thread and workload relation point to Git.

## Causes that the evidence does not support

### Not a segmentation fault

The exception was `EXC_CRASH`, not `EXC_BAD_ACCESS`. The top frame was
`__pthread_kill`, not an invalid memory access.

### Not a normal Git exit code

An external Git command can return a nonzero status without process death. This
incident instead used the app's Node OOM handler.

### Not the external Git executable process

The fatal thread had the name `git`, but it existed inside the ChatGPT process.
The crashed process was PID 67605, not a separate Git PID.

### Not an OS memory kill

The termination namespace was `SIGNAL`, and the app process called `abort()`.
The report does not show a jetsam or resource-pressure termination reason.

### Not proof of total host RAM exhaustion

The report has no physical-memory or peak-RSS value. V8 can fail at an internal
limit before the host exhausts all RAM.

### Not proof of hardware failure

The repeated subsystem-specific signature and workload relation support a
software resource failure. No report field identifies a hardware fault.

### Not proof of repository corruption

The object store contains temporary garbage, but `git count-objects` completed.
The evidence does not show an invalid object or failed object read.

### Not proof that PNG decode caused the crash

The deep stack has a `rust_png` symbol, but the adjacent private frames lack
reliable symbols. The Git evidence has a much stronger causal relation.

## Data-integrity assessment

The adjacent Git commands used `diff`, `hash-object`, `rev-parse`, and `status`.
The log configured `core.hooksPath=/dev/null` and disabled `core.fsmonitor`.

These commands do not normally change tracked work-tree files. `hash-object`
can write an object when Git does not use `--no-write`, but it does not change
the index or work-tree content.

The object store contained four temporary object files after the crash. The
report cannot prove that the fatal worker created them.

No evidence shows a partial commit, index lock, changed branch, or changed
remote. No evidence proves that every app-local event reached durable storage.

## Detection gaps

1. The app did not show a pre-scan file-count notice.
2. The app did not show a review queue size.
3. The app did not show a review byte budget.
4. The app did not show worker heap use.
5. The app did not emit a final OOM log record before abort.
6. The app did not provide a heap snapshot.
7. The app did not stop after repeated long diff failures.
8. The app did not isolate the fatal worker from the main process.
9. The crash report did not include peak RSS.
10. The logs did not identify one final allocation or review item.

## Evidence confidence

| Statement | Confidence | Reason |
| --- | --- | --- |
| Node and V8 OOM caused the abort. | High | The stable stack names `node::OOMErrorHandler`. |
| Thread 54 caused the crash. | High | Apple identifies the fatal thread. |
| The fatal thread was the Git worker. | High | Apple names it `git`. |
| The review-summary feature drove heavy Git work. | High | The Git log names `review-summary` and `review_model`. |
| The umbrella repository drove all long Git records. | High | All 139 long records name `~/work`. |
| The large untracked set increased work size. | High | Git counts and file-path samples agree. |
| The final failed allocation belonged to one named file. | Low | No heap snapshot or final command exists. |
| A memory leak existed. | Low | The evidence shows exhaustion, not retention history. |
| Host RAM was fully exhausted. | Low | The report lacks host memory values. |
| The deep `rust_png` symbol names the cause. | Low | Private symbol resolution is unreliable. |

## Limits of this report

- The terminated process cannot provide a new heap snapshot.
- The crash report lacks peak process memory.
- The app source and private symbols were not available.
- The investigation did not reproduce the crash on purpose.
- The post-crash Git snapshot can differ from the exact crash-time state.
- The log record count is not a unique command count.
- The log does not include a final OOM message.
- The report does not identify the exact V8 limit value.
- The report does not prove which JavaScript object consumed the final memory.
- The report does not prove a product memory leak.

## Immediate containment

### C1. Open a child repository as the Codex workspace

Use `~/work/openagents` for OpenAgents tasks. Do not use `~/work` when a child
repository owns the task.

### C2. Reduce the umbrella untracked set

Move generated baselines, duplicate trees, package stores, and temporary data
outside the root Git work tree. Add reviewed ignore rules for retained local
data.

This report does not remove or ignore those paths. The workspace owner must
review path ownership before cleanup.

### C3. Restart Codex Desktop after containment

A restart creates a new V8 process and clears retained process memory. A
restart does not correct the unsafe scan behavior by itself.

### C4. Avoid review-summary work at umbrella scope

Disable or avoid the review-summary feature for the umbrella root if the
installed product provides such a control. This report did not confirm a public
option for that control.

### C5. Keep the crash evidence

Keep the three Apple reports and applicable app logs until the product team
accepts the incident. Keep them outside the public repository.

## Permanent corrective actions

### P0 product actions

| ID | Action | Acceptance condition | Status |
| --- | --- | --- | --- |
| CDX-OOM-001 | Add a hard changed-file count limit. | A 125,000-file fixture cannot start per-file work for all files. | Proposed |
| CDX-OOM-002 | Add a hard total-byte budget. | Review input stops at the byte budget and reports truncation. | Proposed |
| CDX-OOM-003 | Add bounded Git command output. | Every command record has a finite output byte limit. | Proposed |
| CDX-OOM-004 | Add bounded queue depth. | Queue size cannot grow with the full untracked file count. | Proposed |
| CDX-OOM-005 | Add bounded command concurrency. | The worker runs only the configured maximum command count. | Proposed |
| CDX-OOM-006 | Add a failure circuit breaker. | Repeated long aborts stop the scan and show one notice. | Proposed |
| CDX-OOM-007 | Add cancellation backpressure. | Cancellation stops new work creation for the old review revision. | Proposed |
| CDX-OOM-008 | Add worker process isolation. | A worker OOM does not terminate the desktop app. | Proposed |
| CDX-OOM-009 | Add memory telemetry. | Diagnostics include heap use, external memory, queue size, and scan revision. | Proposed |
| CDX-OOM-010 | Add a large-repository notice. | The user can choose skip, tracked-only, or bounded sample. | Proposed |

### P1 product actions

| ID | Action | Acceptance condition | Status |
| --- | --- | --- | --- |
| CDX-OOM-011 | Summarize untracked directories before file expansion. | The review shows tree counts without one command per file. | Proposed |
| CDX-OOM-012 | Prefer one Git status snapshot per revision. | A review revision uses one stable status result. | Proposed |
| CDX-OOM-013 | Cache file hashes by metadata and revision. | Unchanged untracked files do not get a new hash request. | Proposed |
| CDX-OOM-014 | Add scan progress and limits to diagnostics. | A support bundle explains why the scan stopped. | Proposed |
| CDX-OOM-015 | Preserve turn state across worker restart. | A Git-worker restart does not lose active turn state. | Proposed |
| CDX-OOM-016 | Add an OOM support code. | The app shows a stable incident code after worker recovery. | Proposed |

### P0 workspace actions

| ID | Action | Acceptance condition | Status |
| --- | --- | --- | --- |
| WS-OOM-001 | Remove or relocate `pnpm-khala-8845-baseline`. | The root no longer reports its 113,819 files. | Not started |
| WS-OOM-002 | Remove or relocate `openagents-session-fix`. | The root no longer reports its 7,036 files. | Not started |
| WS-OOM-003 | Review root ignore rules. | Generated and local work-tree paths have explicit policy. | Not started |
| WS-OOM-004 | Remove tracked package-cache history from future states. | Temporary package caches never enter the root index. | Not started |
| WS-OOM-005 | Add a root status-size check. | A local check warns before 10,000 untracked files. | Proposed |

The values in the acceptance conditions are proposal values. Product owners
must confirm final limits with measured performance data.

## Required product behavior

This behavior must hold for every valid Git repository state:

1. A review scan must keep memory within a documented bound.
2. A review scan must keep command count within a documented bound.
3. A review scan must keep output bytes within a documented bound.
4. A canceled revision must not create new work.
5. Repeated failures must open a circuit breaker.
6. The user must receive a clear degraded-result notice.
7. A Git worker failure must not terminate the desktop app.
8. Active turns must keep durable state across worker recovery.
9. Diagnostics must identify the limit that stopped the review.
10. The app must not hash every ignored or excluded file.

## Regression test plan

### Test matrix

| Test | Repository fixture | Expected result |
| --- | --- | --- |
| T1 | 1,000 small untracked files | Full bounded summary completes. |
| T2 | 10,000 small untracked files | The configured file limit applies. |
| T3 | 125,000 small untracked files | The app degrades without process death. |
| T4 | One 5 GiB untracked file | The byte limit applies before full materialization. |
| T5 | 500 deleted tracked files | The app reports deletions without per-file queue growth. |
| T6 | Nested child repositories | The root scan respects repository boundaries. |
| T7 | Rapid file changes during review | Old revision work stops after cancellation. |
| T8 | Git command timeout | The circuit breaker stops repeat attempts. |
| T9 | Worker heap fault injection | The main app remains available. |
| T10 | App restart during review | Durable turn state resumes safely. |
| T11 | Binary and image attachments | Size rules apply before content load. |
| T12 | Ignored package store | The review does not enumerate ignored content. |

### Test measurements

Each test must record these values:

- Peak worker resident memory.
- V8 heap used and heap limit.
- V8 external-memory value.
- File count before and after limits.
- Total input bytes before and after limits.
- Git command count.
- Maximum concurrent command count.
- Maximum queue depth.
- Cancellation latency.
- Circuit-breaker state.
- Main-app availability.
- Active-turn recovery result.

### Failure assertions

The tests must fail if any of these events occurs:

- `node::OOMErrorHandler` appears.
- The main process receives `SIGABRT`.
- Queue depth exceeds the configured limit.
- A canceled revision starts a new Git command.
- The worker reads more than the configured byte limit.
- The app omits the degraded-result notice.
- The main app loses a durable active-turn record.

## Operator recovery procedure

Use this procedure after a similar crash:

1. Save the Apple crash report.
2. Note the incident identifier and product build.
3. Save the applicable `t0` and `t1` Codex Desktop logs.
4. Do not publish raw prompts or raw private logs.
5. Check the fatal thread name.
6. Check for `node::OOMErrorHandler` in the Apple report.
7. Count review-summary records in the Git-worker log.
8. Count untracked files in the selected repository.
9. Identify the largest untracked top-level trees.
10. Open the task from its child repository.
11. Restart Codex Desktop after repository containment.
12. Compare the new crash, if any, with prior incident identifiers.

## Evidence collection commands

The commands below are source data. They use normalized local paths.

### Crash signature

```sh
rg -n 'Triggered by Thread|node::OOMErrorHandler|Exception Type|Termination Reason' \
  ~/Library/Logs/DiagnosticReports/ChatGPT-2026-07-19-122259.ips
```

### Git-worker aggregate

```sh
rg -c 'requestKind=review-summary' \
  ~/Library/Logs/com.openai.codex/2026/07/19/*-67605-t1-*.log
```

### Repository counts

```sh
git ls-files | wc -l
git ls-files -m | wc -l
git ls-files -d | wc -l
git ls-files --others --exclude-standard | wc -l
git status --porcelain=v1 --untracked-files=all | wc -l
git count-objects -vH
```

### Untracked group counts

```sh
git ls-files --others --exclude-standard |
  awk '{split($0,a,"/"); c[a[1]]++} END {for(k in c) print c[k],k}' |
  sort -nr
```

### Evidence hashes

```sh
shasum -a 256 <evidence-file>
```

## Evidence inventory

The local file names in this table are source data. The table does not publish
their content.

| Evidence | Bytes | Lines | SHA-256 |
| --- | ---: | ---: | --- |
| Translated current report | 214,103 | 1,262 | `b02221bf8e477891e9552d1145a6c75eae31786d68bb47823489e6e4e9673a79` |
| `ChatGPT-2026-07-19-122259.ips` | 138,280 | 368 | `d383313c61d58d025b9b981972017fde10388166432683cfcecea49ddfdffb10` |
| Current process `t0` log | 1,731,236 | 4,445 | `bdbe516687c34ba24a3a28f9fd38ad50bcee2925ee75bab143b756b33ad7c595` |
| Current process `t1` log | 1,984,189 | 3,405 | `77df67f6d59fdfa474257039241de435d8777edce22f36f1375177ecaa7694a7` |
| `ChatGPT-2026-07-19-052006.ips` | 138,428 | 368 | `ecd306fcab82c9e20780876c6363089c13398cef8f64a6f3c12cbb82b53664df` |
| Prior process 17908 `t1` log | 2,601,730 | 4,053 | `d15b547ef51a9aaf5b96e52f68d6de3b80a607c8c869f5cf1b6786314c5399e4` |
| `ChatGPT-2026-07-17-183558.ips` | 139,572 | 371 | `89a288590f7fdba0c62f98bc67aa1c64000f472e170bf8db4730c01c692c8e9f` |
| Prior process 3746 `t1` log | 4,027,570 | 4,790 | `87d8bf66934e51dddd7fa85bdbdad91084b7c54fb25f8e5440b271fede8f8c8a` |
| Different-class 2026-07-17 report | 137,384 | 361 | `1267aa570ebe672814c758079d74ea32217ddc8836790acdd033c15d35638b2d` |

## Redaction record

The report applies these redactions:

- It replaces the local account name with `~`.
- It omits raw prompts and model event payloads.
- It omits raw file content from untracked trees.
- It omits credentials, tokens, and environment-secret values.
- It omits internal conversation identifiers.
- It keeps incident identifiers because they identify crash evidence.
- It keeps public-safe repository tree names because they explain scan scope.

## Action register

| Action | Owner | Priority | State |
| --- | --- | --- | --- |
| Publish this after-action report. | OpenAgents documentation | P0 | Complete in this change |
| Use child repository scope for OpenAgents tasks. | Local operator workflow | P0 | Recommended |
| Reduce the root untracked set. | Workspace owner | P0 | Not started |
| Add review file and byte limits. | Codex Desktop product | P0 | Proposed |
| Add queue limits and cancellation backpressure. | Codex Desktop product | P0 | Proposed |
| Isolate the Git worker process. | Codex Desktop product | P0 | Proposed |
| Add large-repository regression tests. | Codex Desktop product | P0 | Proposed |
| Add diagnostic memory metrics. | Codex Desktop product | P1 | Proposed |
| Add a user-visible circuit-breaker notice. | Codex Desktop product | P1 | Proposed |
| Keep local evidence until product triage. | Evidence custodian | P1 | Recommended |

## Closure criteria

This incident can close only after these conditions hold:

1. A product test uses at least 125,000 untracked files.
2. The review feature applies finite file, byte, queue, and output limits.
3. Repeated long Git failures open a circuit breaker.
4. A Git-worker OOM does not terminate the desktop app.
5. The app reports a clear degraded review result.
6. The umbrella repository no longer exposes generated bulk trees by default.
7. A new sustained test shows no recurrence across an equivalent session age.

The documentation task can close after this report passes repository checks,
reaches `main`, and matches `origin/main`.

## Final assessment

The evidence establishes a fatal Node and V8 OOM in the internal Git worker.
The evidence also establishes a large, repeated review-summary workload on the
umbrella repository.

The umbrella repository state made the failure much more likely. It did not
make whole-app termination acceptable.

The permanent fix must bound work before content expansion. It must also keep a
worker failure outside the main desktop process.

Until that fix exists, child-repository scope and root-work-tree hygiene provide
the safest immediate containment.
