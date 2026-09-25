# Failure localization: offline protocol

Issue [#9658](https://github.com/OpenAgentsInc/openagents/issues/9658). This
protocol, the parser table (`crates/coder-one/src/localize/parse.rs`), and
the measurement code (`crates/coder-one/src/localize/offline.rs`) were
fixed and committed before any retained command output was read. Only the
log format (the JSON keys of a session record and a tool call) was looked
at, to write the reader.

## Data

Every retained Microluna session log (`microluna-<d>-<n>.atif.jsonl`)
under these roots, found and deduplicated the way `control.stall` finds
them (`crates/coder-one/src/component/stall.rs`): the first copy of each
trial is read, and interrupted copies and live mirrors are skipped.

- `~/.openagents/terminal-bench/jobs`
- `~/.openagents/terminal-bench/microluna-jobs-9585`
- `bench/terminal-bench/traces`

`sources.json` pins each file read by SHA-256.

**Excluded tasks**, before any of their records are read: the eleven tasks
the Fable pattern map read (`embedding-drift-monitor`, `coq-block-bound`,
`shadow-relay`, `risk-scorer-replay`, `mp-checkpoint-consolidation`,
`payments-pipeline-fix`, `telecom-entity-resolution`, `fp8-rmsnorm-gemm`,
`distributed-dedup`, `intrastat-meldung`, and
`photonic-waveguide-routing`). The pattern was learned from them, so they
can't count as its evidence.

## Units

- **Failing command.** A `run_command` call whose output starts `[exit N]`
  with N not 0, or `[timed out`. A command that couldn't run or was
  refused has no output to read and isn't one.
- **Parsable.** The parser table finds at least one file and line in the
  command's whole output, as the session read it.
- **Resolved.** At least one reference resolves, by
  `localize::parse::resolve`, to a **known file** of the trial: a file a
  session brief carried as `The current <path>`, or a path any session of
  the trial read, wrote, or patched. References into the standard
  library, installed packages, and toolchains never resolve
  (`parse::OUTSIDE`). Offline, the known files stand in for the workspace
  listing the live component uses.
- **The localized region.** The first resolved reference in the
  component's order (the innermost traceback frame first), with the
  component's default window of 6 lines on each side.
- **Next edit.** The first completed `apply_patch` or `write_file` after
  the failing command in the same session.

## Next edit against the region

The session's files are followed call by call: each starts as its brief
carried it, a read from line 1 to the end of the file replaces it, a write
replaces it, and each patch chunk is applied with Microluna's own patch
code. The lines an edit changed are the old lines a line diff marks as
removed, and both neighbors of an inserted line. A patch whose chunks
don't apply to the followed text makes that file unknown.

Each failing command with a resolved reference is one of:

- **region**: the next edit changed a line of that file inside the window;
- **file only**: it changed the file, outside the window;
- **other file**: it changed other files only;
- **no edit**: the session made no edit after it;
- **undetermined**: it changed the file, but the file's text before the
  edit isn't known.

Reported for every such failing command, and apart for the last failing
command before each edit (the output the edit most plausibly answers).
The **chance reference** for an edit to the same file is the share of the
file's lines around which a window would contain a line the edit changed:
the rate a region hit would have if the named line were a random line of
the file. The comparison is an association, not a causal claim: the
session read the same output the component would print from.

## Re-reads

After a failing command with a resolved reference, until the session's
next edit or its end, count the later turns whose every call only reads
(`microluna::tools::reads_only`) and one of which reads the named file: a
`read_file` of it, or a command that names its file name. Of those, count
the turns that showed the named line itself: a `read_file` whose lines
include it, a `sed -n 'a,bp'` range that covers it, or a `cat` or `nl` of
the file. Turns in the failing command's own turn don't count; they were
issued before its output. Reported against all turns and against
read-only turns.

## Secondary counts

- Failing commands whose output shows a failing case in one of the
  mismatch formats (`localize::mismatch::FORMATS`), and how many of those
  expose stages.
- Timed-out commands, and the profiler `evidence.phase_timing` would pick
  for each.

## What counts as a negative result

- Resolved below 30% of failing commands: the error context would rarely
  have anything to print.
- A region rate among same-file edits that is not above the chance
  reference: no association between the named lines and the next edit.
- Re-read turns below 2% of all turns: little waste for the component to
  remove.

Every result is reported, negative ones included. Nothing here admits a
switch into a manifest: `executor.microluna.lean.localize` stays absent
from every manifest until a matched live run measures it. Time to a
passing check can't be measured offline, because no retained session ran
with the component on.
