# Desktop freeze during benchmark preparation

On October 10, 2026, the agent made desktop applications unresponsive while
preparing the Mac for a Clef benchmark. The owner held the power button to
recover. The benchmark had not started model inference.

## Cause and responsibility

The agent sent `SIGSTOP` to 61 application, helper, and build processes,
including Chrome, Ghostty, Cursor, Slack, Loom, Aqua Voice, Obsidian, Rex Beta,
Preview, QuickTime Player, and a development-host build. This was an incorrect
way to prepare a machine that the owner was using. It suspended the applications'
event loops and left their windows unable to respond. A delayed resume watchdog
was insufficient protection: it would leave the owner with frozen applications
for up to 90 minutes.

Apple documents that [`SIGSTOP` stops a process and cannot be caught or ignored](https://developer.apple.com/library/archive/documentation/System/Conceptual/ManPages_iPhoneOS/man2/sigaction.2.html).
An [unresponsive event loop causes an application hang and spinning wait cursor](https://developer.apple.com/documentation/xcode/understanding-hangs-in-your-app).
The operation itself and the WindowServer errors support this diagnosis. The
owner's confirmation that they held the power button explains the subsequent
restart. This incident does not establish a Clef, Metal, or hardware failure.

The owner had authorized stopping interfering workloads. The agent chose an
unsafe implementation of that request; responsibility for that choice belongs
to the agent.

## Evidence and timeline

Times below are local, America/Chicago (UTC−05:00).

| Time | Evidence |
| --- | --- |
| 20:50:19 | Last update of the saved list of 61 suspended processes. |
| 20:51:54 | Power log records display off after the agent's `pmset displaysleepnow`. |
| 20:51:57 | Power log records display on following input. |
| 20:51:58–59 | WindowServer reports multiple clients with `connectionIsUnresponsive: 1` and full event buffers. |
| 20:56:01 | Last complete benchmark host sample: phase `settle`, one-minute load 1.163, AGX device/renderer/tiler utilization 0%. |
| 20:57:31 | Kernel boot time after the owner's power-button recovery. |
| 20:57:38 | Apple panic collector reads an all-zero buffer and reports no panic log. |
| 21:04:21 | Recovery script finishes; subsequent process inspection finds no stopped processes or surviving benchmark/watchdog. |

All 50 retained host samples belong to the settling phase. The request log is
empty. No model server was launched by this run. The host sampler remained
active during the application freeze, which is consistent with stopped desktop
clients rather than a machine already executing a kernel panic. The logs alone
cannot exclude every possible system fault, but the confirmed power-button
recovery resolves why the boot followed the freeze.

## Recovery and continuation

Reboot ended the stopped processes and the benchmark. The agent ran its recorded
resume script and verified that no stopped processes remained. No persistent
power configuration or system indexing setting had been changed.

The new procedure leaves desktop applications and display settings alone. It
uses a quiet lease, waits for low system load, records the measurements, and
rejects a run when load exceeds the limit. Waiting has a deadline. Cancellation
unwinds the harness and terminates only the benchmark servers that it started.
The agent does not signal existing applications or system services to manufacture
quiet conditions.

The first run produces no latency result and must not be included in a performance
comparison. Full system logs stay in private scratch because they contain local
application and machine information. The accompanying sanitized incident record
and host samples retain the evidence relevant to this experiment.
