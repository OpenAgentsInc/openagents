# Subprocess supervision

Three places in this workspace run other programs: the shell loop runs a
proposal as `sh -c`, delegation hands a task to an executor, and CoderBench
runs the agent itself. All three want the same two properties, and both are
easy to get wrong in the same way. `crates/supervise` is the one place that
gets them right, and the three call sites use it.

## A deadline is an ownership contract

`timeout(wall, command.output())` cancels the *wait*. The command keeps
running, its descendants keep running, and nobody reaps any of them. The
[September 2026 audit](../../audits/2026-09-19-codebase-audit/README.md)
reproduced both halves of that: a shell command reported `timed out` at 15
seconds and wrote its marker file a second later, and a delegate killed on
its bound left a background child that wrote one too. An earlier
`kill_on_drop(true)` on the delegate was not enough, because Tokio's
`kill_on_drop` concerns the direct child and establishes no process group.

A job now ends when its cleanup ends:

1. The direct child is spawned with `process_group(0)`, so it leads a new
   process group and everything it starts inherits that group.
2. The deadline, or a caller that drops the future, signals `SIGTERM` to
   the **group**.
3. The supervisor waits up to 250 milliseconds for the direct child.
4. It signals `SIGKILL` to the group and reaps the direct child.

Only then does the job report. A delegation's worktree is therefore removed
after the executor is gone rather than while it is still writing to it.

**Cancellation is the same mechanism as a deadline.** The work runs in a
task of its own, and the future the caller holds owns the only live end of a
channel. An aborted turn, a lost `select!` branch, or an outer timeout drops
that future, which closes the channel, which terminates the tree exactly as
an expired deadline does. The caller does not learn what the job said,
because it stopped waiting; the machine does not keep the process.

`Job::run_holding` also retains a resource until process and output cleanup
finish. Delegation uses it for the checkout guard: cancelling the caller cannot
remove a checkout while the supervisor is still terminating its writer.

`Job::from_command` takes ownership of a prepared `std::process::Command`.
This preserves a boundary wrapper's arguments, working directory, and
complete environment policy, including `env_clear`. Supervision replaces
standard input with null, captures both output streams, and establishes its
own process group. The job owns the command and is no longer `Clone`.
The prepared-command regression checks directory and environment preservation;
all 25 supervisor tests and its doctest pass under Rust 1.97.1.

A job that exits on its own is the same contract read the other way.
Whatever is still in the group is a descendant that outlived the job it
belongs to, so the group is killed there too — without a grace period, since
the job already has its result. That is also what ends a capture on a pipe a
grandchild is still holding open.

## Output is bounded while it is captured

Truncating the string an `output()` call returns bounds what a trace records
and bounds nothing the machine has to hold: a producer that writes a
gigabyte inside its allotted seconds is a gigabyte in memory first, and a
fan-out of six multiplies it.

So stdout and stderr are drained concurrently into capped buffers. Bytes
past the cap are counted and dropped as they arrive, and draining continues
after the cap rather than stopping — a pipe nobody reads fills, and a writer
the kernel has stopped cannot answer its own deadline. Overflow does not
terminate the job.

| What | Bound |
| --- | --- |
| Peak capture memory per job | `2 × stream_max`, plus two 8 KiB read buffers |
| Shell command, per stream | 16 KiB (`shell::OUTPUT_MAX`) |
| Delegation, per stream | 64 KiB (`delegate::OUTPUT_MAX`) |
| Capability probe, per stream | 64 KiB (`capability::bounded::OUTPUT_MAX`) |
| Watched executor session, standard output not yet taken | 8 MiB (`adapter::STREAM_KEEP`); past it, arriving bytes are dropped and reported as a gap |

Each captured stream reports three things: the text that was kept, how many
bytes there were in all, and whether the cap cut it. A cut stream carries a
marker — `…truncated, 1048576 bytes in all` — so a reader can tell a quiet
command from a truncated one. A cap that lands inside a multi-byte character
drops the incomplete tail rather than leaving a replacement character
behind.

**A failed job keeps its output.** A command that reached its deadline, and
a delegation that did, both report what they printed first. A timed-out
job's partial output is often the most useful thing it produced.

## Where the files go

CoderBench is the exception to capture, on purpose. A run's output goes to
`<trace>.stdout` and `<trace>.stderr` beside its trace, because those files
are part of the record a reader opens afterwards, and because CoderBench
waits rather than reads — a pipe nobody is reading fills and stops the
child. Those files are kept. A probe — Coder's survey or a CoderBench
preflight — is not the exception: it runs through `capability::bounded`,
which is `Job::from_command` under a wall clock and the capped capture, on
a thread of its own so a synchronous caller inside an asynchronous host
does not nest runtimes.

CoderBench uses the blocking half of the same crate, so the process-tree
rules above apply to it unchanged.

## Platform support

Unix only, and stated rather than assumed. Process-tree ownership here is
`process_group(0)` and `killpg`. No equivalent is implemented for another
platform, so `supervise` does not compile on one. Claiming otherwise without
an implementation and tests is the failure this crate exists to remove.

## What this does not do

- **It is not a sandbox.** It bounds time and captured output. It does not
  bound what a program reads, writes, or sends, and it does not bound memory
  or CPU. A probe's argv additionally runs only under an operator approval —
  `crates/capability` owns that — and enforced admission for delegated work
  is separate work.
- **A process that leaves the group escapes it.** A descendant that calls
  `setsid` is no longer in the group the job owns, and nothing here reaches
  it. The capture waits a bounded time for such a process to close its end
  of a pipe rather than waiting forever.
- **There is a window between reaping and the last signal.** A group
  identifier belongs to a job while its leader exists, and the supervisor
  signals the group immediately after reaping the leader. Closing that
  window needs `waitid(WNOWAIT)`, which is not on the asynchronous runtime's
  wait path. The trade is written down in `crates/supervise/src/group.rs`
  rather than implied.

## Tests

`crates/supervise/tests/` holds the cases in the audit's shape: a harmless
marker file written after the job was supposed to be over, and a marker that
exists afterwards is a process that outlived its bound. The set covers a
grandchild on a deadline, a cancelled future, an outer timeout, a child that
exits while a descendant remains, two simultaneous jobs where only one is
terminated, a child that ignores `SIGTERM`, reaping, oversized output on
both streams at once, a producer that never stops, split UTF-8 at the cap,
and partial output retained on a timeout. `crates/coder` and
`crates/coderbench` each repeat the marker case through their own call site.
