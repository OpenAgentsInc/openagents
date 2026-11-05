# The owned Coder loop, version 1

Microcoder and the Psionic engine use the same native worker in
[`mcworker.coax`](../../coder/tui/mcworker.coax). Its model loop, request
journal, shell/file tools, approvals and selected workspace confinement run
together. The first five book chapters also retain their small CPU
experiments; this profile supplies their concrete native integration.

## Requests and effects

[`GwPlan`](../../coax/lib/gateway/plan.coax) translates each actual attempt
before sending it. It writes the complete translated body, configured
gateway/model/wire and an endpoint digest to the journal, then obtains a
one-use start ticket. Authentication headers and credential fields stay in
the transport. A failed append starts no transport. Reply text is durable
before the loop sees it. A stopped transport retains an unknown remote
outcome.

[`McRun`](../../coax/lib/microcoder/run.coax) admits the typed `commands` and
`view` actions parsed from the model's schema. It records a command or view
intent before executing, and records its model-visible output and result
before adding them to state. Each gated proposal has a worker-wide monotonic
ticket; an answer for another proposal does nothing. Proposals and answers
are journal records. Approval does not change a selected sandbox.

The system prompt and action schema are constants in the worker and are
recorded in the turn's context. Every translated body captures the result
of state pruning. Consecutive requests for the same configured attempt also
record their common byte-prefix length. This measures request bytes; it
does not assert a provider's tokenization, cache reuse or resolved model
identity. The book's finite prefix experiment separately checks token
boundaries against its own tokenizer and cache.

## Private durable records

The worker creates one exclusive `0600` file at
`~/.openagents/coder/sessions/worker-TIMESTAMP.ajr`; `CODER_HOME` overrides
the parent of `.openagents`. Its path is recorded in Coder's JSONL session,
and each turn context includes the Coder session ID. These are local,
private request records, including source text and model-visible tool output.

[`journal.coax`](../../coax/lib/agent/journal.coax) specifies AJR1 frames:

| Offset | Field |
| --- | --- |
| 0 | Four magic bytes `AJR1` |
| 4 | Sequence, little-endian u64 |
| 12 | Operation group, little-endian u64 |
| 20 | Kind, u8 |
| 21 | Payload length, little-endian u32 |
| 25 | Previous frame's SHA-256 digest, 32 bytes |
| 57 | Exact payload bytes, at most 32,768 |
| After payload | SHA-256 of header and payload, 32 bytes |

Kind values are context 0, request chunk 1, model intent 2, model reply 3,
model completion 4, command intent 5, view intent 6, tool output 7, tool
completion 8, stop 9 and approval decision 10. Requests use ordered chunks
and a final intent, with a maximum body of 262,144 bytes.

[`journal-flow.coax`](../../coax/lib/agent/journal-flow.coax) verifies record
order and operation ownership. Recovery uses `af_recover`; recovered state
has no execution capability. An incomplete intent has an unknown outcome
and is never automatically retried. A cut or corrupt final frame is not a
successful event. A retained hash chain establishes internal consistency;
replacing every record and trusted head can rewrite it consistently.

The native file adapter keeps a separate concrete descriptor, syncs before
acknowledgement, and rolls a failed partial append back to its previous
length. Any failure poisons that writer. Coder's JSONL reporter also stops
execution when its durable append fails, before printing that line or
dispatching another turn.

## Selected native confinement

The default remains `danger-full-access`. Selected policy/asking uses
`workspace-write`, or `read-only` below the write ceiling. These workspace
profiles currently require Linux x86-64, bubblewrap and `prlimit`; an
unavailable or unknown profile refuses execution without falling back.

The policy in [`native-profile.coax`](../../coax/lib/agent/native-profile.coax)
and setup in [`tools.coax`](../../coax/lib/microcoder/tools.coax) mount only
the canonical workspace, public runtime directories and isolated temporary,
device and proc filesystems. A root containing the real home or Coder state
home is refused. The environment is cleared. Separate PID, user and network
namespaces and a seccomp filter prevent IP/Unix socket creation, alternate
x32 syscall entry, process inspection and the named escape interfaces.
Read-only uses a read-only workspace mount.

Each command has at most 30 seconds, 30 CPU seconds per process, 2 GiB of
address space per process, a 16 MiB file-size bound and 128 descriptors.
The owned turn admits at most 64 model steps and 128 tool effects within a
five-minute admission budget. Streaming-model polling observes the deadline;
commands use the remaining whole-second budget. Process-group cleanup and
bubblewrap's parent-death handling stop descendants on timeout. This is not
a cgroup-wide memory or descendant-count quota.

The view tool reads at most 64 KiB within the workspace and runs through the
same selected confinement. Its process wait remains separately bounded.
Claude Code and Codex continue to own their internal tools and model history;
the owned-worker journal does not qualify those internals.

## Executable evidence

Run from the repository root:

```sh
coax/web/tool.sh coder/tests/journal-file.coax
coax/web/tool.sh coder/tests/journal-session.coax
coax/web/tool.sh coder/tests/journal-live.coax
coax/web/tool.sh coder/tests/journal-worker.coax
coax/web/tool.sh coder/tests/native-profile.coax
```

The storage fixture uses ASan/UBSan and forces a partial physical write.
The live fixture reconstructs actual translated requests byte for byte,
runs a shell write and file view, checks cancellation and prevents a new
transport after storage ownership is lost. The process fixture drives the
actual worker, refuses a stale approval, verifies its on-disk journal and
forces a JSONL failure before a task that would otherwise write a file.
The workspace fixture observes writes, read-only refusal, escaping symlinks,
IP and Unix socket refusal, cleared environment and timeout cleanup.

The worker freezes the enabled policy’s effect ceiling at turn admission and records the full declared policy in the context record. A matching approval ticket can approve a proposal within that ceiling; it cannot widen the ceiling. Tasks larger than 16,384 bytes and missing or oversized workspace paths are refused before a request starts.
