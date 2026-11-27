# 18. On the record

> **Status: partly implemented.** The `#authority` hash-chained durable `AJR1`
> run journal ([`coax/lib/agent/journal.coax`](../../coax/lib/agent/journal.coax),
> [`journal-flow.coax`](../../coax/lib/agent/journal-flow.coax),
> [`journal-file.coax`](../../coax/lib/agent/journal-file.coax)) and transcript
> DAG ([`openagents/core/transcript_dag.coax`](../../openagents/core/transcript_dag.coax))
> run on `cpu`, grounded by
> [`openagents/examples/durable-run-journal.coax`](../examples/durable-run-journal.coax)
> and [`openagents/examples/durable-run-journal.out`](../examples/durable-run-journal.out),
> and are consumed by [`coax/lib/gateway/plan.coax`](../../coax/lib/gateway/plan.coax)
> and [`coax/lib/inky/run.coax`](../../coax/lib/inky/run.coax).
> Remaining: end-to-end byte-identical gateway request rebuild across a full
> multi-member turn and the unified in-app trajectory viewer across surfaces.

## Premise

The agent now decides, plans and acts with permission. This chapter writes all
of it down: every decision, plan, grant and member call goes into a record you
can read and replay.

## Draft

[Episode 33](../../docs/transcripts/033.md) defines an open agent as one you
can inspect: you can see each run, each step's input and output, and what it
cost. That definition is older than any code in the repository, and this
chapter keeps it.

The rule comes from the Coder book: if the model saw it, it is in the log.
Here it widens. The log holds the message, the typed decision and its
probabilities, the plan, each grant and approval, each member call and its
result, and the cost of each step.

There are two kinds of record, and they do different jobs. A run journal is
the authority on what happened: what was admitted, what was dispatched and how
it ended. A trajectory is the readable trace of the steps, including the steps
of any member that is itself an agent. Journals decide. Trajectories let you
look.

A record is yours. It is private unless you publish it. [Episode
228](../../docs/transcripts/228.md) proposes public trajectories for public
work, and [Episode 215](../../docs/transcripts/215.md) proposes selling
redacted traces. Both are choices a person makes, and chapter 23 covers the
consent rules.

The evidence chapters (43 to 52) depend on this one. You can't measure whether a member helped
unless both runs, with it and without it, are on the record in the same form.

## What we build

A plan for what this chapter ships once the draft is approved:

- The run journal for the general agent: decisions, plans, grants, calls,
  results and costs.
- Trajectories in the shared trace format, linking a parent step to a member's
  own trace.
- A viewer on each surface, and a replay check that rebuilds every request
  from the journal.

It builds on what exists today:

- [`openagents/core/transcript_dag.coax`](../../openagents/core/transcript_dag.coax)
- [`openagents/core/session.coax`](../../openagents/core/session.coax)
- [`coax/lib/agent/journal.coax`](../../coax/lib/agent/journal.coax)
- [`coax/lib/agent/journal-flow.coax`](../../coax/lib/agent/journal-flow.coax)
- [`coax/lib/nostr/run`](../../coax/lib/nostr/run)
- [`coax/lib/nostr/atif`](../../coax/lib/nostr/atif) (including ATIF-v1.8 friction telemetry)

On the protocol ([chapter 65](ch065-one-schema-three-carriages.md)):

| Record | Schema | Carriage | Today |
| --- | --- | --- | --- |
| Run journal | `openagents.run.v1` `RunRecord` in a `RunRecordEnvelope`, with a `RunHead` ([spec 22.1](../../docs/protocol/openagents-protocol-spec.md#221-openagentsrunv1)) | Nostr kinds 3187 and 30186, encrypted (NIP-RUN) | Spec sketch; no `.proto` file. `coax/lib/nostr/run` handles the NIP's JSON. The local journal is a hash-chained binary file. It stays hand-packed, and a fixture to pin it is planned |
| Trajectory | `openagents.trace.v1` `Manifest`, `Chunk` ([spec 21.4](../../docs/protocol/openagents-protocol-spec.md#214-openagentstracev1)) | Nostr kinds 3198 and 3199 (NIP-ATIF) | Spec sketch; no `.proto` file. `coax/lib/nostr/atif` handles the NIP's JSON |

### The chapter's check

- **Source:** [`coax/lib/agent/journal.coax`](../../coax/lib/agent/journal.coax),
  [`coax/lib/agent/journal-flow.coax`](../../coax/lib/agent/journal-flow.coax),
  [`coax/lib/agent/journal-file.coax`](../../coax/lib/agent/journal-file.coax),
  [`openagents/examples/durable-run-journal.coax`](../examples/durable-run-journal.coax),
  and [`openagents/examples/durable-run-journal.out`](../examples/durable-run-journal.out)
  (remaining: full multi-member turn gateway request replay and surface viewer).
- **Trace:** prepares and commits `AJR1` frames in order (`command_intent`,
  `decision`, and binary `tool_output` containing a NUL byte), matching the
  independent Python `hashlib` genesis SHA-256 prefix `67a58013fb5ada87`, then
  replays all three chained frames from genesis (`seq=3`, `head=11e2162bb682e54f`,
  `bin_exact=true`).
- **Refusal:** an unacknowledged append (`durable=false`) poisons the writer
  (`failed=true`) and refuses further `aj_prepare` calls; a 1-byte header or
  payload flip, truncated tail, out-of-order frame, or overlapping slice each
  fail closed (`ok=false`).
- **Independent check:** `coax/bin/coax run openagents/examples/durable-run-journal.coax`
  and `--release` match [`openagents/examples/durable-run-journal.out`](../examples/durable-run-journal.out),
  frame 0's hash matches the independent Python `hashlib` SHA-256 vector, and
  `#authority` `#test`/`#forall` checks in [`coax/lib/agent/journal.coax`](../../coax/lib/agent/journal.coax)
  verify every framing and admission boundary.

Planned paths: none. The record extends
[`coax/lib/agent/journal.coax`](../../coax/lib/agent/journal.coax) and the
session record in [`coax/lib/agent/session/`](../../coax/lib/agent/session);
there is no second journal.

## Sources

- **Episodes:**
  - [Episode 33, *Agent Inspectability Planning*](../../docs/transcripts/033.md): open means inspectable
  - [Episode 34, *Agent Inspection UX Design*](../../docs/transcripts/034.md): inspection design
  - [Episode 35, *Agent Inspection UI*](../../docs/transcripts/035.md): inspection UI
  - [Episode 162, *Reasoning Stream*](../../docs/transcripts/162.md): a reasoning stream
  - [Episode 228, *Free Autopilot*](../../docs/transcripts/228.md): public trajectories
  - [Episode 249, *Subagent UI Design*](../../docs/transcripts/249.md): inspectable child histories
- **Old code at `coder-v1.0.0`, crates:** `atif`, `atif-nostr`, `hook-log`, `coder-history`, `receipts`
- **NIPs:** [NIP-RUN](../../nostr/nips/openagents/NIP-RUN.md), [NIP-ATIF](../../nostr/nips/openagents/NIP-ATIF.md), [NIP-SESS](../../nostr/nips/openagents/NIP-SESS.md)
- **Also:**
  - [Coder book, chapter 2: Model-visible means logged](../../coder/book/ch002-model-visible-means-logged.md)
- **Promises covered** ([audit](../../docs/audits/2026-11-10-episode-289-port-audit.md#1-what-episode-289-promised)): P4, P12

## Open questions for the owner

1. Confirm the default: records are private, and publishing one is a separate,
   explicit act.

## Source listing

Run the Chapter 18 example in [`openagents/examples/durable-run-journal.coax`](../examples/durable-run-journal.coax) (committed transcript [`openagents/examples/durable-run-journal.out`](../examples/durable-run-journal.out)):

<!-- source: openagents/examples/durable-run-journal.coax durable-run-journal -->
```jai
drj_run :: () -> bool {
  s0: AjState;
  f0: [256] u8; f1: [256] u8; f2: [256] u8;

  // 1. Frame 0: command_intent "echo hi" checked against independent Python hashlib vector.
  p0 := aj_prepare(s0, AjKind.command_intent, 0, "echo hi", f0[..], false);
  s1 := aj_commit(s0, f0[0..<p0.count], true);
  py_vec := [32] u8.[0x67, 0xa5, 0x80, 0x13, 0xfb, 0x5a, 0xda, 0x87, 0x62, 0x26, 0x61, 0xd7, 0x68, 0x32, 0xab, 0x83,
                     0x78, 0xcc, 0xad, 0xd2, 0xa9, 0xd5, 0xba, 0xea, 0xb1, 0xce, 0xe5, 0xb6, 0x25, 0x51, 0xee, 0x64];
  vec_match := aj_same(s1.head, py_vec);
  h0_buf: [16] u8;
  print("frame0 ok=% bytes=% seq=% sha256=% py_vec=%\n",
    p0.ok, p0.count, s1.sequence, drj_hex8(s1.head, h0_buf[..]), vec_match);

  // 2. Frame 1 (decision) + Frame 2 (binary tool_output with NUL byte), then full replay from genesis.
  p1 := aj_prepare(s1, AjKind.decision, 0, "pack=coder,tier=local", f1[..], false);
  s2 := aj_commit(s1, f1[0..<p1.count], true);
  bin := [4] u8.[0, 255, 10, 34];
  p2 := aj_prepare(s2, AjKind.tool_output, 1, cast(string) bin[..], f2[..], false);
  s3 := aj_commit(s2, f2[0..<p2.count], true);

  r0 := aj_read(s0, f0[0..<p0.count]);
  r1 := aj_read(r0.after, f1[0..<p1.count]);
  r2 := aj_read(r1.after, f2[0..<p2.count]);
  bin_exact := r2.ok && r2.count == 4 && f2[r2.at] == 0 && f2[r2.at + 1] == 255 && f2[r2.at + 2] == 10 && f2[r2.at + 3] == 34;
  replay_ok := r0.ok && r1.ok && r2.ok && aj_same(r2.after.head, s3.head) && bin_exact;
  h2_buf: [16] u8;
  print("replay seq=% head=% kinds=%,%,% bin_exact=%\n",
    r2.after.sequence, drj_hex8(r2.after.head, h2_buf[..]),
    cast(int) r0.kind, cast(int) r1.kind, cast(int) r2.kind, bin_exact);

  // 3. Fail-closed refusals: failed fsync poisons writer, 1-byte tamper, truncation, out-of-order, overlap.
  poisoned := aj_commit(s0, f0[0..<p0.count], false);
  after_poison := aj_prepare(poisoned, AjKind.context, 0, "after", f0[..], false);

  tampered := f1;
  tampered[AJ_HEADER] ^= 1;
  rd_tamper := aj_read(s1, tampered[0..<p1.count]);
  rd_trunc := aj_read(s1, f1[0..<p1.count - 1]);
  rd_order := aj_read(s0, f1[0..<p1.count]);
  ov_buf: [128] u8;
  p_overlap := aj_prepare(s0, AjKind.context, 0, "alias", ov_buf[..], true);
  print("refuse poison=% tamper=% trunc=% order=% overlap=%\n",
    poisoned.failed && !after_poison.ok, !rd_tamper.ok, !rd_trunc.ok, !rd_order.ok, !p_overlap.ok);

  return vec_match && replay_ok &&
         poisoned.failed && !after_poison.ok &&
         !rd_tamper.ok && !rd_trunc.ok && !rd_order.ok && !p_overlap.ok;
}
```

Output (`coax/bin/coax run openagents/examples/durable-run-journal.coax`, identical in `--release`):

```text
frame0 ok=true bytes=96 seq=1 sha256=67a58013fb5ada87 py_vec=true
replay seq=3 head=11e2162bb682e54f kinds=5,10,7 bin_exact=true
refuse poison=true tamper=true trunc=true order=true overlap=true
ok=true
```
