# The relay transport, under a recorded episode

The relay door has existed for a while: `RelayDoor::from_env` in
[`generate.rs`](../../crates/coder/src/generate.rs), `CODER_WORKER` and
`CODER_RELAY`, [NIP-CJ](../../nips/openagents/NIP-CJ.md) on the wire, and
`crates/nostr-relay` serving `relay.openagents.com`. Until 2026-09-19 no
run of `coder` had gone through it and left a trace. "The code path
exists" and "it works" are different claims, and this page is the second
one.

## What ran

One prompt, twice, alternating:

```text
In one sentence, and without running any commands, what is an ephemeral
Nostr event kind?
```

Once against a direct door — `CODER_DOOR_KEY` with the Vercel AI Gateway
on `google/gemini-3.8-flash` — and once with `CODER_WORKER` and
`CODER_RELAY` set, against `wss://relay.openagents.com` with a
`coder-worker` answering through the same gateway and the same model.
Eighteen pairs, `TYPESAFE_API_KEY` set for both so Classify ran on both.
Every run wrote its own trace with `--trace`.

Both transports answered every time. Exit code `0`, route `respond`,
eighteen traces each.

## The path is the same

A trace's steps, in order, on both transports:

```text
User -> classify -> System -> Agent
```

One step shape on each side, and the same one: the turn takes the user's
draft, puts it to Classify, records the instructions the generation was
given, and records the answer. Thirty-six traces, one shape. The
transport is not part of the path, which is what makes the two runs
comparable at all.

## The trace names the transport

The session header carries `door`, and it is the word that tells the two
runs apart:

```jsonc
// the direct run
{"record":"session","session":{"model":"google/gemini-3.8-flash","door":"live", …}}

// the relay run
{"record":"session","session":{"model":"unknown","door":"relay", …}}
```

A run whose evidence cannot say how it was routed cannot be compared
against one routed differently, so this field is load-bearing rather than
decorative.

## The step names what answered

The original measurement left a gap here: the relay session header read
`model: relay`, which named a model that does not exist. The header is
written when the session opens and no worker has answered yet, so it
cannot name one. [#9437](https://github.com/OpenAgentsInc/openagents/issues/9437)
moved the model to the step, where it is known:

```jsonc
// the relay run's answer step
{"record":"step","step":{"source":"Agent","model":"google/gemini-3.8-flash", …}}
```

The worker already names the model in its NIP-CJ result; the door used to
drop it. Now it hands the name to the turn as sideband, the answer step
records it, and the rendered document's `model_name` takes the step's word
over the session's. A session that reached two workers records two models
rather than one wrong one. The header says `unknown`, which is what the
session knows at the moment it is written.

This matters because every comparison in this repository rests on knowing
which door answered. `docs/gym/regression.md` refuses a comparison when
door identity moves, and it cannot refuse what it cannot read.

## Judged against the same golden

Step shapes agreeing is one claim. A judge reading both traces the same way
is a stronger one, and `coderbench` landed while this was being measured,
so all thirty-six traces went through it against one golden:

```sh
coderbench diff devin-fan-out-six <trace>
```

Every one exits `1` with the same seven faults, in the same order, word for
word:

```text
7 faults, in the order the path takes:
   1. never ran the capability_probe check
   2. never ran the program_registry check
   3. selected program none, expected delegate-fan-out
   4. never asked the program decision
   5. never asked the independence decision
   6. never ran the admission_check check
   7. started 0 delegations, expected 6
```

Read that for what it is. The golden is `devin-fan-out-six`, and this
prompt was never going to take its path — the program runtime that would
is [#9409](https://github.com/OpenAgentsInc/openagents/issues/9409) and is
not built. So the golden fails on both, which is expected, and what the
run establishes is that it fails **identically** on both: eighteen direct
traces and eighteen relay traces, one fault list between them. A golden
that passed on one and failed on the other would have found a transport
bug or an over-specified golden. Neither is there.

That fault list is the grader as it stood on 2026-09-19. Audit finding A04
([#9418](https://github.com/OpenAgentsInc/openagents/issues/9418)) added
faults about evidence a trace cannot carry, so the same command on the same
files now prints more of them and exits `1` for the same reason. What was
measured here — both transports judged identically — is unchanged.

## Latency, per transport

Eighteen alternating pairs on a developer Mac that was **not quiet** —
several agents were working on it at the same time — so read the medians
and ignore the extremes.

| Transport | Generate p50 | Generate min–max | Wall p50 | Output tokens p50 |
| --- | --- | --- | --- | --- |
| Direct (`door: live`) | 2875 ms | 1741–4463 ms | 3678 ms | 228 |
| Relay (`door: relay`) | 3284 ms | 2616–4463 ms | 4148 ms | 220 |

Generate time is the `duration_ms` the trace records against the agent's
answer step. Wall time is the whole process, Classify and startup
included.

The difference between the two medians, 409 ms of generate time and
470 ms of wall time, is the relay. It can be measured directly rather than
inferred, because the worker logs its own upstream generation time for
each job: subtract that from what the client measured and what remains is
the round trip.

| Relay round trip | p50 | min | max | n |
| --- | --- | --- | --- | --- |
| Client-observed minus worker-observed | 471 ms | 417 ms | 615 ms | 18 |

Two subscriptions, a publish, an authenticated WebSocket, and a stream of
encrypted feedback events cost about half a second per turn against the
production relay from this machine. That is the honest price, and it is
close enough to the median difference between the two transports to say
the two measurements agree.

### Re-measured after the fixes

Six alternating pairs on 2026-09-20, same prompt, same relay, same
gateway and model, on a machine that was again not quiet:

| Transport | Generate p50 | Generate min–max | Wall p50 | Output tokens p50 |
| --- | --- | --- | --- | --- |
| Direct (`door: live`) | 3228 ms | 2656–4329 ms | 4117 ms | 266 |
| Relay (`door: relay`) | 3575 ms | 2506–4067 ms | 4514 ms | 263 |

| Relay round trip | p50 | min | max | n |
| --- | --- | --- | --- | --- |
| Client-observed minus worker-observed, one host | 470 ms | 441 ms | 514 ms | 6 |
| Deployed worker (`deploy/systemd/coder-worker.service`, `coder -p` from another machine) | 4.0 s | 3.9 s | 5.1 s | 6 |

470 ms against the earlier 471 ms. The two runs are a day apart with
different code on the client, so the agreement is worth more than either
number: splitting the waits and carrying the model back did not move the
price of the transport.

The deployed row is a different measurement from the one above it and is
not comparable to it: the worker answers through `devin-local`, so the
worker-observed time is a Devin CLI turn (7.9–21.9 s), not a gateway
generation, and the client and worker are on different machines and
networks. What the row holds is the price of the public relay round trip
for a single turn: `coder -p` wall clock minus the worker's own `answered
in` line, in the setup under [Deployed](#deployed) below
([#9435](https://github.com/OpenAgentsInc/openagents/issues/9435)).

All six relay answer steps name `google/gemini-3.8-flash`, which is what
the worker was answering through. Before the fix all six would have read
`relay`. `coderbench diff devin-fan-out-six` read all twelve traces the
same way it read the first thirty-six: one fault list, word for word,
across both transports.

## The refusal causes

A relay that is unreachable, a worker that is absent, a worker that
started and stopped, and a worker that declined are four different states.
`gym::eval::classify` draws the line that matters: a typed refusal is an
answer, a failure with no code is the harness. `coder -p --json` reports
`cause` and `refusal` as fields, so a harness reads them rather than
matching on prose.

Measured against the production relay on 2026-09-20:

| State | `cause` | `refusal` | Exit | How long | What `error` says |
| --- | --- | --- | --- | --- | --- |
| The relay is not there | `relay_unreachable` | `null` | `1` | 0.7 s | `relay: connect: IO error: Connection refused (os error 61)` |
| No worker is listening | `worker_absent` | `null` | `1` | 31 s | `no worker answered: nothing came back from <pubkey> in 30 seconds: either no worker is listening, or one is and said nothing while it worked` |
| A worker started and stopped | `worker_stalled` | `null` | `1` | up to 180 s | `the worker stopped mid-answer: <pubkey> started answering and stopped` |
| A worker declined | `worker_declined` | `quota_exhausted` | `1` | 1.3 s | `the worker declined (quota_exhausted): …` |
| The environment names two doors | `config` | `null` | `1` | under 0.1 s | `the environment names two doors: CODER_DOOR_KEY asks for an own-key door and CODER_WORKER asks for the relay. Unset one of them.` |

All of them exit `1`, because in all of them the turn did not finish and
there is no reply to show. What separates them is the field, not the code,
and the trace records it too: a failed turn writes a `System` step saying
`the turn did not finish (<cause>): <reason>` before the log closes, so a
reader holding only the trace still learns why. The `config` row is the
exception, and deliberately: a run that cannot say which door it used
writes no trace at all rather than one claiming a door.

### Silence is two waits, not one

The first measurement's absent worker cost 181 seconds, because one
`TURN_TIMEOUT` covered both the wait for a worker and the wait for a
model. [#9436](https://github.com/OpenAgentsInc/openagents/issues/9436)
split them. `CONTACT_TIMEOUT` is 30 seconds and bounds the wait for the
first sign that a worker is there at all; `ANSWER_TIMEOUT` is 180 seconds
and bounds the wait for the answer once one is.

Any signed event from the worker's key, `e`-tagged to the request, is the
sign: a judgment, a partial, a refusal, or the result. All of them prove
something read the job, so the moment one arrives the door moves to the
long wait and a failure after that reads `worker_stalled` rather than
`worker_absent`.

An absent worker is still an inference and the sentence says so. NIP-CJ's
kinds are ephemeral, so an unanswered request leaves nothing on the relay
to ask about and a client cannot prove nobody is listening — a worker that
picked the job up and stayed quiet for 30 seconds reads as absent. A
`status: queued` feedback event would settle it, and that is a NIP-CJ
revision rather than a client change: a worker that lies about being ready
is a worse failure than one that says nothing. The 30 seconds is wide
enough that a worker whose own door is retrying an empty stream is still
counted as present.

`worker_stalled` is exercised by `crates/coder/tests/relay_job.rs`, which
runs a mock worker that sends one judgment and then nothing against a real
relay, with both waits shortened so the test costs seconds.

### What the worker guarantees

The rows above are the terminal's reading of a relay that carries
nothing. The worker's side of the same contract is that every request it
can attribute to a customer gets a typed answer, and that nothing a relay
or a customer sends stops it from serving the next one.
`crates/coder/tests/worker_lifecycle.rs` runs the built binary against a
loopback relay the test controls and holds it to each row.

| What arrives | What the worker does | Code |
| --- | --- | --- |
| The relay drops the socket, restarts, or closes the subscription | Waits one second, doubling to a minute, connects again, subscribes again with the same filter. Jobs published while it was away are lost, since the kinds are ephemeral; the worker is not. | none; `relay: <why>; reconnecting in N s` in the log |
| An event that does not parse, is not kind `25900`, does not verify, or names another worker | Set aside with one log line. Nothing proved who sent it, so nobody is answered. | none; `ignored <id prefix>: <why>` |
| The same event ID a second time | Set aside. One request is answered once; the last 4096 IDs are remembered. | none; `ignored <id prefix>: already delivered` |
| Content that does not decrypt under NIP-44, is not JSON, or is not a JSON object | Refused. The customer signed it, so the customer is told. | `malformed` |
| A `v` the worker does not serve | Refused, at version 2. | `unsupported_version` |
| A `created_at` more than ten minutes in the past | Refused as a replay: nothing on this path is stored, so an old request arriving now was not published now. | `stale` |
| A customer off `CODER_WORKER_ALLOW` | Refused. | `not_admitted` |
| A request past `CODER_WORKER_JOBS` | Refused before anything runs. | `busy` |
| A delegation still running thirty seconds past its stated minutes, or any job past ten minutes | The run is dropped, which ends the executor's process group, and the customer is told. The slot comes back. | `timed_out` |

Every refusal releases the job's slot, so `busy` is a statement about
jobs running now, never about jobs that failed earlier. `malformed`,
`stale`, and `timed_out` are this worker's codes; NIP-CJ lists its codes
as examples rather than a closed set, and the terminal carries any code
through as `refusal`.

Before a worker is deployed, `coder-worker --check` reads the same
configuration a run would and exits `78` when `CODER_WORKER_ALLOW` is
unset and `CODER_RELAY` is not a loopback address: an open worker on a
shared relay answers whoever finds its key.

## The label is not the job

The relay delivers each answer as `["EVENT", <subscription>, <event>]`,
and the subscription is the relay's own unsigned word. A relay that held
an old result — correctly signed by the worker, correctly encrypted to
this customer, `e`-tagged to a request this same keypair published an
hour ago — could deliver it under the current subscription, and a client
that trusted the label would render stale text. Audit finding A08 was
exactly that.

So the door binds the job on what the signature covers, not on the label.
Before a payload is read, the event must be kind `26900` or `27000`,
signed by the worker's key, `e`-tagged to the request this turn
published, and `p`-tagged to this identity. Only then is the event id
deduplicated — checking first would let a forged event claim a genuine
event's id and suppress it. The label itself is never consulted: a
correctly bound answer lands under any label, and nothing bound
elsewhere lands under the right one.

The decrypted payload then has to name a version this NIP defines — `1`
or `2` — and a `type` the event's kind actually carries. A `26900` event
claiming `status: error` is not a refusal, and a payload with no
meaningful type is not contact: `heard` moves to the long wait only once
a well-formed payload of a known type has arrived.

Ordering is versioned. Version `2` partials carry `seq`, a signed count
of the deltas before them; the door displays a delta only when it is the
next one, and the first `seq` that is not — early, late, or repeated —
closes the stream without buffering. Version `1` partials have no signed
order, so they are a liveness signal and never text; a version `1`
result still completes the job with the answer it carries whole. The
worker answers at the version the request named and declines anything
else with `unsupported_version`, so the ordering promise exists exactly
where the terminal can check it.

Two bounds keep a hostile stream finite rather than merely timed: a job
reads at most 1024 deduplicated events and renders at most 256 KiB of
deltas, and past either the turn is refused as a stream error. Deltas
are a preview, never the payload: the result is the answer whole, and a
result with no text fails as an empty answer rather than being completed
by whatever the stream happened to show before a gap.

Termination is otherwise unchanged: the first accepted result or
`status: error` ends the job, and later events for the same `e` tag are
ignored.

`crates/coder/tests/relay_binding.rs` exercises all of it against a
loopback relay that lies: a relabeled old result, an answer for another
job or another customer, missing `e` and `p` tags, a forged event
claiming a genuine result's id, duplicated and reordered partials,
unsequenced and legacy deltas, malformed payloads, wrong kinds, wrong
versions, event and byte floods, and the honest answer and refusal that
must still work. It needs no `CODER_RELAY` and skips nothing.

The 2026-09-20 local verification passed all 25 adversarial relay tests,
five relay unit tests, and two worker socket tests. The worker tests exercise
version negotiation and configured refusals through `answer` with a stub
door; they make no model request. The full Coder suite and strict Clippy
check also passed under Rust 1.97.1. The environment-gated `relay_job`
tests returned early without `CODER_RELAY`; this verification establishes
local protocol behavior, not a deployed version-2 measurement. Deployment
and its remote measurement remain tracked by #9435.

## What the relay could not see

This is the reason the transport is worth having rather than an
implementation detail.

NIP-CJ is ephemeral kinds with NIP-44 payloads. The relay fans the events
out to open subscriptions and stores none of them, and the only plaintext
on the wire is routing metadata: the kind, the `e` tag, the `p` tag. The
task, the transcript, the instructions, the streamed deltas, and the
finished answer are ciphertext between two keypairs. The relay holds no
job state, so there is no queue to drain and a retry is a new request.

Everything this page measures — the path, the timings, the prompts, the
answers — was written locally by the terminal. **The trace records what the
relay could not see.** The `npub` is the account, the relay is transport,
and the evidence is the customer's.

## Reproducing it

Run a worker. It needs an identity and a door of its own; the identity's
public key is what `CODER_WORKER` points at.

```sh
export CODER_WORKER_SECRET=$(openssl rand -hex 32)
export CODER_RELAY=wss://relay.openagents.com
export CODER_DOOR_KEY=…                 # the door the worker answers through
cargo run -p coder --bin coder-worker
# worker  3b332fd8…
```

Then run the turn twice, once each way. `CODER_DOOR_KEY` has to be unset
for the relay leg, and the run now says so rather than quietly routing
past it: an environment naming both a door key and a worker is refused
before the turn starts.

```sh
# direct
coder -p --json --trace direct.atif.jsonl "…"

# relay
env -u CODER_DOOR_KEY -u CODER_AI_GATEWAY_KEY \
  CODER_WORKER=3b332fd8… CODER_RELAY=wss://relay.openagents.com \
  coder -p --json --trace relay.atif.jsonl "…"
```

`--trace` refuses a path that already exists, so give each run its own
file. To watch a refusal instead of an answer, start the worker with
`--decline quota_exhausted`; to watch an absent one, stop the worker and
wait half a minute.

The NIP-CJ wire contract also has tests that run against a real relay
rather than a mock socket, skipped when `CODER_RELAY` is unset:

```sh
CODER_RELAY=wss://relay.openagents.com cargo test -p coder --test relay_job
```

Both passed against the production relay on 2026-09-19.

## Delegations over the relay

The runs above carry one generation per turn. On 2026-09-20 the fan-out
itself crossed the relay: `coderbench run devin-fan-out-six` with
`CODER_DELEGATE=devin-relay`, against a local `crates/nostr-relay` on
`ws://127.0.0.1:7447` and one `coder-worker` with `CODER_EXECUTOR=devin-local`
(the setup in [`worker-executor.md`](worker-executor.md)). The terminal
side ran under `env -i` with `PATH=/usr/bin:/bin`, an empty
`XDG_DATA_HOME`, and no worker secret: no Devin CLI, no capability
approval, no trusted workspace, no credentials on the driving host.

Result: `6 started, 6 verified`, no faults, 17.8 s wall clock for the
turn. The worker logged one probe and six jobs admitted at once, answered
in 9.4–13.7 s each. The same six tasks through the local `devin-local`
executor on the same host, in the direct run recorded at `198f11445`,
took 38 s wall clock, so the relay hop is not what the fan-out waits on;
the executor is.

The relay, at `NOSTR_RELAY_LOG_LEVEL=debug`, logs each ephemeral event it
admits (kind, ID, author, `e` and `p` tags, and the content's byte
length; never the content). Fourteen lines for the run, abbreviated:

```text
25900 03b77e9b… from 82c906a9 p=8f8c559a  132 B   # the probe
26900 85d68a92… from 8f8c559a e=03b77e9b p=82c906a9  304 B
25900 1e9359a3… from 82c906a9 p=8f8c559a  388 B   # six requests
25900 03973dcb… from 82c906a9 p=8f8c559a  388 B
25900 d6df2b8e… from 82c906a9 p=8f8c559a  432 B
25900 b05bbfa3… from 82c906a9 p=8f8c559a  388 B
25900 1d24a8f6… from 82c906a9 p=8f8c559a  388 B
25900 b9cdcf20… from 82c906a9 p=8f8c559a  432 B
26900 f1caec40… from 8f8c559a e=1e9359a3 p=82c906a9  220 B   # six results
26900 67c904cb… from 8f8c559a e=1d24a8f6 p=82c906a9  220 B
26900 c89873b8… from 8f8c559a e=b9cdcf20 p=82c906a9  220 B
26900 855e9e50… from 8f8c559a e=d6df2b8e p=82c906a9  260 B
26900 6cdc3b80… from 8f8c559a e=03973dcb p=82c906a9  220 B
26900 f5d946c7… from 8f8c559a e=b05bbfa3 p=82c906a9  220 B
```

`82c906a9` is the Coder identity, `8f8c559a` the worker. Every request is
tagged to the worker, every result is tagged to its request and to the
Coder identity, and every event's content is NIP-44 ciphertext: an answer
of `5` is 220 bytes on the wire. The six request IDs are the six `job`
lines in the worker's log and the six `relayed.request` values in the
trace. No kind `27000` events appear because the executor door produces
its answer in one piece; the trace records `"feedback": 0` for each.

**Worker-side bound.** The same run with the worker started as
`CODER_WORKER_JOBS=2`: two jobs admitted and answered, four `declined:
busy` in the worker's log, and in the trace four delegations with
`"status": "refused"`, `"refusal": "busy"`, and the worker's message
`this worker is running as many jobs as it admits at once`. CoderBench
reported `2 verified` and four faults, which is the correct grade for a
worker that could not take the work.

## Deployed

On 2026-09-20 the worker in [`deploy/README.md`](../../deploy/README.md)
ran as `coder-worker.service` on a GCE VM (`e2-standard-4`, Debian 12,
`us-central1-a`, beside the Cloud Run relay) at `0757355c1d`, with
`CODER_EXECUTOR=devin-local`, its own Devin CLI login, its own
`capability-trust` approval, `CODER_WORKER_JOBS=6`, and a `CODER_WORKER_ALLOW`
of one Coder public key. The driving machine was a different host on a
different network with `CODER_RELAY=wss://relay.openagents.com` and
`CODER_WORKER=2854d7da72ded5d6b62fa6107ff464129d510235c5017980c40a27cc9134f9ef`.

Six single turns, `coder -p --json "In one sentence, what does a Nostr
relay do?"`, each answered. Client wall clock against the worker's
`answered in` line:

| Turn | Client wall | Worker `answered in` | Round trip |
| --- | --- | --- | --- |
| 1 | 27.0 s | 21.9 s | 5.1 s |
| 2 | 22.7 s | 18.8 s | 3.9 s |
| 3 | 13.8 s | 9.8 s | 4.0 s |
| 4 | 19.4 s | 15.3 s | 4.1 s |
| 5 | 12.2 s | 8.3 s | 3.9 s |
| 6 | 12.0 s | 7.9 s | 4.1 s |

The client wall clock includes the terminal's own start (identity load,
Jev classification, trace open) as well as the relay hop each way, so the
round-trip column is an upper bound on the transport.

Then the fan-out, `coderbench run devin-fan-out-six` with
`CODER_DELEGATE=devin-relay` from a checkout at the task's pinned commit:
`6 started, 6 verified`, no faults, workspace unchanged, 23.4 s for the
turn. The worker's journal shows one `probed`, six `delegated: reading
task` admitted within two seconds of each other, and six `answered in`
lines of 10.1–18.8 s. Against the same six tasks through a local relay and
worker on one host (17.8 s, above), the deployed path costs about 6 s
more, which is the public relay and the VM's Devin CLI rather than
anything in Coder.

Two deployment defects surfaced and are fixed in the assets:

- A trust store at `/var/lib/coder-worker/capability-trust.json` made the
  boundary seal `/var/lib/coder-worker`, which contains the writable grant,
  and every job was refused `boundary_unavailable`. The store now lives
  under `/var/lib/coder-worker/.openagents/`.
- The base unit's `ProcSubset=pid` and `@system-service` syscall filter
  stopped `bwrap` (`Can't read /proc/sys/kernel/overflowuid`). The
  executor drop-in now sets `ProcSubset=all` and adds `@mount`.

## What this does not prove

- **One worker, one customer.** The deployed worker admits one Coder
  public key. Nothing here measures the relay under several customers or
  a worker under load beyond six concurrent jobs.
- **Wall clock, not p50.** Six turns and one fan-out are enough to place
  the round trip in the 4–5 s band; they are not a distribution.
- **The golden did not pass.** It failed identically on both transports,
  which is the transport claim and not a claim about the path Coder takes.
  Making `devin-fan-out-six` pass needs the program runtime,
  [#9409](https://github.com/OpenAgentsInc/openagents/issues/9409).
- **One prompt, one round.** The prompt was chosen to stay inside a single
  generation so the two transports could be compared without the shell
  loop's variance. A turn that runs commands crosses the relay once per
  round, and nothing here measures that.

## Related

- [NIP-CJ](../../nips/openagents/NIP-CJ.md) — the wire protocol.
- [`headless.md`](headless.md) — the flags, the JSON report, and the exit
  codes this page's tables use.
- [`traces.md`](traces.md) — what a trace holds and where it goes.
- [`../coderbench.md`](../coderbench.md) — the judge that read both
  transports the same way.
