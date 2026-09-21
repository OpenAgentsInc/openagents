# Traces

Every Coder Terminal conversation writes itself to a local ATIF trace as
it runs. Nothing uploads, nothing phones home, and nothing has to be
asked for: a conversation on your machine leaves a file on your machine.

Status: implemented in `crates/atif` (the format and the log) and
`crates/coder` (`trace.rs`, wired through `agent.rs` and `main.rs`).

## Why this exists

`crates/coder` used to persist nothing. The transcript lived in `Agent`
for the life of the process and went when the terminal exited. So when
[issue #9379](https://github.com/OpenAgentsInc/openagents/issues/9379)
set out to score `classify.rs` on real `coder` turns, there were none to
score, and the suite had to be harvested from a different agent's session
record instead — 95 states measuring something adjacent to `coder` rather
than `coder`. Every measurement this repository has made rests on suites
written for the purpose. The one workload this repository owns was the
one it could not see.

A trace is the other half of the Gym's picture:

- **Rows** are per-decision, receipt-chained, and comparable across
  doors. They say what one door answered for one state.
- **Traces** are per-episode and ordered. They say what happened *next*,
  which is what an outcome label is derived from.

## Where the traces go

```text
~/.openagents/traces/<session>.atif.jsonl
```

One file per session, the directory and the files readable only by you.
The name leads with the session's UTC start time, so listing the
directory lists your history:

```text
20260919T142233Z-4f1a9c02.atif.jsonl
```

Three settings change that:

| Setting | Effect |
| --- | --- |
| `CODER_TRACE_DIR=<path>` | Write traces to `<path>` instead. |
| `CODER_TRACE=off` | Record nothing. `0`, `no`, and `false` also work. |
| `--trace <path>` | Write this session to that file. |

With `HOME` unset and no `CODER_TRACE_DIR`, there is nowhere to write and
recording is off.

`--trace` is the one a script wants: it names the file, so a caller reads
the trace back without globbing a directory. Naming a file is a request to
record, so it outranks `CODER_TRACE=off`, and a file that cannot be opened
ends the run rather than producing an unrecorded session. The flag works
in both modes — see [`headless.md`](headless.md).

The terminal says which of those it is on the first detail line of a
session, so you never have to guess. Press `⌥V` or type `/verbose` to see
it.

## What is recorded

| Step | When |
| --- | --- |
| `user` | You submit a draft. |
| `system` | The instructions a generation was given, recorded when they change rather than once a turn; and any note about what the host could not do, such as a missing `TYPESAFE_API_KEY`. |
| `agent` | Every reply the model produced, with the turn's token counts, wall time, and the model that produced it. A plan is a reply, and it is recorded verbatim. |
| `agent` with a `shell` call | Every command, its working directory, its whole output, its exit status, and how long it took. |
| `agent` with a decision call | Every `classify` and every `shell_judge`: which door answered, the state and questions that went out, the typed answers that came back, the digest of that state, and the route the table made of it. |

A decision call carries `schema: openagents.decision-call.v1` in its
`extra`, so a reader that wants questions put to a door and not commands
run on a machine separates them on one field. That is the point of using
ATIF rather than a format written here: a Jev, Kev, or Lev call is not a
foreign object in it.

When a generation uses repository context, its instruction step also retains
`repository_context` with schema `openagents.repository-context.v1`. The host
collects this metadata together with the prompt: source paths and line references,
content and excerpt digests, search terms, and coverage diagnostics all describe
the same observation. The recorder does not reread files or interpret quoted
source text as metadata. See [repository source references](repository-evidence.md)
for bounds, digest encoding, and the remaining coverage limits.

### The step names the model, when the session header cannot

A session header carries the model the door serves, and it is written when
the session opens. A door that forwards the turn to a worker somewhere
else does not know the model then — the worker picks it and names it in
the answer — so the header records `unknown` and each answer step carries
`model_name` for what actually produced it. A session that reached two
workers records two models rather than one wrong one.

Own-key doors are unchanged: the header names the model and the steps
inherit it.

Either way the trace names the model rather than the lane, because a model
name is door identity and `docs/gym/regression.md` refuses a comparison
when door identity moves. A run recorded on the `glm` lane says
`zai/glm-5.3-flash`, and nothing reading it has to know what a lane is.
`docs/coder/headless.md` lists the lanes.

## Three decisions worth stating

### A session is one terminal invocation

Start to exit. `coder` does not resume a conversation across processes —
the transcript is the process's — so tying a trace's identity to anything
longer would claim a continuity the agent does not have. Run `coder`
twice and you get two traces.

### Steps are appended; the document is rendered on read

A session appends one complete JSON line per step, flushed and synced
before the call returns. It does not rewrite a document.

Rewriting costs bytes quadratic in the session's length, and — the part
that matters — a process killed partway through a rewrite leaves a
truncated file that parses as nothing at all. The record would be lost
exactly when there is most reason to want it. Appending loses at most the
line that was in flight, and the reader skips an unreadable line rather
than refusing the file.

The ATIF document is computed from the lines on read. There is only one
copy of the truth, so the log and the document cannot disagree.

A log with no closing record is a session that was interrupted, and the
document says `interrupted` rather than pretending the session finished.
`crates/coder/tests/trace.rs` kills a session outright and reads back
what survived.

There are two readers. `atif::log::read` recovers: it splits the file into
lines as bytes before decoding any of them, so a final record torn inside a
multibyte character costs that record and nothing before it, and every line
that did not read is a `Fault` on the recording with its line number and
kind (`torn`, `not_utf8`, `not_json`, `unknown_record`, `bad_step`,
`repeated_session`, `repeated_end`, `after_end`, and so on). The document
carries them under `extra.faults`. `atif::log::read_whole` is the reader
for evidence: it refuses a log with any fault or with no end record, so a
recovered prefix cannot pass as a complete session. CoderBench marks a
trace with faults or without an ending unverifiable, never a pass.

The lifecycle a log holds to is one `session` record first, then steps,
then at most one `end` record last. The writer refuses to append a step
after `finish`, and the reader treats a record after the end, a second
header, or a second ending as a fault while the first stands.

### Nothing in the trace is capped

`crates/coder/src/shell.rs` keeps at most `OUTPUT_MAX` (16 KiB) of a
command's output, and shows the judge and the model's next turn only
`HEAD_MAX` (2,048 bytes) of that.
[`docs/gym/terminal-bench.md`](../gym/terminal-bench.md) argues the 2,048
bound is already too tight. The trace does not inherit it.

Capping output is what makes a trace useless for the analysis it exists
for: a record of a command whose output was cut cannot answer whether the
agent had what it needed. So the call records the whole output the
process held, and puts the bounds beside it:

```json
{
  "schema": "openagents.shell-call.v1",
  "status": "exit 0",
  "exit_code": 0,
  "output_bytes": 9481,
  "shown_bytes": 2048,
  "capture_bytes_max": 16384
}
```

`shown_bytes` is what the agent saw; `output_bytes` is what the machine
said. The difference is a measurable quantity, and it is one worth being
able to ask about.

The remaining ceiling is the shell's own 16 KiB capture, which is
upstream of the trace and belongs to a separate question.

## Reading one

The file is JSON Lines, so the usual tools work:

```sh
ls -t ~/.openagents/traces/
head -1 ~/.openagents/traces/20260919T142233Z-4f1a9c02.atif.jsonl
```

In Rust, `atif::log::read` returns a `Recording`, and
`Recording::document` renders the ATIF document:

```rust
let recording = atif::log::read(path)?;
println!("{}", serde_json::to_string_pretty(&recording.document())?);
```

The document reports `schema_version: ATIF-v1.7`, the session and its
door, the numbered steps, and `final_metrics` — token totals, tool and
decision call counts, reads and searches, the largest tool result, wall
seconds, and the repeated-work table.

## What this is not

- There is no upload, no service, and no trace endpoint. Local files only.
- There is no redaction machinery. These are your own conversations on
  your own machine. What the trace does carry is enough structure for a
  later reader to filter: tool calls, their arguments, and their working
  directory stay distinguishable rather than flattened into prose.
- Harvesting traces into a Gym suite is a separate piece of work, and
  deliberately so — #9379 dropped 151 states whose commands reached into
  private sibling checkouts, and that filtering question deserves its own
  answer.

## Related

- [`docs/gym.md`](../gym.md) — the wider port this is the first piece of.
- [`docs/coder/shell-loop.md`](shell-loop.md) — the loop whose commands
  and judgments a trace records.
- [`docs/coder/headless.md`](headless.md) — `coder -p`, which produces a
  trace from a script rather than from a person at a keyboard.
- [`docs/coder/relay-transport.md`](relay-transport.md) — what the
  session header's `door` field is for, read across two transports.

## Proposed working-state references

The [TypeSafe-native analysis](typesafe-agent-analysis.md) proposes an
evidence store, task frames, and per-recipient context manifests. These are
not implemented by the existing trace format. A trace explains an observed
sequence; the working store retrieves still-valid evidence; the durable
controller decides which effects may resume.

Extend trace metadata additively with evidence item, snapshot, task-frame,
context-manifest, and derived-source references when those contracts land.
Record selection/omission reasons and decision identities without treating
a selected excerpt as the complete captured source. Process capture limits
still apply before tracing; a trace cannot recover output the collector
already discarded.

Keep evidence retention and export explicit. A summary can contain private
source information even when its original artifact is omitted, and a digest
alone does not anonymize predictable content. Deletion should invalidate or
mark unavailable the affected derived references. Offline inspection must
not rerun tools, generation, or decisions; a new comparison is a separately
identified execution with its own budget.
