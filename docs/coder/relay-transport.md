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
{"record":"session","session":{"model":"relay","door":"relay", …}}
```

A run whose evidence cannot say how it was routed cannot be compared
against one routed differently, so this field is load-bearing rather than
decorative.

One gap: on the relay run `model` reads `relay` rather than the model the
worker actually used. The NIP-CJ result carries a `model` field and the
worker fills it in; the door drops it, and the session header is written
before any worker has answered anyway. The relay trace therefore says how
the turn was routed but not what answered at the far end.

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

## The three refusal causes

A relay that is unreachable, a worker that is absent, and a worker that
declined are three different states. `gym::eval::classify` draws the line
that matters: a typed refusal is an answer, a failure with no code is the
harness. `coder -p --json` reports `cause` and `refusal` as fields, so a
harness reads them rather than matching on prose.

| State | `cause` | `refusal` | Exit | How long | What `error` says |
| --- | --- | --- | --- | --- | --- |
| The relay is not there | `relay_unreachable` | `null` | `1` | 0.9 s | `relay: connect: IO error: Connection refused (os error 61)` |
| No worker is listening | `worker_silent` | `null` | `1` | 181 s | `no worker answered: nothing came back from <pubkey> in 180 seconds` |
| A worker declined | `worker_declined` | `quota_exhausted` | `1` | 1.7 s | `the worker declined (quota_exhausted): …` |

All three exit `1`, because in all three the turn did not finish and there
is no reply to show. What separates them is the field, not the code, and
the trace records it too: a failed turn writes a `System` step saying
`the turn did not finish (<cause>): <reason>` before the log closes, so a
reader holding only the trace still learns why.

The `worker_silent` row is the expensive one. An absent worker costs the
full 180-second `TURN_TIMEOUT` before the client gives up, and the client
cannot tell an absent worker from a slow one, because NIP-CJ's kinds are
ephemeral and an unanswered request leaves nothing behind to ask about. A
worker that published a `status: queued` feedback event promptly would let
the terminal fail in seconds instead of minutes; nothing in the protocol
requires one to.

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
for the relay leg: `Door::from_env` prefers an own-key door and would
ignore `CODER_WORKER` otherwise.

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
wait three minutes.

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
