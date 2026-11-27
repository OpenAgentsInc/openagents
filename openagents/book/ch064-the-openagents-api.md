# 64. The OpenAgents API

> **Status: partly implemented.** The `#authority` HTTP caller authentication
> policy ([`coax/web/api/caller-policy.coax`](../../coax/web/api/caller-policy.coax)),
> NIP-98 verifier ([`coax/web/api/nip98.coax`](../../coax/web/api/nip98.coax)),
> `#authority` agent grant admission policy ([`coax/web/api/agent-policy.coax`](../../coax/web/api/agent-policy.coax)),
> and generated `#import_proto` binary grant codec
> ([`coax/web/api/agent-wire.coax`](../../coax/web/api/agent-wire.coax)) run on
> `cpu`, grounded by
> [`openagents/examples/api-caller-grant-admission.coax`](../examples/api-caller-grant-admission.coax)
> and [`openagents/examples/api-caller-grant-admission.out`](../examples/api-caller-grant-admission.out),
> and are consumed by [`coax/web/api/seats.c`](../../coax/web/api/seats.c),
> [`coax/web/api/room.c`](../../coax/web/api/room.c),
> [`apps/openagents-cli/agent.coax`](../../apps/openagents-cli/agent.coax),
> and [`apps/openagents-web/agents.coax`](../../apps/openagents-web/agents.coax).
> Remaining: moving legacy hand-packed site routes onto the shared `openagents.api.v1`
> envelope, generating `openagents.discovery.v1` `ApiDocument`, and shipping a
> loopback partner-app sample stream.

## Premise

The protocol is open to anyone who speaks it. This chapter adds the easier
door for products that only want the result: one API, the same one our own
apps use, that a partner app can build on. It is a second carriage of the same
schema, not a second protocol.

## Draft

Episode 289 says the open ecosystem should result in very simple interfaces.
You go to openagents.com, you load the mobile app, or "you use a partner app
consuming the OpenAgents API". This is promise P13.

A partner app is someone else's product with our agent inside it. Its
developer does not want to run relays or read event kinds. They want to send a
message and get back an answer, a plan to approve, or a stream of work.

So there is one API, over HTTP and WebSocket. A request carries a message and
its context. The reply streams what the runtime produces: the route, the plan,
any approval to ask the person for, each step and the result. A session, an
agent's API key or a signed request identifies the caller, and an agent's key
carries its grant (chapter 5). Limits are published. The API is described in
a machine-readable file, so a partner's coding agent can write the client.

The bodies come from the same schema as the events. A request and each item in
the reply stream are the JSON form of the messages chapter 63 lists, inside
one envelope for errors, pages and repeat-safe requests. Where a record is
also a NIP body, the API's JSON and the event's content are the same JSON. The
plan is to generate the machine-readable file from the schema, so the two
can't drift.

There is a third carriage, and part of the API already uses it. Agent grants
travel as binary protobuf, written by a codec that Coax generates from the
schema. Binary suits frames, files and calls where both ends are ours. Chapter
65 explains the three together.

Today the **OpenAgents Cloud inference, pricing, and usage API** is live in
`coax/web/api/` (`#14069`–`#14072`): retail pricing tables, cached-token and
decision billing, and usage sync endpoints are specified in
[`proto/openagents/api/v1alpha1/inference.proto`](../../proto/openagents/api/v1alpha1/inference.proto)
and backed by [`coax/web/api/inference.c`](../../coax/web/api/inference.c),
[`coax/web/api/router.c`](../../coax/web/api/router.c), and
[`coax/web/api/migrations/0216_inference_openagents_pricing.sql`](../../coax/web/api/migrations/0216_inference_openagents_pricing.sql).
Most older site routes still write hand-packed JSON; the [compliance
plan](../../docs/protocol/2026-11-11-compliance-plan.md) orders the move onto
the shared envelope.

The rule that keeps the API honest is that our own apps use it. Chapter 4's
chat service is behind it. There is no private API with more in it.

Money flows through as it does everywhere else. A partner's users can make
paid calls, plugin authors are paid from them, and the partner can add members
of its own to the same registry.

A partner need not take everything. Episode 289 says "or a subset of it they
care about". The API is grouped so that a product can use answers alone, or
Coder alone, or the registry alone.

[Episode 85](../../docs/transcripts/085.md) designs the first version of this,
joined to the store and to builder payments.

## What we build

A plan for what this chapter ships once the draft is approved:

- The API over HTTP and WebSocket: a message in, a typed stream of route,
  plan, approvals, steps and result out.
- Keys, published limits and a machine-readable description.
- A sample partner app, small enough to read in one sitting.

It builds on what exists today:

- [`proto/openagents/api/v1alpha1/inference.proto`](../../proto/openagents/api/v1alpha1/inference.proto), [`coax/web/api/inference.c`](../../coax/web/api/inference.c), [`coax/web/api/router.c`](../../coax/web/api/router.c), and [`coax/web/api/migrations/0216_inference_openagents_pricing.sql`](../../coax/web/api/migrations/0216_inference_openagents_pricing.sql) (OpenAgents Cloud retail pricing, cached tokens, decision billing & usage APIs, `#14069`)
- [`apps/openagents-cli/api.coax`](../../apps/openagents-cli/api.coax)
- [`proto/README.md`](../../proto/README.md)
- [`docs/protocol/openagents-protocol-spec.md`](../../docs/protocol/openagents-protocol-spec.md)
- [`coax/web/api/agent-wire.coax`](../../coax/web/api/agent-wire.coax)
- [`coax/web/api/nip98.coax`](../../coax/web/api/nip98.coax)
- [`coax/lib/net`](../../coax/lib/net)
- [`coax/lib/gateway/sse.coax`](../../coax/lib/gateway/sse.coax)
- [`coax/lib/access`](../../coax/lib/access)

On the protocol ([chapter 65](ch065-one-schema-three-carriages.md)):

| Record | Schema | Carriage | Today |
| --- | --- | --- | --- |
| The envelope: errors, pages, repeat-safe requests | `openagents.api.v1` ([spec 10](../../docs/protocol/openagents-protocol-spec.md#10-openagentsapiv1-the-http-envelope)) | HTTP JSON | Spec sketch; no `.proto` file. Each route writes its own JSON |
| A message in and the stream out | `openagents.chat.v1`; for hosted runs, planned `openagents.agent.managed.v1alpha1` events ([spec 17.1](../../docs/protocol/openagents-protocol-spec.md#171-openagentschatv1), [spec 18.2](../../docs/protocol/openagents-protocol-spec.md#182-openagentsagentmanagedv1); [compliance plan](../../docs/protocol/2026-11-11-compliance-plan.md), step C5) | HTTP JSON and a stream | Hand-written JSON |
| Who is calling | A NIP-98 signed event, an `oa_key_` API key or a session ([sessions and tokens](../../docs/auth/sessions-and-tokens.md)) | HTTP headers | Built. NIP-98 is an external format |
| Agent grants | `openagents.account.v1alpha1` ([`agents.proto`](../../proto/openagents/account/v1alpha1/agents.proto)) | HTTP, as binary protobuf | In use: the generated codec `coax/web/api/agent-wire.coax` |
| The machine-readable description | `openagents.discovery.v1` `ApiDocument` ([spec 30.3](../../docs/protocol/openagents-protocol-spec.md#303-openagentsdiscoveryv1)), generated from the schema | HTTP | Planned |

### The chapter's check

- **Source:** [`coax/web/api/caller-policy.coax`](../../coax/web/api/caller-policy.coax),
  [`coax/web/api/agent-policy.coax`](../../coax/web/api/agent-policy.coax),
  [`coax/web/api/agent-wire.coax`](../../coax/web/api/agent-wire.coax),
  [`openagents/examples/api-caller-grant-admission.coax`](../examples/api-caller-grant-admission.coax),
  and [`openagents/examples/api-caller-grant-admission.out`](../examples/api-caller-grant-admission.out)
  (remaining: loopback partner-app SSE approval stream over `openagents.api.v1`).
- **Trace:** admits unsigned reads as `CP_PLAYER`, verified NIP-98 reads and
  body-bound writes as `CP_SIGNER` (`record_seen=true`), round-trips a `46`-byte
  protobuf `openagents.account.v1alpha1.GrantRequest` (`verdict=0`,
  `name=Partner Runner`), matches CIDR `203.0.113.0/24`, and admits a `200`-sat
  spend within per-payment (`250`) and 24-hour (`1000`) caps.
- **Refusal:** forged (`CP_BAD_SIG`), body-unbound write (`CP_UNBOUND`), and
  replayed (`CP_REPLAY`) NIP-98 requests refuse without falling back to the
  player key; over-payment (`403` `AG_OVER_PAYMENT`), over-day (`AG_OVER_DAY`),
  missing scope (`AG_NO_SCOPE`), revoked grant (`401` `AG_NOT_ACTIVE`),
  unlisted IP (`AG_WRONG_HOST`), and corrupt protobuf bytes (`AG_BAD_WIRE`)
  all fail closed.
- **Independent check:** `coax/bin/coax run openagents/examples/api-caller-grant-admission.coax`
  and `--release` match [`openagents/examples/api-caller-grant-admission.out`](../examples/api-caller-grant-admission.out),
  agent grant bytes decode under Python protobuf against
  [`agents.proto`](../../proto/openagents/account/v1alpha1/agents.proto), and
  NIP-98 tags match [`nip98_tags.py`](../../coax/tests/nostr/nip98_tags.py).

Planned paths: none. The API is [`coax/web/api/`](../../coax/web/api), and new
rules follow its [COAX.md](../../coax/web/api/COAX.md).

## Sources

- **Episodes:**
  - [Episode 289, *OpenAgents*](../../docs/transcripts/289.md): 3:00, partner apps on the API
  - [Episode 85, *API Design*](../../docs/transcripts/085.md)
  - [Episode 169, *Agent Payments API*](../../docs/transcripts/169.md): an agent payments API
  - [Episode 242, *Khala, Collective Intelligence*](../../docs/transcripts/242.md): an API other tools can call
- **Old code at `coder-v1.0.0`, crates:** `gateway`, `oa-auth`, `oa-tokens`, `push-gateway`, `tenancy`, `coder-sdk`, `coder-service`
- **NIPs:** [NIP-CJ](../../nostr/nips/openagents/NIP-CJ.md), [NIP-AGENT](../../nostr/nips/openagents/NIP-AGENT.md), [NIP-KEYS](../../nostr/nips/openagents/NIP-KEYS.md)
- **Also:**
  - [The OpenAgents Protocol](../../docs/protocol/openagents-protocol-spec.md), sections 8, 10 and 31
- **Schemas:** [`proto/`](../../proto/README.md) defines every wire shape, by the [protocol specification](../../docs/protocol/openagents-protocol-spec.md); [`proto/registry.json`](../../proto/registry.json) lists each shape and its one codec
- **Promises covered** ([audit](../../docs/audits/2026-11-10-episode-289-port-audit.md#1-what-episode-289-promised)): P13

## Open questions for the owner

1. Does the API include an endpoint in the shape other vendors' clients
   already speak, so existing tools can point at it?
2. What does API use cost a partner, and how does that sit with the author's
   share of paid calls?

## Source listing

Run the Chapter 64 example in [`openagents/examples/api-caller-grant-admission.coax`](../examples/api-caller-grant-admission.coax) (committed transcript [`openagents/examples/api-caller-grant-admission.out`](../examples/api-caller-grant-admission.out)):

<!-- source: openagents/examples/api-caller-grant-admission.coax api-caller-grant-admission -->
```jai
acga_run :: () -> bool {
  // 1. Caller authentication: unsigned -> player key; verified bound write -> signer + record seen ID.
  c_unsigned := cp_caller(false, false, true, false, false);
  c_get := cp_caller(true, true, true, false, true);
  c_post := cp_caller(true, true, false, true, true);
  wants_seen := cp_wants_seen(true, true, false, true);
  print("caller unsigned=% get=% post=% record_seen=%\n",
    c_unsigned == CP_PLAYER, c_get == CP_SIGNER, c_post == CP_SIGNER, wants_seen);

  // 2. Round-trip an openagents.account.v1alpha1 GrantRequest via generated protobuf codec + policy check.
  q: AwRequest;
  q.name_n = aw_text(q.name[..], "Partner Runner");
  q.mask = ag_bit(AG_PROFILE_READ) | ag_bit(AG_WALLET_SPEND);
  q.per_day = 1000;
  q.per_payment = 250;
  q.days = 30;
  q.hosts[0].n = aw_text(q.hosts[0].b[..], "203.0.113.0/24");
  q.nhosts = 1;

  wire: [AW_MAX] u8;
  wn := aw_request_write(q, wire[..]);
  dec := aw_request_read(wire[0..<wn]);
  host_ok := ag_host_match("203.0.113.7", cast(string) dec.hosts[0].b[0..<dec.hosts[0].n]);
  st_active := ag_state(1000, 1000 + 30 * AG_DAY_MS, 0, false, false);
  v_agent := ag_agent(st_active, dec.nhosts, host_ok);
  v_scope := ag_scope(dec.mask, AG_WALLET_SPEND);
  v_spend := ag_spend(dec.mask, 200, dec.per_payment, dec.per_day, 500);
  print("grant wire_bytes=% verdict=% name=% host_ok=% agent=% scope=% spend=%\n",
    wn, dec.verdict, cast(string) dec.name[0..<dec.name_n], host_ok,
    v_agent == AG_OK, v_scope == AG_OK, v_spend == AG_OK);

  // 3. Fail-closed refusals: forged/unbound/replayed NIP-98, over-payment/over-day spend,
  // missing scope, revoked grant, unlisted IP, and truncated protobuf wire.
  ref_sig := cp_caller(true, false, true, false, true);
  ref_unbound := cp_caller(true, true, false, false, true);
  ref_replay := cp_caller(true, true, false, true, false);
  ref_pay := ag_spend(dec.mask, 251, dec.per_payment, dec.per_day, 0);
  ref_day := ag_spend(dec.mask, 200, dec.per_payment, dec.per_day, 900);
  ref_noscope := ag_scope(ag_bit(AG_PROFILE_READ), AG_WALLET_SPEND);
  ref_revoked := ag_agent(ag_state(1000, 999999999, 500, false, false), 0, true);
  ref_host := ag_agent(AG_ACTIVE, 1, ag_host_match("198.51.100.9", "203.0.113.0/24"));
  bad_wire := [2] u8.[0x0a, 0x7f];
  ref_wire := aw_request_read(bad_wire[..]).verdict;
  print("refuse nip98=(%,%,%) grant=(pay=%,day=%,scope=%,revoked=%,host=%,wire=%)\n",
    ref_sig == CP_BAD_SIG && cp_refusal_text(ref_sig).count > 0,
    ref_unbound == CP_UNBOUND, ref_replay == CP_REPLAY,
    ref_pay == AG_OVER_PAYMENT && ag_status(ref_pay) == 403,
    ref_day == AG_OVER_DAY, ref_noscope == AG_NO_SCOPE,
    ref_revoked == AG_NOT_ACTIVE && ag_status(ref_revoked) == 401,
    ref_host == AG_WRONG_HOST, ref_wire == AG_BAD_WIRE);

  return c_unsigned == CP_PLAYER && c_get == CP_SIGNER && c_post == CP_SIGNER && wants_seen &&
         wn > 0 && dec.verdict == AG_OK && host_ok &&
         v_agent == AG_OK && v_scope == AG_OK && v_spend == AG_OK &&
         ref_sig == CP_BAD_SIG && ref_unbound == CP_UNBOUND && ref_replay == CP_REPLAY &&
         ref_pay == AG_OVER_PAYMENT && ref_day == AG_OVER_DAY && ref_noscope == AG_NO_SCOPE &&
         ref_revoked == AG_NOT_ACTIVE && ref_host == AG_WRONG_HOST && ref_wire == AG_BAD_WIRE;
}
```

Output (`coax/bin/coax run openagents/examples/api-caller-grant-admission.coax`, identical in `--release`):

```text
caller unsigned=true get=true post=true record_seen=true
grant wire_bytes=46 verdict=0 name=Partner Runner host_ok=true agent=true scope=true spend=true
refuse nip98=(true,true,true) grant=(pay=true,day=true,scope=true,revoked=true,host=true,wire=true)
ok=true
```
