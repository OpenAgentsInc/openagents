# 20. The API fans out

[Chapter 19](ch019-a-pylon-answers-decisions.md) made a computer answer a
signed, encrypted decision job. This chapter is the buyer that keeps the
HTTP interface from [chapter 16](ch016-serve-systemone.md). A caller sends
`POST /v1/systemone`; the adapter finds an available Pylon serving the
configured exact model identity, sends the decision through Nostr and
returns its checked response. Another eligible Pylon gets the next attempt
when the first cannot answer. A configured local model is the last door.

The executable is `psionic-fleet`. Its implementation is Coax in
[`psionic/lib/fleet/`](../lib/fleet/). The C host observes sockets, clocks,
arguments and stop signals. It does not choose a provider, admit a request,
interpret a relay frame or decide which answer can be published.

This is an opt-in loopback adapter, not a replacement for the deployed
public decision API. It uses fresh, throwaway buyer identities. The public
Oct 30 stage demonstration continues to use the
[stage buyer](../tests/pylon/DEMO.md) until a fleet rehearsal is recorded.
There is no production-key path or payment implementation here.

## One decision through the fleet

The caller keeps chapter 16's JSON body:

```text
{"model":"psionic-toy","state":"Water is wet.","questions":{"wet":{"type":"choice","instructions":"Is the water wet?","criteria":{"yes":null,"no":null}}}}
```

The adapter decodes that body before consulting a provider. It admits only
the base name of its configured identity. A request for another model gets
`not_admitted`, without a remote job. The first adapter accepts JSON bodies
up to 16 KiB, matching chapter 19's current Pylon runtime; the full wire
protocol's larger ceiling is not a promise that this executable accepts it.
It does not yet expose chapter 16b's protobuf ingress.

The persistent relay connection answers NIP-42 AUTH, subscribes to 30200
beacons and subscribes to live 26910/27010 answers addressed to its buyer
key. Every event's ID and BIP-340 signature verify in Coax. Beacon content
passes chapter 19's reader with its real SHA-256 digest.

A routable service must advertise the exact identity, the provider's
`pylon/decision` capability, `cj-decision`, `free-v1`, no offering and a null
or zero price. Offline, draining, full, expired and future beacons cannot
route. A lower generation or an old observation cannot replace a held
sample. A valid newer announcement withdrawing the service replaces the
old route with a tombstone. Unrelated services do not consume route slots.
The table holds at most 32 admitted coordinates, preserving their generation
history until the process stops; it is not an unbounded discovery database.

The adapter chooses the greatest advertised free count, with an earlier
slot breaking ties. It marks each attempted coordinate so that it is not
selected again for the same call. The next request can try it again. The
[runnable policy example](../examples/fleet-route.coax) shows these choices
without a network:

```shell
coax/bin/coax run psionic/examples/fleet-route.coax
```

```text
remote slot: 1
after failed attempt: -1
expired fleet: -1
local, same identity: true
local, other identity: false
```

## The answer belongs to this attempt

Each HTTP call gets an unpredictable request ID. Each selected provider
gets a new attempt number and a separately signed 25910 event, encrypted
with NIP-44. Only the admitted model, state and questions enter the job;
unknown HTTP fields are not forwarded to the Pylon. No request or response
content is logged.

An answer must verify under the selected provider's key and have exactly
one `p` tag naming the buyer and one `e` tag naming the exact job event. Its
decrypted wrapper must name this version, request and attempt. The model,
service identity, provider and `pylon:<slug>` door must match the chosen
beacon. A result from another generation or an expired chosen beacon is
not publishable.

Every asked question must appear once with its own type. Choice and score
probabilities name exactly the requested options, lie in `[0,1]`, and sum
to one within one part per million. The selected choice and confidence
match their probabilities, with lexical ties; the score matches the
weighted level. Noul remains a probability in `[0,1]`. The supported numeric
profile is the one Coax's serializer emits: ordinary decimal numbers with
up to nine fractional digits. Other encodings refuse rather than guess.
This validates the answer contract, not the truth of a model's decision.

The chapter 19 executable uses its already deployed result envelope,
`outcome: "answered"`. This adapter consumes that existing wire. NIP-DEC's
broader `completed`/ArtifactRef prose is not silently substituted for it;
reconciling that protocol projection requires an explicit versioned
migration. No new relay kind or protocol version is introduced here.

Both the relay's acceptance and the valid answer are required. Either may
arrive first. A signed provider error moves to the next candidate. A
missing ephemeral answer is a timeout, not proof that nobody ran it. The
adapter does not resend an ambiguous job or create a completion receipt.
Chapter 21 owns checks and receipts.

## Deadlines and the local door

`--timeout-ms` bounds the dispatch budget after HTTP admission;
`--attempt-ms` bounds one remote attempt, including encryption and signing.
A configured fallback keeps one second of that budget reserved. HTTP
reading has a separate five-second bound. Transport connect/flush effects
also retain the shared networking library's bounded waits. This is a
single-request loopback service, not a production ingress or a hard
real-time scheduler.

The local door is an explicitly configured loopback HTTP endpoint. Its
response passes the same question and identity checks and must name the
`psionic` door. It cannot replace one checkpoint with another just because
both have the same model name. If the fleet and local model cannot return
a valid answer, the adapter returns the existing typed `busy` or `timeout`
HTTP refusal. It never fabricates a successful decision.

## Run it

Build on macOS with Xcode command-line tools, or on Linux with the normal
Coax/OpenSSL-through-Nix prerequisites:

```shell
psionic/platform/fleet-build.sh /tmp/psionic-fleet plain
/tmp/psionic-fleet/psionic-fleet \
  --relay wss://relay.openagents.com --port 8798 \
  --identity psionic-toy@sha256:b6680707750842c489bad8a5524bb1b1393ff04f6917b72cf49c2cf87d633573 \
  --timeout-ms 8000 --attempt-ms 2000 --discover-ms 500 \
  --fallback http://localhost:8797
```

Omit `--fallback` to refuse when no remote Pylon can answer. Start a toy
engine and a Pylon using the [join instructions](../../apps/openagents-web/pylon/index.md).
Use the same JSON body above with `curl -H 'content-type: application/json'
--data-binary @request.json http://127.0.0.1:8798/v1/systemone`.
Ctrl-C stops the adapter. Its buyer key is regenerated on restart.

The [verification record](../../docs/verification/psionic-ch20-2026-10-30.md)
contains the actual checks and independent end-to-end results. The C host
has its own [Mac/Linux sanitizer record](../../docs/verification/psionic-fleet-host-2026-10-30.md).

## What this does not establish

A signed beacon is a provider's declaration. Matching its digest in an
answer is not remote attestation that those weights ran. A provider reads
the request it serves; NIP-44 hides the payload from the relay, not from the
provider. This first fleet discovers public, self-declared demo providers;
it has no production trust policy, ownership verification or standing score.
Owner-auth beacons and v2 model groups are unsupported. It uses one relay,
one in-flight HTTP request, no durable retry queue, and no money. A dropped
relay connection reconnects, but an in-flight ambiguous job is not replayed.
The adapter is not publicly deployed by this chapter.
