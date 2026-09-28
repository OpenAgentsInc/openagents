# Cloud fallback

A computer with nothing set up for Coder still answers. When no Codex login,
no Claude Code sign-in, or none with capacity is on the host, Microcoder's
loop generates each step through the OpenAgents cloud. Nobody supplies a
model key or a Vertex token for it, on any machine.

In the capacity book and `coder doctor` the cloud is the provider `vertex`,
because the cloud worker's model is served from Vertex. The host never talks
to Vertex or reads a Google credential.

## Where it sits

The [delegate door](delegate-door.md) lists Microcoder's providers in
preference order: the Codex login, then Claude Code's login, then the cloud.
The cloud is always on the list and always last, so a turn uses it only when
every provider before it is missing or out of capacity. A fresh host, with
no provider CLI installed or signed in, answers its first turn through it.

Two more rules keep it a fallback:

- When the cloud is all Microcoder has, an installed and signed-in Claude
  Code or Codex CLI answers the turn instead.
- An own door key (`CODER_DOOR_KEY` or `CODER_AI_GATEWAY_KEY`) takes the
  cloud off the list: the operator's own Open Responses door answers.

`CODER_CLOUD=off` takes it off a host. Any other value, or none, leaves it
on. `coder doctor` lists it as
`vertex openagents-cloud · connected, has capacity · the OpenAgents cloud
(worker 32c078952ff8… on wss://relay.openagents.com), signed by this host's
key; no token`.

Repository runs (`coder host autostart --route`) don't generate through the
cloud: it answers terminal and phone turns only.

## The path

The cloud is the same serving path as the OpenAgents app's basic chat
([chat worker](../../deployment/chat-worker.md)):

```text
host (its Nostr key) --NIP-CJ 25900, NIP-44--> relay.openagents.com --> chat worker --> AI gateway
host <--27000 partials, 26900 result---------- relay.openagents.com <-- chat worker
```

- Each loop step is one NIP-CJ conversation job. The request carries the
  step's prompt as the user turn and the loop's instructions, plus a line
  asking for exactly one JSON action matching the action schema, since a
  conversation job has no response format. The reply is read as the step's
  action.
- The job is signed by the host's own Nostr key, `~/.openagents/nostr-secret`,
  which Coder creates on first use. That key is the caller identity the
  worker meters.
- The worker's public key is compiled in (`coder::cloud::WORKER`, the same
  key the app carries). `CODER_CLOUD_WORKER` (npub or hex) and
  `CODER_CLOUD_RELAY` name another worker and relay, for a staging worker.
- The worker holds the model credential. The host pays nothing: each step's
  cost is recorded as $0.00, billed.

## Limits

The worker's limits apply to each host key, per job. A loop step is one job.

| Limit | Deployed value |
| --- | --- |
| Jobs per key in any 60 seconds | 6 |
| Jobs per key per UTC day | 40 |
| Jobs for every caller together per UTC day | 3,000 |
| Request size | 96 KiB |
| Jobs at once on the worker | 8 |

## Refusals

The worker's refusals are typed NIP-CJ codes. Three mean "not now" and
become capacity refusals in the book, so the loop fails over to the next
provider with capacity and the next turn skips the cloud until the reset:

| Code | Kind in the book | Holds until |
| --- | --- | --- |
| `rate_limited` | rate limit | the worker's `retry_after_ms`, else one minute |
| `busy` | rate limit | the worker's `retry_after_ms`, else one minute |
| `quota_exhausted` | usage limit | the worker's `retry_after_ms`, else the next UTC midnight |

Since the cloud is last, a quota refusal usually leaves no provider, and the
turn ends with the no-capacity sentence, for example "the OpenAgents cloud
is out of its usage limit until 2026-09-29 00:00 UTC".

Any other code is not a capacity refusal and is the step's error in plain
words: `limit_exceeded` says the step was too large to send, and
`not_admitted` or `internal` is reported with the worker's message. A relay
failure or a worker that doesn't answer is also a plain error.

## Checked by

- `the_cloud_fallback_needs_nothing_on_the_host`,
  `the_cloud_worker_codes_are_capacity_refusals_with_their_waits` in
  `crates/microcoder-loop`
- `a_fresh_host_completes_a_turn_through_the_cloud_fallback`,
  `the_cloud_is_always_the_last_provider_and_needs_no_configuration`,
  `the_cloud_answers_only_when_nothing_on_the_host_can`,
  `a_cloud_refusal_mid_turn_is_recorded_and_the_next_provider_finishes`,
  and the `cloud` module's tests in `crates/coder`

`INVARIANTS.md` (Coder cloud fallback) records the invariants.

## Research runs

`microcoder --provider vertex`, the research CLI, still reads an operator's
own Vertex token (`VERTEX_TOKEN_FILE` or `~/.openagents/vertex-token`) for
model studies. Coder never uses that path.
