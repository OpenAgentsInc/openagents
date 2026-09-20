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
| Client-observed minus worker-observed | 470 ms | 441 ms | 514 ms | 6 |

470 ms against the earlier 471 ms. The two runs are a day apart with
different code on the client, so the agreement is worth more than either
number: splitting the waits and carrying the model back did not move the
price of the transport.

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

## What this does not prove

- **No worker is deployed.** `relay.openagents.com` had nothing listening
  for kind `25900` before this proof and has nothing listening after it.
  The worker that answered was `coder-worker` run from a laptop for the
  length of the measurement. The transport is proven; the service is not
  deployed.
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
