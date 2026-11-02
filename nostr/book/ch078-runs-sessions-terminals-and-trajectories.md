# 78. Runs, sessions, terminals and trajectories

An agent's work leaves four kinds of record. NIP-RUN keeps the run's
journal: the authoritative, hash-chained record of what the controller
admitted, dispatched and observed, and who the controller is. NIP-SESS
describes how a client attaches to an engine session and queues turns.
NIP-TERM carries a live terminal's output in order, with gaps where the
host discarded some. NIP-ATIF carries an agent's trajectory, the full
record of its steps, as a manifest and ordered chunks. This chapter builds
the parts that decide or verify: the run journal's digest and replay with
controller handoff, the trajectory chunk check, and the terminal client's
frame order.

Part VI, Every NIP. Result: NIP-RUN record digests, journal replay that
refuses gaps, forks, broken chains, forged digests, stale or unadmitted
controllers and forbidden transitions, controller handoff with fencing, and
the head-hint check; NIP-ATIF chunk verification that reports each missing
chunk; NIP-TERM's client frame order with duplicates, gaps and lost frames.
The record content is sealed with NIP-44 in the example. NIP-SESS, RUN
retention and the event kinds are described and not built; see
[What is not built](#what-is-not-built).

## What this chapter teaches

- A RUN record's identity: SHA-256 of the schema line and the record's JCS
  form, and why the same record keeps one Digest when it is encrypted
  separately to each recipient.
- How replay orders records by sequence and chain, never by `time` or
  arrival, and refuses at the first gap, fork or broken link.
- Controller handoff: the old controller's `handoff` record names the next
  controller and the dispatchers' fencing acknowledgment, and only the new
  controller at the next generation may append after it.
- The per-attempt state machine: `dispatched`, `observed`, `resolved`,
  `unknown` and `reconciled`, and why an unobserved dispatch is unknown,
  not failed.
- Why a head hint can be stale or ahead but can never erase history.
- ATIF chunks: every chunk checked against the manifest, missing chunks
  reported by index, and the whole checked against the artifact digest.
- TERM frames: applied in sequence, duplicates ignored, gaps shown as
  gaps, and a lost frame stops the client until it reattaches.

## Where the code is

| File | Procedure | What it does |
| --- | --- | --- |
| [journal.coax](../../coax/lib/nostr/run/journal.coax) | `run_record_write(r, out)` | Writes a record's JSON in NIP-RUN field order |
| [journal.coax](../../coax/lib/nostr/run/journal.coax) | `run_digest(r, work, out)` | The record's `sha256:` Digest |
| [journal.coax](../../coax/lib/nostr/run/journal.coax) | `run_replay(records, first, subjects, work)` | Replays a journal from any arrival order to its head, or to the first fault |
| [journal.coax](../../coax/lib/nostr/run/journal.coax) | `run_head_check(h, seq, digest, generation)` | Compares a `30186` head hint with the replayed head |
| [chunks.coax](../../coax/lib/nostr/atif/chunks.coax) | `atif_check(m, entries, chunks, missing)` | Checks a trajectory's chunks against its manifest and artifact |
| [frames.coax](../../coax/lib/nostr/term/frames.coax) | `term_apply(c, f)` | Applies one terminal frame to a client's state |
| [frames.coax](../../coax/lib/nostr/term/frames.coax) | `term_order(frames)` | Sorts retained frames into the order they apply in |

All three files are new with this chapter. `journal.coax` decides which
records are the run's authoritative history and who may append to it, so
it is an `#authority` file: its checks came first, and a mutation run kills
every mutant. `chunks.coax` and `frames.coax` verify observational records
and decide no authority; they also have checks first and a clean mutation
run. `journal.coax` reuses `jcs.coax` for the canonical form and
`envelope.coax`'s `ae_digest` for the Digest; `chunks.coax` reuses
`ae_digest` and the SHA-256 state functions. The example seals a record
with `nip44.coax` from [chapter 20](ch020-nip-44-encrypted-payloads.md).

## Four records, four owners

The four NIPs record different things, and each refuses to stand in for
the others:

| NIP | What it records | Authority |
| --- | --- | --- |
| RUN | The controller's durable journal: admission, dispatch, observation, resolution, handoff, settlement | Decides effects and recovery. Signed by the current controller |
| SESS | A session's attachment, effective configuration, queued turns and commands | The session authority's own record |
| TERM | A host terminal's output frames, in sequence | The host. Output is private and never public |
| ATIF | An agent's trajectory: what a recorder saw, step by step | None. A signature proves who published it, not that it is complete or true |

NIP-ATIF is explicit: "RUN records decide effects and recovery, EVAL
decides whether a result passed, and POL decides what may be disclosed." A
trajectory can be cited as RUN evidence, but it cannot resume, fence or
prove that an effect happened. The same holds for a terminal's output.

## The run journal

A run's records travel as kind `3187` events, each NIP-44 encrypted to one
recipient, with a random 64-hex mailbox `h` tag and `t: oa:run:v1`. Kind
`30186` is an encrypted hint at the current head. The decrypted content is
`{v: "openagents.run-record.v1", requires: [], record, digest}`, and the
record holds `run`, `seq`, `previous`, `controller`, `generation`, `type`,
`subject`, `time` and `data`. The record in `journal.coax`:

<!-- source: coax/lib/nostr/run/journal.coax record -->
```jai
// One record. previous "" is null; step "" and iteration -1 are null;
// attempt 0 is null (run-level). For a handoff, data is written from next
// and fence; for the others it is the caller's JSON object. signer is the
// verified event signer and digest the carried Digest; neither is hashed.
RunRecord :: struct {
  run:string;seq:int;previous:string;controller:string;generation:int;type:int;
  step:string;iteration:int;attempt:int;time:int;data:string;next:string;fence:string;
  signer:string;digest:string;
}
RunSubject :: struct {step:string;iteration:int;attempt:int;state:int;}
RunHead :: struct {error:u32;at:int;seq:int;digest:[71]u8;controller:string;generation:int;settled:bool;unknowns:int;}
```

`subject` is `{step, iteration, attempt}`; a run-level record has null in
all three. `time` is observational Unix seconds. NIP-RUN says "Sequence and
chain determine order; timestamp order does not establish authority", so
the example dates its records backwards and replay never reads `time`.

The codes replay reports, and the ten record types:

<!-- source: coax/lib/nostr/run/journal.coax codes -->
```jai
RN_OK :: cast(u32)0;
RN_GAP :: cast(u32)1;           // a missing sequence below one that exists: incomplete
RN_CONFLICT :: cast(u32)2;      // two different records at one sequence: a fork
RN_BROKEN :: cast(u32)3;        // previous is not the Digest of the record before
RN_DIGEST :: cast(u32)4;        // the carried Digest is not the record's own
RN_NOT_ADMITTED :: cast(u32)5;  // a signer that is not the controller, or not the current one
RN_STALE :: cast(u32)6;         // an older generation, after a handoff
RN_ORDER :: cast(u32)7;         // a transition the state machine forbids
RN_UNFENCED :: cast(u32)8;      // a handoff without the dispatchers' fencing acknowledgment
RN_CLOSED :: cast(u32)9;        // a record after settled
RN_MALFORMED :: cast(u32)10;    // a bad field: seq, attempt, type, run, the next controller
RN_LIMIT :: cast(u32)11;        // no room for the record's JSON or the subjects

RN_CREATED :: 0; RN_ADMITTED :: 1; RN_DISPATCHED :: 2; RN_OBSERVED :: 3; RN_RESOLVED :: 4;
RN_CANCEL_REQUESTED :: 5; RN_UNKNOWN :: 6; RN_RECONCILED :: 7; RN_HANDOFF :: 8; RN_SETTLED :: 9;
RN_TYPES :: 10;
RN_SCHEMA :: "openagents.run-record.v1";
```

### The record's Digest

The Digest is SHA-256 of the UTF-8 schema identifier
`openagents.run-record.v1`, one LF byte, and the record's JCS form, written
`sha256:` and 64 hex digits. `run_record_write` writes the record in NIP
field order; `run_digest` canonicalizes it and hashes the schema line and
the JCS bytes together:

<!-- source: coax/lib/nostr/run/journal.coax write -->
```jai
// Writes the record's JSON in NIP-RUN field order; -1 when out is too small.
run_record_write :: (r:RunRecord,out:[]u8)->int {
  if r.type<0 || r.type>=RN_TYPES {return -1;}
  at:=json_raw(out,0,"{\"run\":");at=json_write_string(out,at,r.run);
  at=json_raw(out,at,",\"seq\":");at=json_write_int(out,at,r.seq);
  at=json_raw(out,at,",\"previous\":");at=rn_null_or_string(out,at,r.previous);
  at=json_raw(out,at,",\"controller\":");at=json_write_string(out,at,r.controller);
  at=json_raw(out,at,",\"generation\":");at=json_write_int(out,at,r.generation);
  at=json_raw(out,at,",\"type\":");at=json_write_string(out,at,RN_NAMES[r.type]);
  at=json_raw(out,at,",\"subject\":{\"step\":");at=rn_null_or_string(out,at,r.step);
  at=json_raw(out,at,",\"iteration\":");at=rn_null_or_int(out,at,r.iteration,-1);
  at=json_raw(out,at,",\"attempt\":");at=rn_null_or_int(out,at,r.attempt,0);
  at=json_raw(out,at,"},\"time\":");at=json_write_int(out,at,r.time);
  at=json_raw(out,at,",\"data\":");
  if r.type==RN_HANDOFF {
    at=json_raw(out,at,"{\"controller\":");at=json_write_string(out,at,r.next);
    at=json_raw(out,at,",\"generation\":");at=json_write_int(out,at,r.generation+1);
    at=json_raw(out,at,",\"fence\":");at=rn_null_or_string(out,at,r.fence);at=json_raw(out,at,"}");
  } else {at=json_raw(out,at,r.data);}
  return json_raw(out,at,"}");
}
```

<!-- source: coax/lib/nostr/run/journal.coax digest -->
```jai
// The record's Digest into out (71 bytes). work holds the record's JSON,
// then the schema line and the JCS form: twice the JSON plus 25 bytes.
run_digest :: (r:RunRecord,work:[]u8,out:[]u8)->u32 {
  if r.type<0 || r.type>=RN_TYPES {return RN_MALFORMED;}
  n:=run_record_write(r,work);if n==-1 {return RN_LIMIT;}
  hashed:=work[n..<work.count];
  at:=json_raw(hashed,0,RN_SCHEMA);at=json_raw(hashed,at,"\n");if at==-1 {return RN_LIMIT;}
  j:=jcs_canonicalize_checked(cast(string)work[0..<n],hashed[at..<hashed.count],false);
  if j.error==JCS_OUTPUT {return RN_LIMIT;}
  if j.error!=JCS_OK {return RN_MALFORMED;}
  if ae_digest(hashed[0..<at+j.written],out)!=AE_OK {return RN_LIMIT;}
  return RN_OK;
}
```

A `handoff` record's `data` is written from the struct's `next` and
`fence` fields, `{controller, generation, fence}` with the next generation,
so the fields replay checks are the fields the Digest covers. Every other
type's `data` is the caller's JSON object; NIP-RUN's per-type data schemas
are not checked here.

Because the Digest covers the record and not the event, one record sealed
separately to two recipients has two event IDs and two ciphertexts but one
Digest. NIP-RUN asks the reader to deduplicate signed events and logical
records separately.

### Replay

Replay takes the records in any arrival order, duplicates included. It
applies them by sequence, starting at 0, and at each sequence:

1. Recomputes each record's Digest and compares it with the carried one.
2. Refuses two different records at one sequence as a fork (`conflict`).
3. Stops at the first sequence with no record. If a record above it
   exists, that is a gap: the result is incomplete, not finished.
4. Checks `previous`: null at 0, else the Digest of the record before.
5. Checks the signer: the verified event signer must be the record's
   `controller`, and that must be the current controller at the current
   generation. An older generation is `stale`.
6. Applies the state machine.

<!-- source: coax/lib/nostr/run/journal.coax replay -->
```jai
// Replays records (any order, duplicates allowed) from seq 0. first is the
// controller the caller admitted for the created record. subjects is the
// work table; work is digest scratch. The head is the last applied record;
// error and at say where replay stopped.
run_replay :: (records:[]RunRecord,first:string,subjects:[]RunSubject,work:[]u8)->RunHead {
  h:RunHead;h.seq=-1;h.controller=first;
  used:=[1]int.[0];admitted:=[1]bool.[false];unknowns:=[1]int.[0];
  run_id:="";
  for r:records {if r.seq<0 {return rn_fail(h,RN_MALFORMED,r.seq);}}
  // seen counts the records at the sequences applied so far, duplicates
  // included. Replay stops at the first sequence with no record; any record
  // not yet seen is above it, so that sequence is a gap.
  seq:=0;seen:=0;
  while true {
    pick:=-1;mine:[71]u8;
    for i:0..<records.count {
      if records[i].seq!=seq {continue;}
      seen+=1;
      d:[71]u8;e:=run_digest(records[i],work,d[..]);if e!=RN_OK {return rn_fail(h,e,seq);}
      if cast(string)d[..]!=records[i].digest {return rn_fail(h,RN_DIGEST,seq);}
      if pick==-1 {pick=i;mine=d;}
      else if cast(string)d[..]!=cast(string)mine[..] {return rn_fail(h,RN_CONFLICT,seq);}
    }
    if pick==-1 {
      if seen!=records.count {return rn_fail(h,RN_GAP,seq);}
      break;
    }
    if h.settled {return rn_fail(h,RN_CLOSED,seq);}
    r:=records[pick];
    if seq==0 {
      if r.previous.count!=0 {return rn_fail(h,RN_BROKEN,seq);}
      run_id=r.run;
    } else {
      if r.previous!=cast(string)h.digest[..] {return rn_fail(h,RN_BROKEN,seq);}
      if r.run!=run_id {return rn_fail(h,RN_MALFORMED,seq);}
    }
    if r.signer!=r.controller {return rn_fail(h,RN_NOT_ADMITTED,seq);}
    if r.generation<h.generation {return rn_fail(h,RN_STALE,seq);}
    if r.controller!=h.controller || r.generation!=h.generation {return rn_fail(h,RN_NOT_ADMITTED,seq);}
    e:=rn_transition(r,admitted[..],subjects,used[..],unknowns[..]);
    if e!=RN_OK {return rn_fail(h,e,seq);}
    if r.type==RN_HANDOFF {h.controller=r.next;h.generation+=1;}
    if r.type==RN_SETTLED {h.settled=true;}
    h.seq=seq;h.digest=mine;h.unknowns=unknowns[0];
    seq+=1;
  }
  h.at=seq;return h;
}
```

`first` is the controller the caller admitted for the `created` record
under its own policy, or a CJ request signed by the owner. NIP-RUN: "A
self-published record cannot appoint itself controller of another
principal's run." A `created` record signed by any other key refuses as
`not_admitted`.

The head is the last applied record. When replay stops at a fault, `error`
names it, `at` names the sequence, and the head is the last good record, so
a client knows how far the journal verified. It never reports the run as
finished from a prefix: EOSE or an empty response cannot prove that no
later record exists.

### The state machine

The run is created at sequence 0 only and admitted once, and nothing is
dispatched before admission. Each attempt of each step moves on its own:

| From | Record | To |
| --- | --- | --- |
| (none) | `dispatched` | dispatched |
| (none) | `resolved` | resolved (refused or cancelled before dispatch) |
| dispatched | `observed` | dispatched |
| dispatched | `resolved` | resolved |
| dispatched | `unknown` | unknown |
| unknown | `reconciled` | reconciled |

`cancel_requested` resolves nothing; NIP-RUN says it "does not itself
resolve work". A retry is a new attempt, so it starts again at (none).
Nothing applies after `settled`. A run may settle with unknown outcomes
outstanding; the head counts them in `unknowns`, and such a run cannot
report verified success.

<!-- source: coax/lib/nostr/run/journal.coax transition -->
```jai
// The state machine for one record, after its chain and signer checked.
// admitted[0] is whether the run was admitted.
rn_transition :: (r:RunRecord,admitted:[]bool,table:[]RunSubject,used:[]int,unknowns:[]int)->u32 {
  if r.seq==0 {return ifx r.type==RN_CREATED then RN_OK else RN_ORDER;}
  if r.type==RN_CREATED {return RN_ORDER;}
  if r.type==RN_ADMITTED {if admitted[0] {return RN_ORDER;} admitted[0]=true;return RN_OK;}
  if r.type==RN_CANCEL_REQUESTED || r.type==RN_SETTLED {return RN_OK;}
  if r.type==RN_HANDOFF {
    if r.fence.count==0 {return RN_UNFENCED;}
    if r.next.count==0 || r.next==r.controller {return RN_MALFORMED;}
    return RN_OK;
  }
  // The rest act on one attempt of one step.
  if r.step.count==0 || r.attempt<1 {return RN_MALFORMED;}
  i:=rn_subject(table,used,r);if i<0 {return RN_LIMIT;}
  from:=table[i].state;t:=r.type;
  ok:=(t==RN_DISPATCHED && admitted[0] && from==-1) || (t==RN_OBSERVED && from==RN_DISPATCHED)
    || (t==RN_RESOLVED && (from==-1 || from==RN_DISPATCHED)) || (t==RN_UNKNOWN && from==RN_DISPATCHED)
    || (t==RN_RECONCILED && from==RN_UNKNOWN);
  if !ok {return RN_ORDER;}
  to:=ifx t==RN_OBSERVED then from else t;
  if r.type==RN_UNKNOWN {unknowns[0]+=1;}
  if r.type==RN_RECONCILED {unknowns[0]-=1;}
  table[i].state=to;return RN_OK;
}
```

An unobserved dispatch is `unknown`, not failure. NIP-RUN: "Once
dispatched, failure to observe a result is unknown, not evidence that no
effect occurred." Only attributable evidence moves it to `reconciled`.

### Controller handoff and fencing

One controller serializes a run's journal. To hand it off, the old
controller stops new dispatch, obtains a fencing acknowledgment from every
dispatcher that can still act, and then appends `handoff`. The next record
is signed by the new controller, at the next generation, with the handoff's
Digest as `previous`. Dispatchers persist the generation and reject older
ones. NIP-RUN: "A signed handoff without actual dispatcher fencing is
insufficient." `rn_transition` refuses a handoff with no fencing
acknowledgment as `unfenced`, and one that names no controller or the same
controller as `malformed`.

After the handoff, a record from the old controller at the old generation
is `stale`; one from the new controller at the old generation is also
`stale`; one at a skipped generation, or from the old controller at the new
generation, is `not_admitted`. Replay never elects a controller from
whichever signed record arrived first.

### Head hints

A `30186` head names `{run, seq, digest, generation, ...}`. It is a
retrieval hint, not the journal. `run_head_check` compares it with the
replayed head:

<!-- source: coax/lib/nostr/run/journal.coax head -->
```jai
// Checks a 30186 head hint against the replayed journal. A hint is a
// retrieval hint, never the journal: one behind (lower sequence or older
// generation) is stale and erases nothing; one ahead means records are
// missing (incomplete); the same sequence with another Digest is a fork.
run_head_check :: (h:RunHead,seq:int,digest:string,generation:int)->u32 {
  if h.error!=RN_OK {return h.error;}
  mine:=h.digest;
  if seq>h.seq {return RN_GAP;}
  if generation<h.generation || seq<h.seq {return RN_STALE;}
  if digest!=cast(string)mine[..] {return RN_CONFLICT;}
  if generation!=h.generation {return RN_NOT_ADMITTED;}
  return RN_OK;
}
```

A hint behind the journal is stale and erases nothing ("A head from an old
generation cannot erase later history"). A hint ahead of it means records
are missing and need bounded replay. The same sequence with another Digest
is a fork.

### Worked example: a three-record chain, by hand

The run is `9999...` (64 nines). Key 1 is the x-only public key of the
scalar 1, `79be667e...f81798`; key 2 is that of 2, `c6047f94...709ee5`.
T0 is 1790000000. The first three records:

| seq | type | signer | generation | subject | time |
| --- | --- | --- | --- | --- | --- |
| 0 | `created` | key 1 | 0 | null | T0 + 100 |
| 1 | `admitted` | key 1 | 0 | null | T0 + 99 |
| 2 | `dispatched` | key 1 | 0 | `build`, attempt 1 | T0 + 98 |

Record 0 in JCS form has its keys sorted (`controller`, `data`,
`generation`, `previous`, `run`, `seq`, `subject`, `time`, `type`, and
inside `subject`: `attempt`, `iteration`, `step`), no whitespace, and
`previous` null. It is 294 bytes:

```text
{"controller":"79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798","data":{},"generation":0,"previous":null,"run":"9999999999999999999999999999999999999999999999999999999999999999","seq":0,"subject":{"attempt":null,"iteration":null,"step":null},"time":1790000100,"type":"created"}
```

The hashed input is the 24 bytes `openagents.run-record.v1`, one LF, then
those 294 bytes: 319 bytes in all. Its SHA-256 is record 0's Digest:

```text
D0 = sha256:95049ef399c42b01b6ae1a43613101467d8b8c92f9031445970c0a8b1c367163
```

Record 1 sets `previous` to D0. That adds the 71-character Digest and two
quotes in place of `null`, so its JCS form is 294 + 73 - 4 + 1 = 364 bytes
(the `type` value `admitted` is one byte longer than `created`, and the
time is still ten digits):

```text
D1 = sha256:e3c22e5652391dae99b3eedb21f2916be072169e9aa7e43674f3d88aea2c0d89
```

Record 2 sets `previous` to D1, names step `build` and attempt 1, and is
366 bytes:

```text
D2 = sha256:ab466665c375391da1fddfed52da6305c64039930068423f1244ea3aa53049f1
```

Verifying the chain by hand, in the order replay does it:

1. Seq 0: one record. Its recomputed Digest is D0, as carried. `previous`
   is null. The signer is key 1, the controller named is key 1, and key 1
   is the admitted controller at generation 0. Type `created` at 0: allowed.
2. Seq 1: `previous` is D0, the Digest just applied. Key 1, generation 0.
   `admitted`, the first time: allowed.
3. Seq 2: `previous` is D1. Key 1, generation 0. `dispatched` for
   (`build`, null, 1): the run is admitted and the attempt is new, so it
   moves to dispatched.

The head is now seq 2, D2, key 1 at generation 0. The three faults the
chain must refuse, with these records:

- **A gap.** Records 0, 1 and 3 arrive but not 2. Replay applies 0 and 1,
  finds nothing at 2, sees that one record was not seen, and stops with
  `incomplete` at seq 2. The head stays at seq 1.
- **A reorder.** Records 2, 0, 1 arrive in that order. Replay looks up each
  sequence, not the arrival position, so it gives the same head, D2.
- **A fork.** A second record at seq 2 with any changed field has a
  different Digest. With both present, replay stops with `conflict` at
  seq 2. With only the other one present, it chains to D1 and applies, but
  record 3's `previous` names D2, so replay stops at seq 3 as a broken
  chain.

The full journal continues: seq 3 `handoff` from key 1 to key 2 with
fencing acknowledgment `sha256:8888...`, seq 4 `unknown` for `build`
attempt 1 by key 2 at generation 1 (the new controller cannot see the
result), seq 5 `reconciled`, and seq 6 `settled`. Its head is seq 6,
`sha256:b071b125...`, key 2, generation 1, no unknowns outstanding.

## Trajectory chunks

A trajectory travels as one manifest and its chunks, privately as kind
`3188` envelopes NIP-44 encrypted to the owner, or publicly as kind `3198`
(the manifest) and `3199` (each chunk). The manifest pins the whole bytes
by `artifact.digest` and size, and lists `{index, digest, size}` for each
chunk. A chunk carries `trajectory_id`, `artifact`, `index`, `count`,
`digest` and `text`. The records:

<!-- source: coax/lib/nostr/atif/chunks.coax record -->
```jai
AtifEntry :: struct {index:int;digest:string;size:int;}
AtifManifest :: struct {publisher:string;trajectory_id:string;artifact:string;size:int;}
AtifChunk :: struct {author:string;trajectory_id:string;artifact:string;index:int;count:int;digest:string;text:string;}
// outcome and reason; for a refused chunk, chunk is its position in the
// list given; missing counts the entries with no chunk (the first ones are
// written to the caller's array); prefix is the verified leading bytes.
AtifResult :: struct {outcome:u32;reason:u32;chunk:int;missing:int;prefix:int;}
```

<!-- source: coax/lib/nostr/atif/chunks.coax codes -->
```jai
AT_VERIFIED :: cast(u32)0;     // the bytes are the artifact's; rendered fields unverified
AT_PARTIAL :: cast(u32)1;      // content_unavailable: chunks are missing
AT_REFUSED :: cast(u32)2;

AT_OK :: cast(u32)0;
AT_MANIFEST :: cast(u32)1;     // entries not 0..n-1, n outside 1..4096, a size outside 1..32768
AT_AUTHOR :: cast(u32)2;       // a chunk not signed by the manifest's publisher
AT_TRAJECTORY :: cast(u32)3;   // a chunk for another trajectory_id or artifact
AT_INDEX :: cast(u32)4;        // index outside 0..count-1, or count not the number of entries
AT_DIGEST :: cast(u32)5;       // the chunk's digest is not SHA-256 of its text
AT_ENTRY :: cast(u32)6;        // the chunk's digest or size is not the manifest entry's
AT_ARTIFACT :: cast(u32)7;     // the concatenation is not the artifact's digest or size

AT_MAX_CHUNKS :: 4096;
AT_MAX_TEXT :: 32768;
```

NIP-ATIF's validation runs in order and stops at the first failure.
`atif_check` does steps 3 and 4: every chunk (author, trajectory and
artifact, index and count, the digest over `text`, the manifest entry's
digest and size), then every entry must have a chunk, then the
concatenation against the artifact:

<!-- source: coax/lib/nostr/atif/chunks.coax check -->
```jai
// Checks chunks (any order, duplicates allowed) against the manifest and its
// entries. missing receives the indexes of the first missing entries.
atif_check :: (m:AtifManifest,entries:[]AtifEntry,chunks:[]AtifChunk,missing:[]int)->AtifResult {
  n:=entries.count;
  if n<1 || n>AT_MAX_CHUNKS {return at_refuse(AT_MANIFEST,-1);}
  for e,i:entries {if e.index!=i || e.size<1 || e.size>AT_MAX_TEXT {return at_refuse(AT_MANIFEST,-1);}}
  text:[AT_MAX_TEXT]u8;
  for c,i:chunks {
    if c.author!=m.publisher {return at_refuse(AT_AUTHOR,i);}
    if c.trajectory_id!=m.trajectory_id || c.artifact!=m.artifact {return at_refuse(AT_TRAJECTORY,i);}
    if c.count!=n || c.index<0 || c.index>=n {return at_refuse(AT_INDEX,i);}
    // No entry is outside 1..32768 bytes, so no such text can match one.
    if c.text.count<1 || c.text.count>AT_MAX_TEXT {return at_refuse(AT_ENTRY,i);}
    for k:0..<c.text.count {text[k]=c.text[k];}
    d:[71]u8;
    if ae_digest(text[0..<c.text.count],d[..])!=AE_OK || cast(string)d[..]!=c.digest {return at_refuse(AT_DIGEST,i);}
    e:=entries[c.index];
    if c.digest!=e.digest || c.text.count!=e.size {return at_refuse(AT_ENTRY,i);}
  }
  // Every entry needs a chunk; report the missing ones.
  r:AtifResult;r.chunk=-1;prefix_open:=true;
  for e:entries {
    found:=false;
    for c:chunks {if c.index==e.index {found=true;}}
    if !found {
      if r.missing<missing.count {missing[r.missing]=e.index;}
      r.missing+=1;prefix_open=false;
    } else if prefix_open {r.prefix+=e.size;}
  }
  if r.missing>0 {r.outcome=AT_PARTIAL;return r;}
  // The concatenation in index order.
  // The state is ours and each text was bounded and hashed above, so these
  // primitives cannot refuse; their statuses are not consulted.
  state:[112]u8;started:=sha256_init(state[..]);total:=0;
  for e:entries {
    for c:chunks {
      if c.index!=e.index {continue;}
      for k:0..<c.text.count {text[k]=c.text[k];}
      added:=sha256_update_checked(state[..],text[0..<c.text.count],false);total+=c.text.count;break;
    }
  }
  raw:[32]u8;ended:=sha256_final_checked(state[..],raw[..],false);
  d:[71]u8;for i:0..<7 {d[i]="sha256:"[i];}
  hexed:=hex_encode_checked(cast(string)raw[..],d[7..<71],false);
  if cast(string)d[..]!=m.artifact || total!=m.size {return at_refuse(AT_ARTIFACT,-1);}
  return r;  // r.outcome is still AT_VERIFIED, zero
}
```

A missing chunk makes the trajectory `content_unavailable`. NIP-ATIF: "a
reader never renders a prefix as though it were the whole. It may show a
verified prefix explicitly labeled as partial." `atif_check` writes the
missing indexes to the caller's array and returns how many leading bytes
verified, so the caller can say which chunk is missing and label the
prefix.

Without an ATIF renderer, `step_count`, `state` and `steps_digest` stay
unverified, so the best outcome is "bytes verified, rendered fields
unverified" (NIP-ATIF's "unverified: content matched; rendered fields not
checked").

### Worked example: three chunks, one missing

The log `trajectory-1` has three records, one per chunk:

| index | text | bytes | digest |
| --- | --- | --- | --- |
| 0 | `{"kind":"start"}` and LF | 17 | `sha256:0966c8cb...` |
| 1 | `{"kind":"step","step_id":1}` and LF | 28 | `sha256:0fffd7c4...` |
| 2 | `{"kind":"end"}` and LF | 15 | `sha256:6a542dd2...` |

The artifact is the 60 bytes in index order, `sha256:a7c87f3a...`. With
all three chunks, in any order, each digest and size matches its entry and
the concatenation hashes to the artifact. With chunk 1 missing, both
present chunks check, entry 1 has no chunk, and the result is partial:
1 missing, index 1, and a verified prefix of 17 bytes (chunk 0 only;
chunk 2 is verified but does not follow the prefix).

## Terminal frames

A NIP-TERM frame's body is `output` or `exit` (sequenced) or `gap` or
`detached` (not sequenced). Sequence numbers are per terminal, start at 1,
and grow by one; `exit` is the last. Over a relay, each frame is a `3188`
artifact sealed with NIP-44 to the attached device, and "Retained frames
can return in any order after a reconnect, so a client orders them by
sequence number before it applies them."

<!-- source: coax/lib/nostr/term/frames.coax record -->
```jai
TermFrame :: struct {type:int;seq:int;from:int;to:int;}
TermClient :: struct {applied:int;complete:bool;}
```

<!-- source: coax/lib/nostr/term/frames.coax codes -->
```jai
TF_OUTPUT :: 0; TF_EXIT :: 1; TF_GAP :: 2; TF_DETACHED :: 3;

TA_APPLIED :: cast(u32)0;
TA_DUPLICATE :: cast(u32)1;   // at or below the applied number: ignored
TA_LOST :: cast(u32)2;        // above the next number: reattach with after = applied
TA_GAP :: cast(u32)3;         // a discarded range, shown as a gap
TA_EXIT :: cast(u32)4;        // the last frame: the attachment is complete
TA_COMPLETE :: cast(u32)5;    // a frame after exit: nothing applies
TA_DETACHED :: cast(u32)6;
TA_MALFORMED :: cast(u32)7;
```

The client keeps the highest number through which it applied every frame or
a gap. NIP-TERM's client rules, as `term_apply`:

<!-- source: coax/lib/nostr/term/frames.coax apply -->
```jai
term_apply :: (c:[]TermClient,f:TermFrame)->u32 {
  if c[0].complete {return TA_COMPLETE;}
  if f.type==TF_DETACHED {return TA_DETACHED;}
  next:=c[0].applied+1;
  if f.type==TF_GAP {
    if f.from<1 || f.to<f.from {return TA_MALFORMED;}
    if f.from>next {return TA_LOST;}
    if f.to<next {return TA_DUPLICATE;}
    c[0].applied=f.to;return TA_GAP;
  }
  if f.type!=TF_OUTPUT && f.type!=TF_EXIT {return TA_MALFORMED;}
  if f.seq<1 {return TA_MALFORMED;}
  if f.seq<next {return TA_DUPLICATE;}
  if f.seq>next {return TA_LOST;}
  c[0].applied=f.seq;
  if f.type==TF_EXIT {c[0].complete=true;return TA_EXIT;}
  return TA_APPLIED;
}
```

<!-- source: coax/lib/nostr/term/frames.coax order -->
```jai
// Sorts frames by sequence, a gap by its from, keeping arrival order among
// equal keys. A detached frame has no sequence (seq 0): it sorts first and
// changes nothing when applied.
term_key :: (f:TermFrame)->int {
  if f.type==TF_GAP {return f.from;}
  return f.seq;
}
term_order :: (frames:[]TermFrame) {
  for i:1..<frames.count {
    j:=i;
    while j>0 && term_key(frames[j-1])>term_key(frames[j]) {
      t:=frames[j-1];frames[j-1]=frames[j];frames[j]=t;j-=1;
    }
  }
}
```

A gap moves the applied number past the discarded range; the client shows
it as a gap and "never joins the output before and after a gap as if it
were continuous". A frame above the next number with no gap before it means
the transport lost frames: the client applies nothing further and
reattaches with `after` set to its applied number.

Worked by hand: a client reattaches with `after` = 2 and receives, out of
order, `exit 9`, `output 8`, `gap 3..7`, `output 8` again and `output 2`.
Sorted by sequence (a gap by its `from`): output 2, gap 3..7, output 8,
output 8, exit 9. Output 2 is at or below 2: duplicate. The gap starts at
3, the next number, so applied becomes 7. Output 8 is next: applied, 8.
The second output 8: duplicate. Exit 9: applied, and the attachment is
complete. A fresh client at 2 that receives output 4 first stops with
`lost` and stays at 2.

## Sealing a record with NIP-44

NIP-RUN content uses NIP-44 v2 to each recipient. The example builds record
0's content, `{"v":"openagents.run-record.v1","requires":[],"record":...,"digest":...}`,
433 bytes, and seals it twice with `nip44_encrypt_checked`: to recipient A
with conversation key bytes all 1 and nonce bytes all 17, and to recipient
B with key bytes all 2 and nonce bytes all 34 (fixed bytes, TEST ONLY; a
real nonce comes from a CSPRNG and is never reused). 433 bytes pad to 448;
with the 2-byte length prefix and the 65 bytes of version, nonce and MAC,
the payload is 515 bytes, 688 base64 characters. The two payloads differ;
both open to the same 433 bytes, so the record has one Digest. B's key on
A's payload fails the MAC check.

## SESS: sessions, adapters and queue law

NIP-SESS gives clients one session contract over engines that differ: a
session is a durable conversation lineage, not a process or a task. An
adapter declares what its engine supports, and the session authority
records the effective configuration it actually applied. Commands
(`submit`, `enqueue`, `cancel_queued`, `reorder`, `steer`, `interrupt`,
`configure`, `archive`, `close`) carry an expected revision and an
authorization, and use the same idempotency rule as CTRL in
[chapter 76](ch076-work-coordination-and-control.md): `(authority, session,
command)` with the SHA-256 of the command's JCS form as fingerprint;
`ctrl_fingerprint` computes it. The pinned NIP-SESS text carries an owner
note that the profile is being revised to describe Loro rooms on
`coder-sync`, so this chapter builds none of it.

## What is not built

- **RUN:** the per-type `data` schemas (owner, grant, baseline, enforcement
  plan, evidence and so on) are not checked; `data` is the caller's JSON.
  The `3187` and `30186` events, mailbox tags and relay read restrictions,
  retention horizons and expiry, `block_refs` citations, dispatcher-side
  generation checks, shared fencing authorities, reconciliation runs and
  OPT lineage are not built. The caller verifies event signatures and
  decrypts before replay.
- **ATIF:** the manifest's closed-shape check, `schema_version`, `form`,
  `coverage` and `derivation`; the `3188`, `3198` and `3199` events; the
  ATIF digest rule, rendering and `steps_digest`; children, parents and
  continued segments.
- **TERM:** opening, attaching, input, resize, signals and close; rights;
  the frames' JSON and base64; byte budgets; lifetime; and the extensions.
- **SESS:** all of it, pending the owner's revision.

## The example

[nostr/examples/run-journal.coax](../examples/run-journal.coax) builds the
seven-record journal of the worked example and prints each record's
`previous` and Digest. It replays the journal in order and from a shuffled
list with duplicates, then shows each refusal. It checks four head hints,
seals record 0's content to two recipients, checks the trajectory's chunks
with one missing and with two broken, and applies the terminal frames of
the worked example.

<!-- source: nostr/examples/run-journal.coax example -->
```jai
// Chapter 78: one run's journal through a controller handoff, replayed in
// two arrival orders; the same record sealed with NIP-44 to two recipients;
// a three-chunk trajectory with one chunk missing; and a terminal's frames
// after a reconnect. Keys are the x-only public keys of the scalars 1, 2
// and 7; conversation keys and nonces are fixed bytes: TEST ONLY.
#load "../../coax/lib/nostr/run/journal.coax";
#load "../../coax/lib/nostr/atif/chunks.coax";
#load "../../coax/lib/nostr/term/frames.coax";
#load "../../coax/lib/nostr/nip44.coax";
#import "Basic";

T0 :: 1790000000;
KEY_1 :: "79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798";
KEY_2 :: "c6047f9441ed7d6d3045406e95c07cd85c778e4b8cef3ca7abac09b95c709ee5";
KEY_7 :: "5cbdf0646e5db4eaa398f365f2ea7a0e3d419b7e0330e39ce92bddedcac4f9bc";
RUN :: "9999999999999999999999999999999999999999999999999999999999999999";
FENCE :: "sha256:8888888888888888888888888888888888888888888888888888888888888888";

RN_WORDS :: [12]string.["ok","incomplete: a gap","conflict: a fork","broken chain","digest mismatch",
  "not_admitted","stale","order","unfenced handoff","closed: after settled","malformed","limit_exceeded"];
AT_OUTCOMES :: [3]string.["bytes verified, rendered fields unverified","partial: content_unavailable","refused"];
AT_REASONS :: [8]string.["","manifest","author","trajectory","index","digest","entry","artifact"];
TA_WORDS :: [8]string.["applied","duplicate","lost: reattach","gap","exit: complete","complete: ignored","detached","malformed"];

key_name :: (key:string)->string {
  if key==KEY_1 {return "key 1";}
  if key==KEY_2 {return "key 2";}
  return "key 7";
}
record :: (seq:int,type:int,key:string,generation:int)->RunRecord {
  r:RunRecord;r.run=RUN;r.seq=seq;r.controller=key;r.signer=key;r.generation=generation;r.type=type;
  r.iteration=-1;r.time=T0+100-seq;r.data="{}";return r;
}
on_step :: (r:RunRecord)->RunRecord {s:=r;s.step="build";s.attempt=1;return s;}
// Chains records: each previous is the Digest before it; store keeps the digests.
chain :: (rs:[]RunRecord,store:[]u8) {
  work:[4096]u8;prev:="";
  for i:0..<rs.count {
    rs[i].previous=prev;d:=store[i*71..<i*71+71];
    status:=run_digest(rs[i],work[..],d);
    rs[i].digest=cast(string)d;prev=rs[i].digest;
  }
}
replay :: (label:string,rs:[]RunRecord) {
  t:[8]RunSubject;w:[4096]u8;h:=run_replay(rs,KEY_1,t[..],w[..]);
  print("%: %",label,RN_WORDS[h.error]);
  if h.error!=RN_OK {print(" at seq %",h.at);}
  if h.seq>=0 {d:=h.digest;print("; head seq %, % generation %, %...",h.seq,key_name(h.controller),h.generation,cast(string)d[0..<19]);}
  if h.settled {print(", settled");}
  print("\n");
}
```

<!-- source: nostr/examples/run-journal.coax main -->
```jai
main :: () {
  print("RUN: run 9999..., created by key 1; key 1 hands off to key 2 at seq 3\n");
  rs:[7]RunRecord;store:[497]u8;
  rs[0]=record(0,RN_CREATED,KEY_1,0);rs[1]=record(1,RN_ADMITTED,KEY_1,0);rs[2]=on_step(record(2,RN_DISPATCHED,KEY_1,0));
  rs[3]=record(3,RN_HANDOFF,KEY_1,0);rs[3].next=KEY_2;rs[3].fence=FENCE;
  rs[4]=on_step(record(4,RN_UNKNOWN,KEY_2,1));rs[5]=on_step(record(5,RN_RECONCILED,KEY_2,1));rs[6]=record(6,RN_SETTLED,KEY_2,1);
  chain(rs[..],store[..]);
  json:[1024]u8;n:=run_record_write(rs[0],json[..]);
  print("record 0 as written (% bytes):\n  %\n",n,cast(string)json[0..<n]);
  for r:rs {
    print("seq % %, % generation %, step %, time T0 + %\n  previous %\n  digest   %\n",r.seq,RN_NAMES[r.type],key_name(r.signer),r.generation,
      ifx r.step.count==0 then "null" else r.step,r.time-T0,ifx r.previous.count==0 then "null" else r.previous,r.digest);
  }

  print("replay\n");
  replay("in order",rs[..]);
  mixed:=[10]RunRecord.[rs[6],rs[4],rs[0],rs[2],rs[5],rs[2],rs[1],rs[3],rs[6],rs[0]];
  replay("reversed and shuffled, with three duplicates",mixed[..]);
  replay("only seq 0 to 2, before the handoff",rs[0..<3]);
  gap:=[6]RunRecord.[rs[0],rs[1],rs[2],rs[3],rs[5],rs[6]];
  replay("seq 4 missing",gap[..]);
  other:=rs[4];other.time+=1;od:[71]u8;w:[4096]u8;
  status:=run_digest(other,w[..],od[..]);other.digest=cast(string)od[..];
  fork:=[8]RunRecord.[rs[0],rs[1],rs[2],rs[3],rs[4],other,rs[5],rs[6]];
  replay("a second record at seq 4",fork[..]);
  alone:=[7]RunRecord.[rs[0],rs[1],rs[2],rs[3],other,rs[5],rs[6]];
  replay("that record instead of the first",alone[..]);
  x:=rs;xd:[497]u8;x[4].controller=KEY_1;x[4].signer=KEY_1;x[4].generation=0;chain(x[..],xd[..]);
  replay("key 1 appends seq 4 after its handoff",x[..]);
  x=rs;x[3].fence="";chain(x[..],xd[..]);
  replay("the handoff without a fencing acknowledgment",x[..]);
  x=rs;x[0].controller=KEY_7;x[0].signer=KEY_7;chain(x[..],xd[..]);
  replay("key 7 signs a created record naming itself",x[0..<1]);
  x=rs;x[2].attempt=2;
  replay("seq 2 edited to attempt 2, digest kept",x[..]);
  x=rs;x[4].type=RN_RECONCILED;chain(x[..],xd[..]);
  replay("reconciled with no unknown before it",x[..]);
  more:[8]RunRecord;md:[568]u8;for i:0..<7 {more[i]=rs[i];}
  more[7]=record(7,RN_CANCEL_REQUESTED,KEY_2,1);chain(more[..],md[..]);
  replay("cancel_requested after settled",more[..]);

  print("head hints against the replayed journal\n");
  t:[8]RunSubject;h:=run_replay(rs[..],KEY_1,t[..],w[..]);
  print("seq 6, generation 1, the head's digest: %\n",RN_WORDS[run_head_check(h,6,rs[6].digest,1)]);
  print("seq 3, generation 0 (key 1's last head): %\n",RN_WORDS[run_head_check(h,3,rs[3].digest,0)]);
  print("seq 8: %\n",RN_WORDS[run_head_check(h,8,rs[6].digest,1)]);
  print("seq 6 with seq 5's digest: %\n",RN_WORDS[run_head_check(h,6,rs[5].digest,1)]);

  print("NIP-44: record 0's content sealed to two recipients\n");
  content:[2048]u8;
  at:=json_raw(content[..],0,"{\"v\":\"openagents.run-record.v1\",\"requires\":[],\"record\":");
  at=json_raw(content[..],at,cast(string)json[0..<n]);
  at=json_raw(content[..],at,",\"digest\":");at=json_write_string(content[..],at,rs[0].digest);at=json_raw(content[..],at,"}");
  plain:=content[0..<at];
  k1:[32]u8;k2:[32]u8;n1:[32]u8;n2:[32]u8;
  for i:0..<32 {k1[i]=1;k2[i]=2;n1[i]=17;n2[i]=34;}
  ework:[4096]u8;p1:[4096]u8;p2:[4096]u8;opened:[2048]u8;dwork:[8192]u8;
  s1:=nip44_encrypt_checked(NIP44_COMPAT,k1[..],n1[..],plain,ework[..],p1[..],false);
  s2:=nip44_encrypt_checked(NIP44_COMPAT,k2[..],n2[..],plain,ework[..],p2[..],false);
  print("content: % bytes; payloads: % and % characters\n",at,s1.written,s2.written);
  print("  recipient A: %...\n  recipient B: %...\n",cast(string)p1[0..<24],cast(string)p2[0..<24]);
  o1:=nip44_decrypt_checked(NIP44_COMPAT,k1[..],cast(string)p1[0..<s1.written],dwork[..],opened[..],false);
  same1:=o1.error==NIP44_OK && cast(string)opened[0..<o1.written]==cast(string)plain;
  o2:=nip44_decrypt_checked(NIP44_COMPAT,k2[..],cast(string)p2[0..<s2.written],dwork[..],opened[..],false);
  same2:=o2.error==NIP44_OK && cast(string)opened[0..<o2.written]==cast(string)plain;
  print("both open to the same content, one record digest: %\n",ifx same1 && same2 then "yes" else "no");
  wrong:=nip44_decrypt_checked(NIP44_COMPAT,k2[..],cast(string)p1[0..<s1.written],dwork[..],opened[..],false);
  print("recipient B's key on A's payload: %\n",ifx wrong.error==NIP44_MAC then "refused, MAC mismatch" else "opened");

  print("ATIF: trajectory-1, a 60-byte log in three chunks, published by key 1\n");
  m:=AtifManifest.{publisher=KEY_1,trajectory_id=AT_ID,artifact=AT_ALL,size=60};
  e:[3]AtifEntry;at_entries(e[..]);miss:=[3]int.[-1,-1,-1];
  c:=[3]AtifChunk.[at_chunk(2,AT_D2,AT_L2),at_chunk(0,AT_D0,AT_L0),at_chunk(1,AT_D1,AT_L1)];
  r:=atif_check(m,e[..],c[..],miss[..]);
  print("chunks 2, 0, 1: %, % bytes\n",AT_OUTCOMES[r.outcome],r.prefix);
  two:=[2]AtifChunk.[c[0],c[1]];
  r=atif_check(m,e[..],two[..],miss[..]);
  print("chunk 1 missing: %, % missing (index %), verified prefix % bytes\n",AT_OUTCOMES[r.outcome],r.missing,miss[0],r.prefix);
  bad:=c;bad[2].text="{\"kind\":\"step\",\"step_id\":2}\n";
  r=atif_check(m,e[..],bad[..],miss[..]);
  print("chunk 1's text edited: %, %, at list position %\n",AT_OUTCOMES[r.outcome],AT_REASONS[r.reason],r.chunk);
  bad=c;bad[1].author=KEY_7;
  r=atif_check(m,e[..],bad[..],miss[..]);
  print("chunk 0 signed by key 7: %, %, at list position %\n",AT_OUTCOMES[r.outcome],AT_REASONS[r.reason],r.chunk);

  print("TERM: frames retained over a relay, after a reconnect with after = 2\n");
  f:=[5]TermFrame.[TermFrame.{type=TF_EXIT,seq=9},TermFrame.{type=TF_OUTPUT,seq=8},TermFrame.{type=TF_GAP,from=3,to=7},
    TermFrame.{type=TF_OUTPUT,seq=8},TermFrame.{type=TF_OUTPUT,seq=2}];
  term_order(f[..]);
  client:=[1]TermClient.[TermClient.{applied=2}];
  names:=[4]string.["output","exit","gap","detached"];
  for fr:f {
    a:=term_apply(client[..],fr);
    if fr.type==TF_GAP {print("gap % to %: %\n",fr.from,fr.to,TA_WORDS[a]);}
    else {print("% %: %\n",names[fr.type],fr.seq,TA_WORDS[a]);}
  }
  fresh:=[1]TermClient.[TermClient.{applied=2}];
  print("a fresh client at 2 gets output 4: %, applied stays %\n",TA_WORDS[term_apply(fresh[..],TermFrame.{type=TF_OUTPUT,seq=4})],fresh[0].applied);
}
```

The transcript ([nostr/fixtures/run-journal.out](../fixtures/run-journal.out)):

```text
RUN: run 9999..., created by key 1; key 1 hands off to key 2 at seq 3
record 0 as written (294 bytes):
  {"run":"9999999999999999999999999999999999999999999999999999999999999999","seq":0,"previous":null,"controller":"79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798","generation":0,"type":"created","subject":{"step":null,"iteration":null,"attempt":null},"time":1790000100,"data":{}}
seq 0 created, key 1 generation 0, step null, time T0 + 100
  previous null
  digest   sha256:95049ef399c42b01b6ae1a43613101467d8b8c92f9031445970c0a8b1c367163
seq 1 admitted, key 1 generation 0, step null, time T0 + 99
  previous sha256:95049ef399c42b01b6ae1a43613101467d8b8c92f9031445970c0a8b1c367163
  digest   sha256:e3c22e5652391dae99b3eedb21f2916be072169e9aa7e43674f3d88aea2c0d89
seq 2 dispatched, key 1 generation 0, step build, time T0 + 98
  previous sha256:e3c22e5652391dae99b3eedb21f2916be072169e9aa7e43674f3d88aea2c0d89
  digest   sha256:ab466665c375391da1fddfed52da6305c64039930068423f1244ea3aa53049f1
seq 3 handoff, key 1 generation 0, step null, time T0 + 97
  previous sha256:ab466665c375391da1fddfed52da6305c64039930068423f1244ea3aa53049f1
  digest   sha256:c86e8b7605c80d2c6a9f8db27435cc3be2a47e34e032526b1443e68e69eee3e5
seq 4 unknown, key 2 generation 1, step build, time T0 + 96
  previous sha256:c86e8b7605c80d2c6a9f8db27435cc3be2a47e34e032526b1443e68e69eee3e5
  digest   sha256:62cdbddee0ab9ded39e8ad3be801cbb9e17df7b0c21e63151a1f0e72fff4645d
seq 5 reconciled, key 2 generation 1, step build, time T0 + 95
  previous sha256:62cdbddee0ab9ded39e8ad3be801cbb9e17df7b0c21e63151a1f0e72fff4645d
  digest   sha256:fd922bec5e277b2d0d222c7e6fea072b3f5bdd95e6f6b2dd711f12cd5b3d900d
seq 6 settled, key 2 generation 1, step null, time T0 + 94
  previous sha256:fd922bec5e277b2d0d222c7e6fea072b3f5bdd95e6f6b2dd711f12cd5b3d900d
  digest   sha256:b071b1250fbbd3fe616a4f1f2a672218d3efce1517c8ade3716600b5faf603ae
replay
in order: ok; head seq 6, key 2 generation 1, sha256:b071b1250fbb..., settled
reversed and shuffled, with three duplicates: ok; head seq 6, key 2 generation 1, sha256:b071b1250fbb..., settled
only seq 0 to 2, before the handoff: ok; head seq 2, key 1 generation 0, sha256:ab466665c375...
seq 4 missing: incomplete: a gap at seq 4; head seq 3, key 2 generation 1, sha256:c86e8b7605c8...
a second record at seq 4: conflict: a fork at seq 4; head seq 3, key 2 generation 1, sha256:c86e8b7605c8...
that record instead of the first: broken chain at seq 5; head seq 4, key 2 generation 1, sha256:26ccb0af8c1d...
key 1 appends seq 4 after its handoff: stale at seq 4; head seq 3, key 2 generation 1, sha256:c86e8b7605c8...
the handoff without a fencing acknowledgment: unfenced handoff at seq 3; head seq 2, key 1 generation 0, sha256:ab466665c375...
key 7 signs a created record naming itself: not_admitted at seq 0
seq 2 edited to attempt 2, digest kept: digest mismatch at seq 2; head seq 1, key 1 generation 0, sha256:e3c22e565239...
reconciled with no unknown before it: order at seq 4; head seq 3, key 2 generation 1, sha256:c86e8b7605c8...
cancel_requested after settled: closed: after settled at seq 7; head seq 6, key 2 generation 1, sha256:b071b1250fbb..., settled
head hints against the replayed journal
seq 6, generation 1, the head's digest: ok
seq 3, generation 0 (key 1's last head): stale
seq 8: incomplete: a gap
seq 6 with seq 5's digest: conflict: a fork
NIP-44: record 0's content sealed to two recipients
content: 433 bytes; payloads: 688 and 688 characters
  recipient A: AhERERERERERERERERERERER...
  recipient B: AiIiIiIiIiIiIiIiIiIiIiIi...
both open to the same content, one record digest: yes
recipient B's key on A's payload: refused, MAC mismatch
ATIF: trajectory-1, a 60-byte log in three chunks, published by key 1
chunks 2, 0, 1: bytes verified, rendered fields unverified, 60 bytes
chunk 1 missing: partial: content_unavailable, 1 missing (index 1), verified prefix 17 bytes
chunk 1's text edited: refused, digest, at list position 2
chunk 0 signed by key 7: refused, author, at list position 1
TERM: frames retained over a relay, after a reconnect with after = 2
output 2: duplicate
gap 3 to 7: gap
output 8: applied
output 8: duplicate
exit 9: exit: complete
a fresh client at 2 gets output 4: lost: reattach, applied stays 2
```

Each refusal stops at a different check: a missing record at the gap
(`incomplete`), a second record at seq 4 at the fork (`conflict`), the
other record alone at seq 5's `previous` (broken chain), key 1 appending
after its own handoff at the generation (`stale`), the handoff without an
acknowledgment at the transition (`unfenced`), key 7's `created` at the
signer (`not_admitted`), an edited record whose Digest was kept at the
Digest, `reconciled` with no `unknown` at the state machine, and a record
after `settled` (`closed`). The trajectory with an edited chunk refuses at
the chunk's digest, and a chunk signed by key 7 at its author.

## How the code is checked

### Checks in the example

The checks are in
[nostr/examples/checks/run-journal.coax](../examples/checks/run-journal.coax),
which loads the example, so running the example doesn't run them. They
compare the code with values that share no code with it:

- The seven record Digests from Python's `json.dumps(sort_keys=True,
  separators=(",", ":"), ensure_ascii=False)` and `hashlib.sha256`, with
  each record built as a Python dict and chained there. `jcs.coax`'s bytes
  for record 0 must equal Python's 294 bytes.
- PLAN.md's "Done means": the journal replays to the same head after the
  controller handoff in all seven rotations of the arrival order and each
  reversed; each of the three chunks, when missing, is reported by index
  with its verified prefix.
- The NIP-44 payload length and the chunk sizes of the worked examples.

<!-- source: nostr/examples/checks/run-journal.coax check -->
```jai
// The seven record digests, from Python's json.dumps(sort_keys=True,
// separators=(",", ":"), ensure_ascii=False) and hashlib.sha256 over
// b"openagents.run-record.v1\n" and those bytes, building each record as a
// Python dict with previous set to the digest before. They share no code
// with jcs.coax, json.coax or sha256.coax. The JCS lengths are 294, 364,
// 366, 539, 363, 366 and 363 bytes.
PY_DIGESTS :: [7]string.[
  "sha256:95049ef399c42b01b6ae1a43613101467d8b8c92f9031445970c0a8b1c367163",
  "sha256:e3c22e5652391dae99b3eedb21f2916be072169e9aa7e43674f3d88aea2c0d89",
  "sha256:ab466665c375391da1fddfed52da6305c64039930068423f1244ea3aa53049f1",
  "sha256:c86e8b7605c80d2c6a9f8db27435cc3be2a47e34e032526b1443e68e69eee3e5",
  "sha256:62cdbddee0ab9ded39e8ad3be801cbb9e17df7b0c21e63151a1f0e72fff4645d",
  "sha256:fd922bec5e277b2d0d222c7e6fea072b3f5bdd95e6f6b2dd711f12cd5b3d900d",
  "sha256:b071b1250fbbd3fe616a4f1f2a672218d3efce1517c8ade3716600b5faf603ae"];
PY_JCS_0 :: "{\"controller\":\"79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798\",\"data\":{},\"generation\":0,\"previous\":null,\"run\":\"9999999999999999999999999999999999999999999999999999999999999999\",\"seq\":0,\"subject\":{\"attempt\":null,\"iteration\":null,\"step\":null},\"time\":1790000100,\"type\":\"created\"}";

journal :: (rs:[]RunRecord,store:[]u8) {
  rs[0]=record(0,RN_CREATED,KEY_1,0);rs[1]=record(1,RN_ADMITTED,KEY_1,0);rs[2]=on_step(record(2,RN_DISPATCHED,KEY_1,0));
  rs[3]=record(3,RN_HANDOFF,KEY_1,0);rs[3].next=KEY_2;rs[3].fence=FENCE;
  rs[4]=on_step(record(4,RN_UNKNOWN,KEY_2,1));rs[5]=on_step(record(5,RN_RECONCILED,KEY_2,1));rs[6]=record(6,RN_SETTLED,KEY_2,1);
  chain(rs,store);
}

#test "the chain matches Python's digests, and record 0's JCS bytes are Python's" #steps 400000000 {
  rs:[7]RunRecord;store:[497]u8;journal(rs[..],store[..]);
  for i:0..<7 {#assert rs[i].digest==PY_DIGESTS[i];}
  #assert rs[0].previous=="" && rs[1].previous==PY_DIGESTS[0] && rs[2].previous==PY_DIGESTS[1];
  json:[1024]u8;canon:[1024]u8;n:=run_record_write(rs[0],json[..]);
  j:=jcs_canonicalize_checked(cast(string)json[0..<n],canon[..],false);
  #assert j.error==JCS_OK && cast(string)canon[0..<j.written]==PY_JCS_0 && PY_JCS_0.count==294;
}

// PLAN.md's Done means: the journal replays to the same head after a
// controller handoff, whatever order the records arrive in.
#test "Done means: the same head after a handoff, in every rotation and reversed" #steps 2000000000 {
  rs:[7]RunRecord;store:[497]u8;journal(rs[..],store[..]);
  t:[8]RunSubject;w:[4096]u8;
  for k:0..<7 {
    rot:[7]RunRecord;for i:0..<7 {rot[i]=rs[(i+k)%7];}
    h:=run_replay(rot[..],KEY_1,t[..],w[..]);d:=h.digest;
    #assert h.error==RN_OK && h.seq==6 && h.controller==KEY_2 && h.generation==1 && h.settled;
    #assert cast(string)d[..]==PY_DIGESTS[6];
    rev:[7]RunRecord;for i:0..<7 {rev[i]=rot[6-i];}
    h=run_replay(rev[..],KEY_1,t[..],w[..]);d=h.digest;
    #assert h.error==RN_OK && cast(string)d[..]==PY_DIGESTS[6];
  }
}

// PLAN.md's Done means: a chunk missing from its manifest is reported.
#test "Done means: each missing chunk is reported by index, never ignored" #steps 400000000 {
  m:=AtifManifest.{publisher=KEY_1,trajectory_id=AT_ID,artifact=AT_ALL,size=60};
  e:[3]AtifEntry;at_entries(e[..]);
  all:=[3]AtifChunk.[at_chunk(0,AT_D0,AT_L0),at_chunk(1,AT_D1,AT_L1),at_chunk(2,AT_D2,AT_L2)];
  prefix:=[3]int.[0,17,45];
  for gone:0..<3 {
    kept:[2]AtifChunk;k:=0;
    for i:0..<3 {if i!=gone {kept[k]=all[i];k+=1;}}
    miss:=[3]int.[-1,-1,-1];
    r:=atif_check(m,e[..],kept[..],miss[..]);
    #assert r.outcome==AT_PARTIAL && r.missing==1 && miss[0]==gone && r.prefix==prefix[gone];
  }
}

// The worked numbers: the content sealed with NIP-44 and the chunk sizes.
#test "the transcript's numbers" #steps 400000000 {
  #assert AT_L0.count==17 && AT_L1.count==28 && AT_L2.count==15 && AT_L0.count+AT_L1.count+AT_L2.count==60;
  // 433 content bytes pad to 448; 2 + 448 + 65 = 515 raw bytes, 688 base64 characters.
  #assert nip44_calc_padded_len(433)==448 && nip44_payload_size(NIP44_COMPAT,433)==688;
}
```

The NIPs publish no wire examples with real ids or digests: NIP-RUN and
NIP-TERM give none, and NIP-ATIF's worked fixture lives in the deleted Rust
tree. So there are no published ids to compare or to find wrong. The
independent check is the Python digest chain above.

### The library's own checks

`journal.coax` checks the record JSON byte for byte, record 0's Digest
against Python's, the work-buffer bounds at their exact edges, the Done
means replay in order, reversed and shuffled with duplicates, prefixes of
the journal, gaps, forks in both orders, broken chains, forged Digests, a
non-null `previous` at 0, another run's record, each controller and
generation case around a handoff, unfenced and malformed handoffs, each
forbidden transition, records after `settled`, the subject table's bounds
and every head-hint outcome. `chunks.coax` checks three chunks in any
order, duplicates, one-byte chunks, each missing chunk, each refusal in the
NIP's order, the manifest bounds (4,096 entries and 32,768-byte chunks) and
the artifact's digest and size. `frames.coax` checks in-order frames,
duplicates, lost frames, exit, gaps, malformed frames and the stable sort.

```shell
coax/bin/coax test coax/lib/nostr/run/journal.coax
coax/bin/coax mutate coax/lib/nostr/run/journal.coax --root-only
coax/bin/coax test coax/lib/nostr/atif/chunks.coax
coax/bin/coax mutate coax/lib/nostr/atif/chunks.coax --root-only
coax/bin/coax test coax/lib/nostr/term/frames.coax --library
coax/bin/coax mutate coax/lib/nostr/term/frames.coax --library --root-only
```

On 2026-11-01, `journal.coax` passed its 8 checks (103 with `jcs.coax`,
`envelope.coax` and SHA-256's own) and `coax mutate --root-only` killed all
335 mutants; `chunks.coax` passed its 6 checks and killed all 97;
`frames.coax` passed its 5 checks and killed all 64. None survived and none
is marked equivalent. The full mutation run (`COAX_MUTATION_FULL=1`) and
cross-target runs were not done.

## Summary

- A RUN record's Digest is SHA-256 of `openagents.run-record.v1`, LF and
  its JCS form; it is the same for every recipient's copy.
- `run_replay` orders records by sequence and chain, never by time or
  arrival, and refuses gaps (incomplete), forks (conflict), broken links,
  forged Digests and forbidden transitions where they occur.
- Handoff needs the dispatchers' fencing acknowledgment; after it only the
  new controller at the next generation appends, and the old one is stale.
- An unobserved dispatch is unknown until reconciled with evidence.
- A head hint behind the journal is stale, ahead of it is incomplete, and
  never replaces the journal.
- `atif_check` reports each missing chunk by index and never treats a
  prefix as the whole; a trajectory proves who published it, not what
  happened.
- `term_apply` applies frames in sequence, ignores duplicates, shows gaps
  and stops on a lost frame until the client reattaches.

## Dependencies

- [76. Work, coordination and control](ch076-work-coordination-and-control.md)
- [20. NIP-44 encrypted payloads](ch020-nip-44-encrypted-payloads.md)

## Sources

- [OpenAgents NIP-RUN](../nips/openagents/NIP-RUN.md)
- [OpenAgents NIP-SESS](../nips/openagents/NIP-SESS.md)
- [OpenAgents NIP-TERM](../nips/openagents/NIP-TERM.md)
- [OpenAgents NIP-ATIF](../nips/openagents/NIP-ATIF.md)
- [OpenAgents shared contracts](../nips/openagents/contracts.md)
- [RFC 8785, JSON Canonicalization Scheme](https://www.rfc-editor.org/rfc/rfc8785)
- The Coax Nostr plan: [coax/nostr.md](../../coax/nostr.md).
