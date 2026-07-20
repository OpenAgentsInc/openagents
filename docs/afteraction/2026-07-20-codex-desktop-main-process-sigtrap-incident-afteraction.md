# 2026-07-20 Codex Desktop Main-Process SIGTRAP Incident After-Action

## Document control

| Field | Value |
| --- | --- |
| Status | Final forensic report |
| Incident date | 2026-07-20 |
| Report date | 2026-07-20 |
| Incident class | Local desktop application crash |
| Impact level | High local impact |
| Recurrence | Confirmed |
| Primary incident | `52D034B0-B3F9-4A4C-88D8-88DCA3A4E27A` |
| Affected product | Codex Desktop |
| Affected build | `26.715.52143 (5591)` |
| Affected subsystem | Electron main-process V8 value deserialization |
| Direct cause | Internal Chromium fatal trap on the main process |
| Immediate workload | Five concurrent task streams and two renderer consumers |
| Additional workload | Long session and recurrent Git review pressure |
| Report scope | Evidence, cause, impact, containment, and corrective actions |
| Change scope | Documentation only |

## Executive summary

Codex Desktop crashed after its Electron main process entered an internal fatal
trap. macOS reported `EXC_BREAKPOINT (SIGTRAP)` and `Trace/BPT trap: 5`.

Thread 0 had the name `CrBrowserMain`. The ARM state identifies `brk 0` at the
program counter. Chromium uses `brk #0` for its immediate-crash path on ARM64.
Thus, the app stopped itself after an internal fatal condition.

The stable stack contains `v8::ValueDeserializer::ReadValue` three times. It
also contains `v8::ExternalMemoryAccounter::Update`. This stack places the
failure in V8 value deserialization and an external-memory count update on the
main process.

The app log shows a high-rate message workload before the crash. In the final
two minutes, the log has 139 records and five distinct task identifiers. It has
106 completed `reasoning-summary` records and 22 `summary-part` records.

The app sent almost the same workload to two renderer consumers. The primary
window received 69 records. A hidden avatar-overlay window received 68 records.
All 137 renderer records in that window show `rendererWindowFocused=false`.

The last app record occurred 1.861 seconds before the crash. It completed a
`reasoning-summary` item for the primary renderer. The report does not include a
JavaScript exception or a named failed assertion.

The session also retained the Git review pattern from the prior incident. Its
Git logs contain 2,487 review-summary records. They include 1,396 diff records,
1,079 hash records, and 131 records of at least 80 seconds.

The Git worker did not directly cause this crash. Its last record ended 4
minutes and 55.382 seconds before the crash. The fatal thread was thread 0,
not thread 54, and the current stack has no `node::OOMErrorHandler`.

The incident is recurrent. The local archive contains six Codex Desktop
crashes in three days. Two use the same main-process SIGTRAP stack. Four use
the Git-worker Node and V8 OOM stack from the prior after-action report.

The direct fatal path has high confidence. The concurrent message workload has
high temporal confidence. Memory retention is a credible causal factor,
but the evidence does not prove a leak or the exact failed condition.

## Final conclusion

Codex Desktop terminated its main process during V8 value deserialization. The
fatal `brk 0` instruction shows an intentional internal crash path, not an
ordinary invalid-memory access.

The crash occurred during sustained transfer of concurrent task events. The app
sent `reasoning-summary` events to the primary window and a hidden overlay
renderer.
The same session also performed a large Git review workload.

The evidence supports a product defect in the main-process message or resource
lifecycle. It does not identify one exact object, assertion, or allocation.

The product must bound message fanout, retained stream state, and background
review work. It must also isolate recoverable workers from the trusted shell.

## Source data notice

This report treats code, commands, paths, timestamps, identifiers, hashes, and
crash fields as source data. The report does not change those values.

The report changes private absolute paths to `~` or a public repository path.
This change keeps the technical relation and removes the local account name.

The report does not include raw prompts, raw model events, raw shell output, or
private task identifiers. It uses public-safe aggregates and normalized fields.

The local evidence files remain outside Git. The evidence inventory gives their
SHA-256 hashes so an authorized investigator can identify the same files.

## Question under review

This report answers these questions:

1. What terminated Codex Desktop?
2. Which thread caused the termination?
3. Which subsystem was active in the fatal stack?
4. Which workload immediately preceded the crash?
5. Did the Git worker cause this crash?
6. Did the same failure occur before?
7. Which facts remain unknown?
8. Which controls can prevent another crash?

## Scope

### In scope

- The macOS report for the 2026-07-20 05:50:58 local crash.
- The translated copy supplied for this review.
- The Codex Desktop logs for process 68077.
- Five earlier Codex Desktop crash reports.
- The prior Git-worker OOM after-action report.
- Immediate containment actions.
- Product corrective actions.
- A regression plan for a permanent product fix.

### Out of scope

- A change to Codex Desktop product source.
- A destructive reproduction of the crash.
- A heap dump from the terminated process.
- A claim about the exact failed V8 object.
- A claim about the exact Chromium assertion.
- A claim about total physical-memory use.
- Publication of raw task or model content.
- A release or product-promise transition.

## Incident classification

| Dimension | Classification | Basis |
| --- | --- | --- |
| Availability | Local application outage | The main app process terminated. |
| Data integrity | No confirmed repository loss | No evidence shows repository corruption. |
| Confidentiality | No confirmed disclosure | The crash stayed on the local host. |
| Safety | No physical safety impact | The incident affected a desktop process. |
| Scope | One operator host | The evidence covers one Mac. |
| Duration | Immediate termination | `SIGTRAP` ended the process. |
| Recurrence | Recurrent | Two reports have this SIGTRAP stack. |
| Detection | Reactive | macOS wrote a report after termination. |
| Root class | Internal fatal condition | Chromium executed an immediate crash. |

## Affected environment

| Field | Source value |
| --- | --- |
| Process | `ChatGPT [68077]` |
| Executable | `/Applications/ChatGPT.app/Contents/MacOS/ChatGPT` |
| Bundle identifier | `com.openai.codex` |
| Product version | `26.715.52143` |
| Build | `5591` |
| Architecture | `ARM-64 (Native)` |
| Process role | `Background` |
| Parent process | `launchd [1]` |
| Hardware model | `Mac17,6` |
| Host memory | `128 GB` |
| OS | `macOS 26.4 (25E246)` |
| Release type | `User` |
| System Integrity Protection | `enabled` |
| Codex Framework version | `150.0.7871.124` |
| App launch time | `2026-07-19 18:36:10.7035 -0500` |
| Crash time | `2026-07-20 05:50:58.0200 -0500` |
| App session age | 11 hours, 14 minutes, 47.3165 seconds |
| Host uptime field | `110000 seconds` |
| Incident identifier | `52D034B0-B3F9-4A4C-88D8-88DCA3A4E27A` |
| Crash Reporter Key | `66C55955-03C2-232D-9DA2-25C745D3CBED` |
| Sleep/Wake UUID | `05D51AF1-A29C-469D-9C52-C77502A3E073` |

The executable name is `ChatGPT`, but the bundle identifier is
`com.openai.codex`. This report uses the product name Codex Desktop.

## User-visible impact

The main process terminated without a normal close sequence. All app windows
and in-process workers lost their current process state.

Five task streams had events in the final two-minute window. The primary window
was visible but not focused. The avatar-overlay window was not visible.

The evidence does not prove loss of committed repository data. File changes
that reached the file system before the crash should remain on disk.

The evidence does not prove that each in-flight task event reached durable app
state. The report has no close receipt for each active task.

The app restarted with a new process after the crash. A restart restores the
process, but it does not correct the product defect.

## Detection

macOS detected the process trap and wrote an Apple crash report. The report
contains the signal, thread state, stack, binary images, and incident identifier.

Codex Desktop wrote separate main-process and Git-worker logs. The main log
ended 1.861 seconds before the crash. It has no final fatal text record.

No source shows a proactive notice for main-process memory, message rate,
renderer fanout, or retained task state.

## Time convention

The Apple report uses local Central Daylight Time with offset `-0500`. The app
logs use UTC with suffix `Z`.

This report gives local time first in the timeline. It gives UTC in a separate
column when the app log supplies UTC.

## Detailed timeline

| Local time | UTC time | Event | Evidence |
| --- | --- | --- | --- |
| 18:36:10.704 | 23:36:10.704 | Codex Desktop process 68077 starts. | Apple report |
| 18:36:11.168 | 23:36:11.168 | The first main-log record appears. | Main app log |
| 18:36:13.472 | 23:36:13.472 | The first Git-worker record appears. | Git-worker log |
| 19:00:31.746 | 00:00:31.746 | A root diff ends after 87,382 ms. | Git-worker log |
| 05:30:00.942 | 10:30:00.942 | A review-summary status request aborts before start. | Git-worker log |
| 05:45:47.066 | 10:45:47.066 | Another status request aborts before start. | Git-worker log |
| 05:46:02.638 | 10:46:02.638 | The Git worker writes its last record. | Git-worker log |
| 05:46:13.596 | 10:46:13.596 | The primary renderer confirms concurrent summaries. | Main app log |
| 05:48:58.086 | 10:48:58.086 | The final two-minute evidence window starts. | Main app log |
| 05:50:22.747 | 10:50:22.747 | The hidden overlay confirms concurrent summaries. | Main app log |
| 05:50:54.842 | 10:50:54.842 | An overlay summary completes. | Main app log |
| 05:50:54.922 | 10:50:54.922 | The same summary completes for the primary window. | Main app log |
| 05:50:56.102 | 10:50:56.102 | The final overlay summary record appears. | Main app log |
| 05:50:56.159 | 10:50:56.159 | The final primary summary record appears. | Main app log |
| 05:50:58.020 | 10:50:58.020 | Thread 0 executes `brk 0`. | Apple report |
| 05:51:01.000 | 10:51:01.000 | macOS writes the report header timestamp. | Apple report |

The main log shows continuous task-summary traffic until 1.861 seconds before
the fatal trap. The Git-worker log ended earlier.

The timeline gives a strong relation between the current trap and message
transfer. It does not identify the exact final message.

## Crash signal analysis

### Signal chain

| Step | Source value | Interpretation |
| --- | --- | --- |
| 1 | `Triggered by Thread: 0 CrBrowserMain` | The Electron main thread caused the crash. |
| 2 | `EXC_BREAKPOINT (SIGTRAP)` | The process entered a trap instruction. |
| 3 | `Namespace SIGNAL, Code 5` | macOS classified the termination as signal 5. |
| 4 | `Trace/BPT trap: 5` | The process did not complete a normal exit. |
| 5 | `esr: ... (Breakpoint) brk 0` | ARM64 executed an explicit breakpoint trap. |
| 6 | `v8::ValueDeserializer::ReadValue` | V8 read a serialized value. |

This chain is direct evidence of an internal fatal path on the main thread.

### Stable top stack

The top frames are source data from the Apple report:

```text
Thread 0 Crashed:: CrBrowserMain
0   Codex Framework  ares_llist_replace_destructor + 2265604
1   Codex Framework  v8::ToExternalPointerTag(...) + 468
2   Codex Framework  v8::ToExternalPointerTag(...) + 352
3   Codex Framework  v8::ExternalMemoryAccounter::Update(...) + 9024
4   Codex Framework  v8_simulator_probe_memory_continuation + 23332
5   Codex Framework  v8::Platform::SystemClockTimeMillis() + 861956
6   Codex Framework  node::StreamBase::ReadStopJS(...) + 65752
7   Codex Framework  v8::ValueDeserializer::ReadValue(...) + 760
8   Codex Framework  v8::ValueDeserializer::ReadValue(...) + 532
9   Codex Framework  v8::ValueDeserializer::ReadValue(...) + 252
```

The repeated deserializer frames establish the active subsystem. The external
memory frame supports a memory relation, but it does not prove OOM.

### Intent of the trap

The ARM thread state gives this source value:

```text
pc: 0x000000011db2f794
esr: 0xf2000000 (Breakpoint) brk 0
```

Chromium defines `brk #0` as its ARM64 immediate-crash instruction. The source
also states that the path can run in allocation hooks without allocation.

This fact means that the product selected a fatal stop. It does not reveal
which `CHECK`, allocation guard, or fatal condition selected the path.

### Main-process state

The lower stack returns through CoreFoundation, HIToolbox, AppKit, and
`ChromeMain`. The failure occurred on the app's browser main process.

This process owns application-wide Electron coordination. A fatal trap there
ends the complete desktop application.

### Git-worker state

Thread 54 still had the name `git`. Its stack waited in `kevent`, `uv_run`, and
the Node worker loop. It did not execute the trap.

The current stack does not contain `node::OOMErrorHandler`, `abort`, or
`pthread_kill`. This incident therefore differs from yesterday's OOM event.

### Deep symbol caution

The first frame resolves to `ares_llist_replace_destructor`. The stack also has
other exported symbol names with large offsets.

The Codex Framework binary has limited public symbols. A private address can
map to the nearest exported name.

This report does not claim that a DNS list destructor caused the crash. It uses
the signal, instruction, thread, repeated V8 frames, and the same prior stack.

## Interpretation of the fatal trap

`SIGTRAP` alone does not mean that a debugger caused the event. Chromium uses
the same signal for its immediate-crash path on macOS ARM64.

The current report does not contain `node::OOMErrorHandler`. Thus, it does not
prove a Node heap OOM in this incident.

The external-memory frame and the recent OOM history keep resource pressure
in the causal model. They do not identify an exact memory limit.

The Apple report has no heap snapshot, peak resident-memory value, or
`vmSummary`. It cannot prove total process memory.

## Main-process log analysis

### Log identity

The affected process produced five desktop log files across the UTC date
boundary:

| Log | Bytes | Lines | Role in this report |
| --- | ---: | ---: | --- |
| 2026-07-19 `t0` | 248,644 | 654 | Early main app events |
| 2026-07-19 `t1` | 135,965 | 304 | Early Git-worker events |
| 2026-07-19 `t2` | 0 | 0 | No usable evidence |
| 2026-07-20 `t0` | 7,076,626 | 19,128 | Main app and renderer events |
| 2026-07-20 `t1` | 1,366,221 | 2,469 | Git-worker events |

The `t0` file is the primary workload source for the current crash. The `t1`
file gives the longer-session Git context.

### Final two-minute aggregate

| Metric | Measured value |
| --- | ---: |
| Main-log records | 139 |
| Distinct task identifiers | 5 |
| Primary-renderer records | 69 |
| Avatar-overlay records | 68 |
| Completed summary records | 106 |
| Summary-part records | 22 |
| ResizeObserver error records | 5 |
| Records with a focused renderer | 0 |
| Records with an unfocused renderer | 137 |

The two non-renderer records were application records. The five task identifiers
show concurrent active streams, not five proven OS threads.

### Renderer fanout

Most summary events appear twice. One record names the visible primary window.
The adjacent record names the hidden avatar-overlay window.

The primary renderer used `rendererWebContentsId=1`. The overlay renderer used
`rendererWebContentsId=14`. The report omits private task identifiers.

The near-equal 69 and 68 counts show that the main process fanned almost all
final-window events to both renderer consumers.

### Concurrent summary control

The log repeatedly contains this source field:

```text
Concurrent reasoning summaries feature override resolved featureOverride=true
```

The primary renderer recorded that value at `2026-07-20T10:46:13.596Z`. The
overlay recorded it at `2026-07-20T10:50:22.747Z`.

This field establishes enabled concurrent summaries. It does not prove that
the feature contains the product defect.

### ResizeObserver errors

The final window contains five errors with this source text:

```text
ResizeObserver loop completed with undelivered notifications.
```

These errors occurred in the primary renderer. They are user-interface errors,
but the fatal stack is in the main-process V8 deserializer.

This report treats the errors as adjacent load evidence. It does not treat them
as the direct cause.

## Git-worker comparison

### Review-summary aggregate

| Metric | Measured value |
| --- | ---: |
| Review-summary records | 2,487 |
| Diff records | 1,396 |
| Hash records | 1,079 |
| Status records | 7 |
| Revision-parse records | 5 |
| Records with `failureReason=aborted` | 678 |
| Records with `failureReason=abortedBeforeStart` | 1,809 |
| Records at or above 80,000 ms | 131 |
| Maximum duration | 90,490 ms |
| Records with `outputLimitMaxBytes=null` | 2,487 |

The command and failure counts each sum to 2,487. The values describe log
records, not proven unique user requests.

### Repository relation

The review logs explicitly name the umbrella `~/work` root in 663 records.
They name `~/work/openagents` in 15 records.

The first UTC-date `t1` record ends a root diff after 87,382 ms. Thus, the
session retained the workload shape from the prior OOM report.

### Why Git is not the direct trigger

The final Git record occurred at `2026-07-20T10:46:02.638Z`. The fatal trap
occurred at `2026-07-20T10:50:58.020Z`.

The Git worker was idle in its event loop at the crash time. The browser main
thread executed the trap.

Git review pressure can still increase shared process resource use. The report
therefore treats it as a causal workload with medium confidence.

## Recurrence analysis

### Local crash sequence

| Local crash time | Build | PID | Fatal thread | Signal | Primary stack class | Session age |
| --- | --- | ---: | --- | --- | --- | --- |
| 2026-07-17 12:13:34 | `26.715.21425 (5488)` | 47847 | `0 CrBrowserMain` | `SIGTRAP` | V8 value deserializer | 5 h 17 m 37 s |
| 2026-07-17 18:35:55 | `26.715.31251 (5538)` | 3746 | `54 git` | `SIGABRT` | Node OOM handler | 6 h 17 m 16 s |
| 2026-07-19 05:20:03 | `26.715.31925 (5551)` | 17908 | `54 git` | `SIGABRT` | Node OOM handler | 4 h 34 m 6 s |
| 2026-07-19 12:22:56 | `26.715.31925 (5551)` | 67605 | `54 git` | `SIGABRT` | Node OOM handler | 4 h 17 m 32 s |
| 2026-07-19 18:35:49 | `26.715.31925 (5551)` | 1074 | `54 git` | `SIGABRT` | Node OOM handler | 5 h 49 m 26 s |
| 2026-07-20 05:50:58 | `26.715.52143 (5591)` | 68077 | `0 CrBrowserMain` | `SIGTRAP` | V8 value deserializer | 11 h 14 m 47 s |

The archive contains two stable crash classes. Both classes occur after long,
active desktop sessions.

### Same-signature SIGTRAP

The 2026-07-17 SIGTRAP has the same first ten symbolic frames as the current
crash. Both reports use thread 0, `CrBrowserMain`, `brk 0`, and three V8
deserializer frames.

The same crash class persisted from build 5488 to build 5591. The builds have
different private addresses, but the stack shape is stable.

### Related OOM signature

Four reports use thread 54, the name `git`, `SIGABRT`, and
`node::OOMErrorHandler`. The prior report documents that class in detail.

The current crash is not another direct Git-worker OOM. The related history
shows that the same application workload has repeated resource failures.

## Causal model

### Direct cause

The Electron main process executed Chromium's ARM64 immediate-crash instruction
during V8 value deserialization. `SIGTRAP` terminated Codex Desktop.

### Immediate workload trigger

The main process handled five concurrent task streams. It sent
`reasoning-summary` events to the primary renderer and a hidden overlay renderer.

### Environmental factors

The app had run for more than 11 hours. The same session created 2,487 Git
review-summary records and 131 long Git records.

The local archive also shows four recent fatal OOM events. These facts increase
the probability of retained resource pressure.

### Product root cause

The main-process message or resource lifecycle reached a fatal internal
condition under a valid concurrent workload. The product did not contain or
recover from that condition.

### Control failure

The product had no visible limit or circuit breaker for renderer fanout,
retained summary state, main-process memory, or total background work.

It also did not give the user a recoverable error before process termination.

## Five-why analysis

### Why 1: Why did the application exit?

The main process received `SIGTRAP` after it executed `brk 0`.

### Why 2: Why did the process execute `brk 0`?

Chromium selected its immediate-crash path after an internal fatal condition.

### Why 3: Which subsystem reached the condition?

The stable stack places the condition in V8 value deserialization on
`CrBrowserMain`.

### Why 4: Which workload preceded the condition?

Five task streams produced high-rate `reasoning-summary` traffic for two
renderer consumers during a long active session.

### Why 5: Why did a valid workload terminate the app?

The main-process path lacked sufficient limits, isolation, or recovery for the
observed message and resource state.

## Factors that increased the risk

### F1. The process had a long active session

The app session lasted more than 11 hours. Retained state can grow across a
long session, but the report has no heap history.

### F2. Five task streams were active

Five distinct task identifiers produced events in the final two minutes.

### F3. Two renderer consumers received the stream

The primary and overlay renderers received almost equal event counts.

### F4. The app enabled concurrent summaries

Both renderers recorded `featureOverride=true` for concurrent summaries.

### F5. The overlay was hidden but active

The overlay used `rendererWindowVisible=false` while it continued to receive
summary events.

### F6. Git review pressure remained in the session

The session created 2,487 review-summary records and 131 long records.

### F7. Review command limits were not visible

All applicable Git records had `outputLimitMaxBytes=null`.

### F8. Recent crashes had fatal memory paths

Four earlier reports contain `node::OOMErrorHandler`.

### F9. The fatal condition had process scope

The browser main process owned the fatal path. Its exit ended the whole app.

### F10. No proactive resource notice appeared

The logs did not show a user notice for message rate, memory, or fanout.

## Causes that the evidence does not support

### Not an ordinary segmentation fault

The exception was `EXC_BREAKPOINT`, not `EXC_BAD_ACCESS`. The ARM state shows
an explicit `brk 0` instruction.

### Not a proven Git-worker OOM

The fatal thread was thread 0. The stack has no `node::OOMErrorHandler`.

### Not a normal Git exit code

The final Git command aborted before start. A Git exit did not terminate the
main process.

### Not an OS memory kill

The termination namespace was `SIGNAL`. The process executed a trap instruction.

### Not proof of total host RAM exhaustion

The host had 128 GB of installed memory. The report does not give free memory
or process peak RSS at the crash time.

### Not proof of a memory leak

The event history supports resource-pressure concern. It does not show object
retention across heap snapshots.

### Not proof of a debugger action

No external process targeted this process in the report. Chromium itself uses
`brk #0` for immediate crash.

### Not proof of DNS failure

The top exported symbol includes `ares`. Limited symbols make that name
unreliable for private code.

### Not proof of hardware failure

The repeated product-specific stacks support a software failure. No report
field identifies a hardware fault.

### Not proof of repository corruption

The current fatal stack does not show an invalid Git object or failed file
write.

### Not proof that ResizeObserver caused the crash

The renderer errors are adjacent events. The fatal stack is in the Electron
main-process deserializer.

## Data-integrity assessment

The crash ended in-process state without a normal close sequence. This event
can interrupt incomplete app persistence or task delivery.

The report does not show a partial commit, changed branch, changed remote, or
repository lock failure. File changes already written should remain.

The Git commands near the event used status, diff, and hash operations. They do
not normally change tracked work-tree files.

No evidence proves that each app-local task event reached durable storage. The
operator must review in-flight task status after restart.

## Detection gaps

1. The app did not report main-process heap use.
2. The app did not report V8 external memory.
3. The app did not report retained serialized-message bytes.
4. The app did not report summary queue depth.
5. The app did not report per-renderer fanout rate.
6. The app did not suppress a hidden overlay consumer.
7. The app did not emit the failed `CHECK` text.
8. The app did not emit a final fatal log record.
9. The crash report did not include peak RSS.
10. The app did not recover the main-process operation.
11. The app did not give one stable user support code.
12. The app did not identify the final serialized value.

## Evidence confidence

| Statement | Confidence | Reason |
| --- | --- | --- |
| Thread 0 caused the crash. | High | Apple identifies the fatal thread. |
| Chromium executed an intentional trap. | High | The ARM state shows `brk 0`. |
| V8 deserialization was active. | High | Three stable frames name `ReadValue`. |
| Five task streams were active. | High | The final log window has five identifiers. |
| Two renderers received the stream. | High | The final counts are 69 and 68. |
| The Git worker directly caused this trap. | Low | It was not the fatal thread. |
| Long-session resource pressure contributed. | Medium | Session age and failure history support it. |
| A memory leak existed. | Low | No heap history exists. |
| One named message caused the trap. | Low | The final message is not identified. |
| The evidence identifies the exact Chromium assertion. | Low | The release binary lacks private symbols. |

## Limits of this report

- The terminated process cannot provide a new heap snapshot.
- The crash report lacks peak process memory.
- The app source and private symbols were not available.
- The investigation did not reproduce the crash on purpose.
- The log aggregate cannot identify one final serialized value.
- The task count is an event-window count, not a scheduler inventory.
- The report does not identify the exact V8 limit value.
- The report does not prove object retention.
- The report does not prove that fanout alone causes the failure.
- The prior OOM class has a related workload but a different fatal thread.

## Immediate containment

### C1. Restart Codex Desktop after a long high-load session

A restart creates a new V8 process and clears retained process state. It does
not correct the product defect.

### C2. Reduce concurrent active task streams

Keep fewer simultaneous task streams until the product has a measured
fanout and memory limit.

### C3. Disable or close the overlay when it is not in use

Use a product control for the avatar overlay if one exists. A hidden overlay
should not consume duplicate summary traffic without a product need.

This report did not confirm the name or availability of such a control.

### C4. Use child repository scope

Open the exact child repository for a task. Do not use the umbrella `~/work`
root when a child repository owns the work.

### C5. Avoid large review-summary scans at umbrella scope

The session retained the Git review pattern from the prior OOM incident.

### C6. Keep the crash evidence

Keep the Apple report and applicable logs until the product team accepts the
incident. Keep them outside the public repository.

## Permanent corrective actions

### P0 product actions

| ID | Action | Acceptance condition | Status |
| --- | --- | --- | --- |
| CDX-TRAP-001 | Capture the fatal check identity. | Support data names the exact check without private content. | Proposed |
| CDX-TRAP-002 | Bound serialized message bytes. | One event cannot exceed the documented byte limit. | Proposed |
| CDX-TRAP-003 | Bound summary queue depth. | Queue size cannot grow with active event count. | Proposed |
| CDX-TRAP-004 | Bound renderer fanout. | Hidden consumers do not receive unneeded duplicate events. | Proposed |
| CDX-TRAP-005 | Add per-consumer backpressure. | A slow consumer cannot retain an unlimited queue. | Proposed |
| CDX-TRAP-006 | Add stale event fences. | A closed task or renderer cannot receive new events. | Proposed |
| CDX-TRAP-007 | Add a main-process memory gate. | The app degrades before its documented cap. | Proposed |
| CDX-TRAP-008 | Isolate deserialization risk. | A bad worker value cannot terminate the trusted shell. | Proposed |
| CDX-TRAP-009 | Recover active task state. | The app restores durable tasks after helper failure. | Proposed |
| CDX-TRAP-010 | Add a circuit breaker. | Repeated fatal-risk signals stop new fanout. | Proposed |

### P1 product actions

| ID | Action | Acceptance condition | Status |
| --- | --- | --- | --- |
| CDX-TRAP-011 | Add message telemetry. | Diagnostics include bytes, type, queue, and consumer count. | Proposed |
| CDX-TRAP-012 | Add memory telemetry. | Diagnostics include heap, external memory, and RSS. | Proposed |
| CDX-TRAP-013 | Add hidden-renderer lifecycle tests. | Hidden consumers stop when their UI closes. | Proposed |
| CDX-TRAP-014 | Add long-session soak tests. | An 18-hour test has no unbounded resource trend. | Proposed |
| CDX-TRAP-015 | Add concurrent-stream stress tests. | The app survives the configured maximum task count. | Proposed |
| CDX-TRAP-016 | Add a stable support code. | Recovery shows one crash-class identifier. | Proposed |

### Open actions from the OOM report

The corrective actions in the prior report remain applicable. File, byte,
queue, output, concurrency, cancellation, and process-isolation limits still
need product proof.

The new report adds a separate main-process message boundary. A fix for only
the Git-worker OOM path does not close this SIGTRAP class.

## Required product behavior

This behavior must hold for every valid task and renderer state:

1. A serialized message must have a finite byte limit.
2. Each consumer queue must have a finite depth.
3. A hidden consumer must stop when it has no product need.
4. A stale task must not publish a new event.
5. A stale renderer must not receive a new event.
6. A slow consumer must apply backpressure or explicit truncation.
7. Main-process memory must stay within a documented bound.
8. Background Git work must stay within its documented bounds.
9. A helper failure must not terminate the trusted shell.
10. Active tasks must retain durable recovery state.
11. The user must receive a clear degraded-result notice.
12. Diagnostics must name the limit or fatal check that stopped work.

## Regression test plan

### Test matrix

| Test | Workload or fault | Expected result |
| --- | --- | --- |
| T1 | One task and one visible renderer | Full stream completes. |
| T2 | Five tasks and one renderer | The configured concurrency limit applies. |
| T3 | Five tasks and two renderers | Fanout stays within queue and byte limits. |
| T4 | Hidden overlay during active streams | Unneeded delivery stops. |
| T5 | Slow overlay consumer | Backpressure prevents queue growth. |
| T6 | Oversized serialized value | Decode refuses without process death. |
| T7 | Malformed serialized value | Decode returns a typed failure. |
| T8 | Renderer close during fanout | Stale delivery stops. |
| T9 | Task cancel during fanout | Stale event publication stops. |
| T10 | Main helper memory pressure | The app degrades before the cap. |
| T11 | Git review and five task streams | Both workloads stay within limits. |
| T12 | 125,000-file Git fixture | The review truncates without app death. |
| T13 | 18-hour sustained session | Resource use has no unbounded trend. |
| T14 | Helper fault injection | The main app remains available. |
| T15 | App restart after helper failure | Durable task state recovers. |

### Test measurements

Each test must record these values:

- Main-process resident memory.
- V8 heap used and heap limit.
- V8 external-memory value.
- Serialized input bytes by message type.
- Message count by task and renderer.
- Maximum queue depth by consumer.
- Maximum concurrent task count.
- Stale-event refusal count.
- Truncated-event count.
- Backpressure duration.
- Git command count and output bytes.
- Active handles, streams, and workers after stop.
- Main-app availability.
- Active-task recovery result.

### Failure assertions

The tests must fail if any of these events occurs:

- `EXC_BREAKPOINT (SIGTRAP)` reaches the main process.
- The main process executes `brk 0` for a workload input.
- `node::OOMErrorHandler` appears in the main process.
- A queue exceeds its configured limit.
- A message exceeds its configured byte limit.
- A hidden closed consumer receives a new event.
- A canceled task publishes a new event.
- A malformed value terminates the process.
- Git review exceeds its configured resource limits.
- The app omits the degraded-result notice.
- The app loses a durable active-task record.

## Operator recovery procedure

Use this procedure after a similar crash:

1. Save the Apple crash report.
2. Note the incident identifier and product build.
3. Save the applicable `t0` and `t1` app logs.
4. Do not publish raw prompts or private task identifiers.
5. Check the fatal thread name.
6. Check the ARM instruction at the program counter.
7. Check for `v8::ValueDeserializer::ReadValue`.
8. Check for `node::OOMErrorHandler`.
9. Count active task identifiers in the final two minutes.
10. Count renderer consumers in the final two minutes.
11. Count review-summary Git records for the session.
12. Restart Codex Desktop.
13. Review in-flight task and repository state.
14. Compare the new report with both known crash classes.

## Evidence collection commands

The commands below are source data. They use normalized local paths.

### Crash signature

```sh
rg -n 'Triggered by Thread|Exception Type|Termination Reason|brk 0|ValueDeserializer' \
  ~/Library/Logs/DiagnosticReports/ChatGPT-2026-07-20-055101.ips
```

### Final main-log window

```sh
awk '$1 >= "2026-07-20T10:48:58.020Z" {print}' \
  ~/Library/Logs/com.openai.codex/2026/07/20/*-68077-t0-*.log
```

### Task and renderer counts

```sh
rg -o 'threadId=[^ ]+' <final-window-log> | sort -u | wc -l
rg -c 'rendererWindowAppearance=primary' <final-window-log>
rg -c 'rendererWindowAppearance=avatarOverlay' <final-window-log>
```

### Git-worker aggregate

```sh
rg -c 'requestKind=review-summary' \
  ~/Library/Logs/com.openai.codex/2026/07/{19,20}/*-68077-t1-*.log
```

### Recurrence signature

```sh
rg -n 'node::OOMErrorHandler|ValueDeserializer::ReadValue|Triggered by Thread' \
  ~/Library/Logs/DiagnosticReports/ChatGPT-2026-07-{17,19,20}-*.ips
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
| Translated current report | 220,122 | 1,299 | `cb5e3c481946dfd5db6f05b9e408dbf400ee2503d31ebcfbe58a554801a2f462` |
| `ChatGPT-2026-07-20-055101.ips` | 142,525 | 381 | `5101e55116307fc71feddbef9992bd70d46b06eff27372e96d0cb3a2aca49bc9` |
| Current UTC-date `t0` log | 7,076,626 | 19,128 | `8ee8f6d7f041682a3c910cdf8d3b94c924858ab2ef56793a601a9d67812b0d11` |
| Current UTC-date `t1` log | 1,366,221 | 2,469 | `d45f62020cfb06810d14110a7b62a01ebf74a89644dadef3cbfecf6ccd2d9463` |
| Prior UTC-date `t0` log | 248,644 | 654 | `43a864e7f1b5f18115d638630f9ffa77ee3f664d7a2b33c48e6f65f69d00ef5b` |
| Prior UTC-date `t1` log | 135,965 | 304 | `9375455c8d1b31a6545b09bfa5dd14074140f058790603cb36d2301f8601ff20` |
| 2026-07-17 SIGTRAP report | 137,384 | 361 | `1267aa570ebe672814c758079d74ea32217ddc8836790acdd033c15d35638b2d` |
| 2026-07-17 OOM report | 139,572 | 371 | `89a288590f7fdba0c62f98bc67aa1c64000f472e170bf8db4730c01c692c8e9f` |
| 2026-07-19 05:20 OOM report | 138,428 | 368 | `ecd306fcab82c9e20780876c6363089c13398cef8f64a6f3c12cbb82b53664df` |
| 2026-07-19 12:22 OOM report | 138,280 | 368 | `d383313c61d58d025b9b981972017fde10388166432683cfcecea49ddfdffb10` |
| 2026-07-19 18:35 OOM report | 145,005 | 392 | `1f56c7470498de52dcc7b115a13f5ee4b37a3fdfa7f2f46b847d9bd4815d962e` |

## Redaction record

The report applies these redactions:

- It replaces the local account name with `~`.
- It omits raw prompts and model event payloads.
- It omits private task and turn identifiers.
- It omits raw shell output and file content.
- It omits credentials, tokens, and secret values.
- It keeps incident identifiers because they identify crash evidence.
- It keeps renderer identifiers because they explain process fanout.
- It keeps public repository paths where they explain workload scope.

## Action register

| Action | Owner | Priority | State |
| --- | --- | --- | --- |
| Publish this after-action report. | OpenAgents documentation | P0 | Complete in this change |
| Restart after sustained high-load sessions. | Local operator workflow | P0 | Recommended |
| Reduce simultaneous stream count. | Local operator workflow | P0 | Recommended |
| Stop hidden overlay delivery when unused. | Codex Desktop product | P0 | Proposed |
| Add message, queue, and fanout limits. | Codex Desktop product | P0 | Proposed |
| Add main-process memory limits. | Codex Desktop product | P0 | Proposed |
| Isolate risky helper decode and review work. | Codex Desktop product | P0 | Proposed |
| Add incident-scale regression tests. | Codex Desktop product | P0 | Proposed |
| Add fatal-check and memory diagnostics. | Codex Desktop product | P1 | Proposed |
| Keep local evidence until product triage. | Evidence custodian | P1 | Recommended |

## Closure criteria

This incident can close only after these conditions hold:

1. The product identifies the exact fatal check for this signature.
2. Serialized values have finite byte limits.
3. Each renderer queue has a finite depth.
4. Hidden renderer delivery has an explicit lifecycle.
5. Main-process memory has a tested upper bound.
6. Git review work has finite resource limits.
7. A malformed or oversized value cannot terminate the app.
8. A helper or worker failure cannot terminate the trusted shell.
9. The 18-hour concurrent-stream soak test passes.
10. The incident-scale Git and stream combination passes.
11. Active task state recovers after a contained failure.
12. No known crash class recurs across the accepted soak window.

The documentation task can close after this report passes repository checks,
reaches `main`, and exists in the final STE inventory.

## Final assessment

The evidence establishes an intentional Chromium fatal trap in the Electron
main process. The stable stack places the trap in V8 value deserialization.

The evidence also establishes five concurrent task streams, two renderer
consumers, a hidden active overlay, and a long high-load session.

The evidence does not establish one exact failed message or a proven memory
leak. Those claims require product symbols and memory evidence.

The permanent fix must bound message work before deserialization and fanout. It
must also preserve the app when a helper or worker fails.

Until that fix exists, session restarts, lower concurrency, exact repository
scope, and unused-overlay closure provide the safest containment.

## Addendum: relation to the prior OOM report and OpenAgents IDE prevention

### Addendum control

| Field | Value |
| --- | --- |
| Addendum date | 2026-07-20 |
| Subject | Unified prevention for two recurrent Codex Desktop crash classes |
| Scope | OpenAgents Desktop IDE message, project, Git, and worker paths |
| Claim type | Conditional architecture and release-gate conclusion |
| External product effect | None |

### Purpose

This addendum relates the current SIGTRAP to the prior Git-worker OOM report.
It also extends the OpenAgents IDE prevention contract.

The two events have different direct causes. The prior event ended in the Git
worker OOM handler. The current event ended in the browser main process.

They share one product-risk category. Valid background work can grow or move
through an application-wide process without complete limits and isolation.

### Prior report relationship

The prior report is
[2026-07-19 Codex Desktop Git Review OOM Incident After-Action](./2026-07-19-codex-desktop-git-review-oom-incident-afteraction.md).

That report defines file, byte, queue, command, cancellation, memory, and
worker-isolation controls. All those controls remain necessary.

The current report adds serialized-message size, renderer queue, hidden-consumer
lifecycle, fanout, and main-process survival controls.

### Unified failure-class decomposition

The combined failure class has this sequence:

1. A valid user workload creates concurrent background work.
2. Project review creates Git diff and hash work.
3. Task streams create serialized summary events.
4. Multiple consumers receive the events.
5. Work or retained state reaches an unsafe product condition.
6. A worker OOM or main-process fatal check occurs.
7. The process boundary lets that failure end the whole app.

The OpenAgents IDE must break each link. A renderer limit alone is not enough.
A Git limit alone is also not enough.

### Two-boundary prevention model

The first boundary limits admitted work:

```text
admitted_files <= file_limit
review_bytes <= review_byte_limit
git_output_bytes <= git_output_limit
queued_git_operations <= git_queue_limit
active_git_operations <= git_concurrency_limit
serialized_message_bytes <= message_byte_limit
queued_messages_per_consumer <= consumer_queue_limit
active_streams <= stream_limit
retained_completed_items <= retention_limit
```

Each limit must be finite and positive. The product must apply each limit
before content expansion or message fanout.

The second boundary contains implementation defects. Git, review, and risky
decode work must have a separate process-fate boundary from the trusted shell.

The helper must receive only a decoded, bounded request. It must not receive
session, credential, policy, database, approval, or receipt authority.

If the helper exits, the trusted shell must remain available. The supervisor
must publish a typed failure and preserve durable task state.

### Incident-specific OpenAgents controls

| ID | Required control | Acceptance condition |
| --- | --- | --- |
| OA-IDE-TRAP-001 | Exact task and renderer attachment | Each event binds one task, generation, and consumer. |
| OA-IDE-TRAP-002 | Message byte limit | Oversized values refuse before full decode. |
| OA-IDE-TRAP-003 | Consumer queue limit | Each queue stays below its finite maximum. |
| OA-IDE-TRAP-004 | Fanout limit | One event has a finite consumer count. |
| OA-IDE-TRAP-005 | Hidden-consumer lifecycle | An unused hidden consumer stops delivery. |
| OA-IDE-TRAP-006 | Slow-consumer backpressure | A slow renderer cannot retain unlimited events. |
| OA-IDE-TRAP-007 | Stale generation fence | Old tasks and consumers cannot publish. |
| OA-IDE-TRAP-008 | Retention limit | Completed summary state has a finite lifetime. |
| OA-IDE-TRAP-009 | Main memory gate | The shell degrades before its memory cap. |
| OA-IDE-TRAP-010 | Decode isolation | A bad helper value cannot terminate the shell. |
| OA-IDE-TRAP-011 | Typed degradation | The app shows the stopped limit and recovery action. |
| OA-IDE-TRAP-012 | Durable recovery | Active tasks survive a helper replacement. |

These controls supplement OA-IDE-OOM-001 through OA-IDE-OOM-016 in the prior
report. Neither control set replaces the other.

### Combined regression corpus

| Test | Fixture or fault | Required result |
| --- | --- | --- |
| OA-TRAP-T1 | Five streams and two renderer consumers | Queues and bytes stay within limits. |
| OA-TRAP-T2 | Hidden overlay consumer | Unneeded delivery stops. |
| OA-TRAP-T3 | Slow renderer | Backpressure keeps a finite queue. |
| OA-TRAP-T4 | Oversized serialized value | Decode refuses without shell death. |
| OA-TRAP-T5 | Malformed serialized value | A typed error replaces the event. |
| OA-TRAP-T6 | Renderer close during fanout | Stale delivery stops. |
| OA-TRAP-T7 | 125,000-file Git fixture and five streams | Both work classes stay bounded. |
| OA-TRAP-T8 | Git-worker OOM injection | The shell remains available. |
| OA-TRAP-T9 | Decode-helper fatal injection | The shell remains available. |
| OA-TRAP-T10 | 18-hour combined soak | No unbounded memory or queue trend occurs. |
| OA-TRAP-T11 | Helper restart | Durable task state recovers. |
| OA-TRAP-T12 | Seven repeated combined sessions | Neither known crash signature appears. |

### Required measurements

Each combined test must record these values:

- visible and admitted file counts.
- Git command count and output bytes.
- active and queued Git operations.
- active task and consumer counts.
- serialized bytes by message type.
- maximum consumer queue depth.
- retained completed-item count.
- worker and main-process memory.
- cancellation and stale-refusal counts.
- active handles, streams, and workers after stop.
- user-visible degraded-state code.
- durable task recovery result.
- exact candidate and app-tree digests.

### Failure assertions

The combined gate fails if any of these facts occurs:

- The main process receives `SIGTRAP` for a workload input.
- `node::OOMErrorHandler` appears in the main-process path.
- A Git or review worker failure terminates the shell.
- A decode-helper failure terminates the shell.
- A message or queue exceeds its configured limit.
- A hidden closed consumer receives a new event.
- A canceled generation publishes a new event.
- Repository size increases active work after all limits apply.
- The app loses active task or canonical document state.
- Teardown leaves an owned worker, queue, stream, or subscription.
- The UI hides truncation, omission, memory pressure, or circuit state.

### Addendum closure criteria

This addendum can support a prevention claim only after all these conditions
hold:

1. An authoritative ProductSpec or admitted packet binds both control sets.
2. Git and message limits apply before work expansion.
3. Risky helpers have a separate process-fate boundary.
4. The complete combined corpus passes on each supported Desktop target.
5. Receipts contain all required measurements and exact digests.
6. A distinct reviewer reproduces both injected failure classes.
7. The normal IDE and chat regression corpora remain green.
8. The release claim names the exact IDE rung and open gaps.

### Addendum determination

The OpenAgents IDE can prevent both observed crash classes by construction.
Exact scope and fixed resource limits keep valid workloads within bounds.

Process isolation then protects the shell from a worker or decode defect. Typed
degradation and durable task state preserve the user task after failure.

This report makes no release claim for the complete prevention contract.
The claim remains conditional on implementation and incident-scale evidence.

After those gates pass, a large repository and concurrent task streams can
produce only a bounded, visible, recoverable result. They cannot terminate the
OpenAgents Desktop shell.
