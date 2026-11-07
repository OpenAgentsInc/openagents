# CDP cleans up dead owners' browsers (#13020)

Each new profile records its owner PID in the directory name and the selected
executable in `cdp-executable` before Chrome starts. The next CDP launch scans
the same temporary root, including an explicit `TMPDIR`. It preserves live
owners. For a dead owner it requires the exact recorded executable and original
headless/muted/profile argument prefix on a process whose PID equals its
process group, then repeats the observation immediately before signalling.
The Coax decision calls a thin POSIX group-signal effect so Chrome's helpers
are terminated too. No process-name-wide kill is used.

Only after a complete fresh process inventory shows no remaining user of the
profile can its directory be removed. Unknown or headed processes using that
profile preserve it. PID reuse of the owner conservatively postpones recovery.
No owner label means no ownership claim: old profiles remain outside this
recovery. Executable recording precedes spawning, so recovery does not depend
on the owner surviving long enough to write a browser PID file.

The process inventory is bounded to 1 MiB with a one-second command deadline;
failed, oversized or incomplete output refuses recovery. A sweep stops starting
new profile work after three seconds; group shutdown waits at most one second.
Remaining candidates can be recovered by a later launch. Ordinary close and
the browser flags are unchanged. Native C adds only the POSIX signal effect;
ownership and matching decisions are Coax.

## Checks

The policy's checks were written before its implementation. Four policy checks
pass, and all **174 mutants are killed**, with no equivalents or survivors
(`policy.txt`, `mutation.txt`). Cases include malformed/overflow PIDs, unknown
names, path and control-character injection, profile-prefix collisions,
wrong executables/flags, nonleaders and live owners. The complete CDP library
passes **129 checks**, including its existing argument/protocol cases (`cdp.txt`).

The physical regression runs the actual compiled `shot` tool against a plain
data page, with a 30-second wait. It runs in an isolated temporary root, and
every browser is headless and muted:

1. Launch another live owner's browser as a preservation control.
2. Start `shot`, find its real Chrome and profile, then SIGKILL only the tool.
   Confirm the browser and profile still exist before recovery.
3. Launch another CDP browser. Its startup must remove the orphan's complete
   process group and profile.
4. Ask the live control to evaluate `6*7`; it must still return 42 and retain
   its profile. Close the live browsers normally.
5. Require the fd-guard log's modification time to remain unchanged throughout
   the case. A changed log refuses the result; the test never invokes or
   disables the LaunchAgent.

Before the sweep implementation (owner labels already installed), the physical
case failed on the surviving orphan/profile (`physical-red.txt`). With recovery
implemented, it passes (`physical.txt`), logs the actual group signal, and
leaves no test browser running. This was measured on macOS with Chrome
157.0.8103.39. Linux runtime and unrelated historical profile formats are not
qualified by this Mac check.

```sh
coax/bin/coax test coax/lib/net/cdp-owner-policy.coax --library
coax/bin/coax mutate coax/lib/net/cdp-owner-policy.coax --library
coax/bin/coax test coax/lib/net/cdp.coax --library
# Build shot via tool.sh first (no arguments prints usage with status 2).
coax/web/tool.sh cdp-owner-check PATH_TO_COMPILED_SHOT
coax/web/tool.sh tools-check
```

`PATH_TO_COMPILED_SHOT` is the compiler-qualified executable created by
`coax/web/tool.sh shot` under `coax/web/build/tools/shot-COMPILER_SHA256`.
The new physical regression itself is Coax; no external watchdog supplies its
cleanup result.
