# Linux bounded socket SDK and realm delivery

The actual Linux SDK now compiles the same portable worker source as Metal.
Normal and ASan/UBSan/LeakSanitizer physical-peer workloads pass the existing
Coax pressure checker without changing its limits: 125 semantic and 26
payload root mutants were already killed at the shared component checkpoint
`2720b08b0a`, and those unchanged gates are reused.

During a 1.7-second delivery stall, the SDK retains exactly six leased events
and 512 payload bytes, unchanged across the second observation. Sixty-four
empty outbound commands fit; the 65th refuses and produces the exact error
then close outcome. An oversized text produces one empty error, binary is
silent, and every delivered 128-byte payload keeps its authored sequence.
Linux's first and current normal workloads each drain 495 complete payloads
after the stall; their instrumented workloads each drain 14. These observed counts differ with
physical read batching and remain inside the pre-authored finite workload.
No comparator or tolerance was changed.

The physical peer reaches a 100 ms flush timeout after 32,030 complete frames
in the first normal run. This separately observes peer/kernel backpressure;
it does not identify kernel queue capacity with the 512-byte application
payload budget. One thousand engine contexts are replaced with the same
logical owner IDs. All four replacement connections actually open and echo,
an old held lease keeps only its retired owner, and process teardown reports
zero contexts, jobs, active workers, message bytes and threads.

The current `cbf057b592` Origin/cookie worker ownership change is also
qualified on Linux with a fresh 17-source Coax emission and zero source
differences. Its normal and instrumented pressure receipts are retained
separately. This exercises the preserved empty-context compatibility API;
it does not claim that Linux has a target cookie-jar adapter yet.

The ordinary `realm-host.c`/`host-sockets.c` path passes all 79 common
transport rows, normally and instrumented, with the new worker backend.
The unchanged 277-mutant semantic/20-mutant JSON authorities admit those
actual rows. The instrumented composition first exposed a null source
passed to `memcpy` for an empty terminal event. That red run is retained;
the dispatcher now skips the zero-length copy and the complete corrected
trace passes without sanitizer reports. The narrow HTTP side of this
workload reuses the prior frozen net-helper fixture; it does not qualify a
current compiler release, authenticated Linux world or new HTTP behavior.

Artifacts remain under
`~/.openagents/aethyrforge/host-contract-12869/linux-sockets/` locally and
on coderos-4080. `emission` preserves the earlier closure;
`emission-context` holds current HostNetwork C/header/profile alongside the
retained checker/recording face. `before-context-sources` preserves SDK
sources before the opening-context migration. Build manifests and exact
executables are in `sdk-{first,asan,context,context-asan}` and
`realm-sdk-{first,context,context-asan,zero-asan,final}`. Raw plans, complete
event bytes and peer observations are in `pressure-{first,asan,context,context-asan}`.
The final 79-row receipts, zero-copy failure and compiled source observations
are copied beside this record in `linux-sockets/`.
The complete measured SDK source archive is `context-sdk-sources.tar`, SHA-256
`cd5f843af1057f1a7b463a7f3aec80f6d593d80672feda223b66b33650ef22b8`.
Generated C/headers/profiles and exact executables remain in the same durable
directory; the committed build records identify every input and executable.

The original admitted 598 CPU compiler emitted these pure Coax inputs from
the genuine c224 compiler root, with product inputs/overlays recorded
separately. No compiler sources, receipt or load/build admission was changed.
A fresh current world preparation pinned `83ba3f1d58` and reverified the original
receipt, but its primary emission stopped at 840 seconds with status 137,
timeout true and zero output. No C/header was admitted; that failure is in
the separate `linux-audio/window-current-{preparation,native}` record.

No GPU window or audible output was opened for these checks. Browser queue
adoption, Linux opening-context selection, ordinary Vulkan world/room/audio,
remaining HTTP/clip services and presentation/compiler admission remain
open. The desktop default remains web and both follow-up issues remain open.
The local tools check reports 1,787 tracked tools, including 35 vendored
vector files, and zero new files without a firm reason.
