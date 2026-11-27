# 2. Model-visible means logged

Anything the model sees must be recorded in an append-only log first, and every
intent with an external effect must be acknowledged durably before execution.
If a session crashes mid-turn, forks, or rewinds, the runtime reconstructs the
exact history from the log instead of trusting volatile memory.

OpenAgents implements this contract across two complementary `#authority`
modules:

1. **Append-only conversation transcript DAG**
   ([`openagents/core/transcript_dag.coax`](../../openagents/core/transcript_dag.coax)):
   stores main-chain turns, isolates sidechain subagent entries so they never
   corrupt `head_uuid`, supports `/rewind` branching and snip relinking,
   severs history cleanly at `FLAWED_SYS_COMPACT_BOUNDARY`, and recovers valid
   JSONL prefixes after truncated writes.
2. **Durable AJR1 hash-chained journal**
   ([`coax/lib/agent/journal.coax`](../../coax/lib/agent/journal.coax), wired to
   disk in [`coax/lib/agent/journal-file.coax`](../../coax/lib/agent/journal-file.coax)
   and [`coax/lib/agent/journal-flow.coax`](../../coax/lib/agent/journal-flow.coax)):
   prepares exact binary frames (`AJR1` magic + sequence + group + kind +
   length + previous SHA-256 + payload + frame SHA-256). Only an acknowledged
   `aj_commit(..., true)` advances the chain head; an unacknowledged write
   poisons `AjState.failed = true` so no subsequent tool effect can mistake a
   partial write for committed intent.

## Profile

This chapter's profile is `openagents-transcript-ajr1-v1, bounded CPU; coder-owned-v1, native Linux x86_64`:

- **Sidechain isolation:** appending a sidechain message (`is_sidechain = true`)
  records it in the DAG without mutating `head_uuid` or appearing in
  `flawed_dag_has_main_uuid`.
- **Snip & compact boundary fidelity:** `flawed_dag_mark_snipped` marks pruned
  nodes without rewriting earlier records, and `FLAWED_SYS_COMPACT_BOUNDARY`
  anchors post-compaction reads cleanly.
- **Two-phase durable journal (`aj_prepare` -> `aj_commit`):** a failed sync
  returns a poisoned state (`failed = true`, `sequence` unchanged) that refuses
  further `aj_prepare` calls; a committed frame chains its SHA-256 digest into
  the next frame so flipping any byte or replaying out of sequence is refused
  by `aj_read`.

## Worked example

The runnable example [transcript-log.coax](../examples/transcript-log.coax)
exercises both `openagents/core/transcript_dag.coax` and
`coax/lib/agent/journal.coax`: main-chain vs. sidechain appends, snipping,
compact boundaries, sync-failure poisoning, single-bit tamper rejection, and
out-of-order replay refusal.

<!-- source: coder/examples/transcript-log.coax example -->
```jai
main :: () {
  // Part A: Append-only transcript DAG with sidechain isolation, snipping, and compact boundary
  dag: [1] FlawedTranscriptDag;
  flawed_dag_init(dag[..], 42);
  _ = flawed_dag_append(dag[..], 10, -1, FLAWED_ROLE_USER, FLAWED_SYS_NONE, false, 100);
  _ = flawed_dag_append(dag[..], 11, -1, FLAWED_ROLE_ASSISTANT, FLAWED_SYS_NONE, false, 200);
  // Sidechain subagent message does NOT advance main-chain last_uuid
  _ = flawed_dag_append(dag[..], 90, 11, FLAWED_ROLE_USER, FLAWED_SYS_NONE, true, 250);
  _ = flawed_dag_append(dag[..], 12, -1, FLAWED_ROLE_USER, FLAWED_SYS_NONE, false, 300);
  snipped := flawed_dag_mark_snipped(dag[..], 11);
  _ = flawed_dag_append(dag[..], 13, -1, FLAWED_ROLE_SYSTEM, FLAWED_SYS_COMPACT_BOUNDARY, false, 400);
  _ = flawed_dag_append(dag[..], 14, -1, FLAWED_ROLE_ASSISTANT, FLAWED_SYS_NONE, false, 500);
  print("dag: session=% entries=% main_head=% sidechain_in_main=% snipped_11=%\n",
    dag[0].session_id, dag[0].count, dag[0].head_uuid, flawed_dag_has_main_uuid(dag[..], 90), snipped);
  print("dag lookup: uuid_10=% uuid_13_boundary=% uuid_14=%\n",
    flawed_dag_find_main(dag[..], 10), flawed_dag_find_main(dag[..], 13), flawed_dag_find_main(dag[..], 14));

  // Part B: Durable AJR1 hash-chained journal frames — intent committed only after sync
  state: AjState;
  f0: [256] u8;
  p0 := aj_prepare(state, AjKind.command_intent, 0, "coax test openagents/core/query_loop.coax", f0[..], false);
  failed := aj_commit(state, f0[0..<p0.count], false);
  print("ajr1 sync-fail: poisoned=% seq=% retry_prepare_ok=%\n",
    failed.failed, failed.sequence, aj_prepare(failed, AjKind.context, 0, "", f0[..], false).ok);

  s1 := aj_commit(state, f0[0..<p0.count], true);
  r0 := aj_read(state, f0[0..<p0.count]);
  hb: [16] u8;
  print("ajr1 frame 0: ok=% bytes=% seq=% head=%\n", r0.ok, p0.count, s1.sequence, hex8(s1.head, hb[..]));

  // Tampering a single payload byte is refused immediately by aj_read
  tampered := f0;
  tampered[AJ_HEADER] ^= 1;
  r_bad := aj_read(state, tampered[0..<p0.count]);
  print("ajr1 tamper check: read_ok=%\n", r_bad.ok);

  // Chained frame 1 links to frame 0's digest
  f1: [256] u8;
  p1 := aj_prepare(s1, AjKind.model_reply, 1, "all tests passed", f1[..], false);
  s2 := aj_commit(s1, f1[0..<p1.count], true);
  r1 := aj_read(s1, f1[0..<p1.count]);
  print("ajr1 frame 1: ok=% seq=% replay_out_of_order_ok=% head=%\n",
    r1.ok, s2.sequence, aj_read(state, f1[0..<p1.count]).ok, hex8(s2.head, hb[..]));
}
```

Run it in debug or release:

```shell
coax/bin/coax run coder/examples/transcript-log.coax
```

```text
dag: session=42 entries=6 main_head=14 sidechain_in_main=false snipped_11=true
dag lookup: uuid_10=0 uuid_13_boundary=4 uuid_14=5
ajr1 sync-fail: poisoned=true seq=0 retry_prepare_ok=false
ajr1 frame 0: ok=true bytes=130 seq=1 head=3d9589c59d217a7a
ajr1 tamper check: read_ok=false
ajr1 frame 1: ok=true seq=2 replay_out_of_order_ok=false head=5326063954cbb326
```

The checked fixture is [transcript-log.out](../examples/transcript-log.out),
matching byte-for-byte in debug and release.

## Checks and production adoption

- `#authority` checks in
  [`openagents/core/transcript_dag.coax`](../../openagents/core/transcript_dag.coax)
  and [`coax/lib/agent/journal.coax`](../../coax/lib/agent/journal.coax).
- Adopted in [`openagents/runtime/oa-worker.coax`](../../openagents/runtime/oa-worker.coax) and
  [`openagents/daemon/host.coax`](../../openagents/daemon/host.coax), with native Linux host
  verification in [`coax/lib/agent/native-profile.coax`](../../coax/lib/agent/native-profile.coax)
  and [`coder/tests/journal-worker.coax`](../../coder/tests/journal-worker.coax).

## Done means

- [x] Transcript DAG sidechain isolation, snipping, and compact boundaries pass
  under `#authority`.
- [x] AJR1 durable frames refuse sync-failure reuse, byte tampering, and
  out-of-order replay.
- [x] `coder/examples/transcript-log.coax` matches
  `coder/examples/transcript-log.out` in debug and release.
