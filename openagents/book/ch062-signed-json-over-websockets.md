# 62. Signed JSON over WebSockets

> **Status: partly implemented.** The `#authority` NIP-01 event reader and
> BIP-340 verifier ([`coax/lib/nostr/event.coax`](../../coax/lib/nostr/event.coax),
> [`event_sign.coax`](../../coax/lib/nostr/event_sign.coax)), relay frame codec
> ([`coax/lib/nostr/message.coax`](../../coax/lib/nostr/message.coax)), and
> relay client state machine ([`coax/lib/nostr/relay.coax`](../../coax/lib/nostr/relay.coax))
> run on `cpu`, grounded by
> [`openagents/examples/nostr-signed-relay-flow.coax`](../examples/nostr-signed-relay-flow.coax)
> and [`openagents/examples/nostr-signed-relay-flow.out`](../examples/nostr-signed-relay-flow.out),
> and are consumed by [`apps/openagents-cli/relay.coax`](../../apps/openagents-cli/relay.coax),
> [`apps/shared/nostr-feed.coax`](../../apps/shared/nostr-feed.coax),
> and [`psionic/lib/pylon/run.coax`](../../psionic/lib/pylon/run.coax).
> Remaining: publishing the operated OpenAgents agent-records relay limits table
> and generating the complete data-class list directly from `proto/`.

## Premise

Earlier chapters publish events without saying much about what carries them.
This chapter states the open carriage under everything public: signed JSON,
sent over WebSockets, through relays that anyone can run. It is one of three
ways a record travels, and all three carry the same schema.

## Draft

[Episode 289](../../docs/transcripts/289.md): "No blockchain garbage, just
data, signed JSON over WebSockets. Very easy."

That sentence is the public face of the protocol. An event is a small JSON
object: who wrote it, when, what kind it is, some tags, the content, and a
signature over all of it.
A relay is a server that accepts events and hands them to whoever asks. A
client connects to relays over a WebSocket, publishes events and subscribes to
the ones it wants. This is Nostr. It has no chain, no token and no global
agreement to wait for.

Three properties matter for an agent. An event is signed, so you know who said
it without trusting the relay that gave it to you. An event is plain data, so
any program in any language can read it. And relays can be replaced: the same
event can sit on many, and losing one loses nothing.

The archive comes back to this protocol many times: a plugin registry in
[Episode 66](../../docs/transcripts/066.md), login in [Episode
82](../../docs/transcripts/082.md), a client in [Episode
177](../../docs/transcripts/177.md), and negotiated markets in [Episode
266](../../docs/transcripts/266.md). Each time the reason is the same. A
platform can change its rules or disappear, and work built on it goes with it.

Where does an event's content come from? Each record in this book has one
definition: a protobuf message in [`proto/`](../../proto/README.md). Most
are still sketches in the spec, and chapter 65 lists which exist. The
message's JSON form goes in the event's content, and that JSON is what gets
signed. The same message is the JSON body of an API call (chapter 64). It can
also travel as binary frames where both ends are ours and size matters, as in
a realm session or sync. [Section 8 of the protocol
spec](../../docs/protocol/openagents-protocol-spec.md#8-one-message-three-carriages-nostr-http-and-rpc)
calls this one message, three carriages, and chapter 65 walks through it.

Signed JSON over WebSockets is the carriage for anything public or signed. Any
program can read it, and anyone can check who said it.

Today most of that JSON is written by hand, one Coax library for each NIP.
Coax generates binary codecs from a schema, and JSON ones are planned. Chapter
52 says when a hand-written codec is allowed.

Nostr From Scratch builds the carriage in Coax: the event model, a client, a
relay and the NIPs. This book uses that library and does not rebuild it. What
this chapter adds is the short version, and the relay we run for the agent's
own records.

## What we build

A plan for what this chapter ships once the draft is approved:

- A short tour that publishes one event and reads it back from two relays,
  using the existing library.
- The relay we run for agent records, with its limits published.
- A list of which records are public events and which are private, encrypted
  ones, taken from each schema's data class
  ([spec 6](../../docs/protocol/openagents-protocol-spec.md#6-data-classes-descriptor-options-and-authority)).

It builds on what exists today:

- [`coax/lib/nostr/event.coax`](../../coax/lib/nostr/event.coax)
- [`coax/lib/nostr/relay.coax`](../../coax/lib/nostr/relay.coax)
- [`nostr/relay/relay_server.coax`](../../nostr/relay/relay_server.coax)
- [`coax/lib/nostr/nip59.coax`](../../coax/lib/nostr/nip59.coax)
- [`nostr/book/README.md`](../../nostr/book/README.md)
- [`apps/openagents-cli/relay.coax`](../../apps/openagents-cli/relay.coax)
- [`proto/nostr/README.md`](../../proto/nostr/README.md)

On the protocol ([chapter 65](ch065-one-schema-three-carriages.md)):

| Record | Schema | Carriage | Today |
| --- | --- | --- | --- |
| The event, a filter and the relay messages | `nostr.event.v1alpha2` `SignedEvent`, `Filter`, `ClientFrame`, `RelayFrame` ([`event.proto`](../../proto/nostr/event/v1alpha2/event.proto)). NIP-01 is the definition | Nostr: JSON over a WebSocket | Schema file with a generated Coax codec, tested against fixture bytes. It is a typed mirror: the signed JSON stays the authority, and `coax/lib/nostr` checks a signature over the bytes it received |
| A private record | `envelope.v1alpha1` `ArtifactEnvelope` ([spec 8.3](../../docs/protocol/openagents-protocol-spec.md#83-encrypted-carriages)) | Nostr kind 3188, encrypted with NIP-44 | Spec sketch; no `.proto` file. `coax/lib/nostr/envelope.coax` writes the JSON |
| Any record in this book | One message in `proto/` ([spec 8.1](../../docs/protocol/openagents-protocol-spec.md#81-the-rule)) | Nostr, HTTP or binary frames | See each chapter's table and [chapter 65](ch065-one-schema-three-carriages.md) |

### The chapter's check

- **Source:** [`coax/lib/nostr/event.coax`](../../coax/lib/nostr/event.coax),
  [`coax/lib/nostr/message.coax`](../../coax/lib/nostr/message.coax),
  [`coax/lib/nostr/relay.coax`](../../coax/lib/nostr/relay.coax),
  [`openagents/examples/nostr-signed-relay-flow.coax`](../examples/nostr-signed-relay-flow.coax),
  and [`openagents/examples/nostr-signed-relay-flow.out`](../examples/nostr-signed-relay-flow.out)
  (remaining: live dual-relay tour over [`relay_server.coax`](../../nostr/relay/relay_server.coax)).
- **Trace:** reads and BIP-340-verifies a captured kind `30617` relay `EVENT`
  frame (`verified=true`), writes a `412`-byte client `["EVENT",...]` frame,
  reads `["OK",<id>,true,""]`, and steps `Relay` through `REQ`, `EVENT` publish,
  `RL_ACK`, `RL_DELIVER`, `RL_DUPLICATE`, and `RL_EOSE`.
- **Refusal:** flipping one hex nibble in an event ID fails
  `nostr_message_event_verify_vartime` with `NE_ID` (`verified=false`); an
  unverified event fails `rl_input` with `RL_FORGED` (`forged=1`); a stale
  connection generation fails with `RL_STALE`; an unknown subscription fails
  with `RL_UNKNOWN`; a prefixless refusal `OK` fails with `NM_MESSAGE`.
- **Independent check:** `coax/bin/coax run openagents/examples/nostr-signed-relay-flow.coax`
  and `--release` match [`openagents/examples/nostr-signed-relay-flow.out`](../examples/nostr-signed-relay-flow.out),
  and IDs/signatures/relay behavior are independently checked by
  [`event_reference.cjs`](../../coax/tests/nostr/event_reference.cjs),
  [`event_sign_vectors.py`](../../coax/tests/nostr/event_sign_vectors.py),
  and [`relay_rust_differential.py`](../../coax/tests/nostr/relay_rust_differential.py).

Planned paths: none. The carriage is [`coax/lib/nostr`](../../coax/lib/nostr)
and the relay in [`nostr/relay`](../../nostr/relay).

## Sources

- **Episodes:**
  - [Episode 289, *OpenAgents*](../../docs/transcripts/289.md): 2:30, signed JSON over WebSockets
  - [Episode 288, *Three DevDays Later*](../../docs/transcripts/288.md): decentralized, permissionless protocols
  - [Episode 66, *Nostr Plugin Registry*](../../docs/transcripts/066.md): a registry on Nostr
  - [Episode 82, *Nostr Login*](../../docs/transcripts/082.md)
  - [Episode 177, *Commander as Nostr Client*](../../docs/transcripts/177.md): a Nostr client
  - [Episode 266, *Single Points of Failure and Nostr Markets*](../../docs/transcripts/266.md): single points of failure
  - [Episode 267, *Immortal Infrastructure*](../../docs/transcripts/267.md): separate relay, provider and client roles
- **Old code at `coder-v1.0.0`, crates:** `nostr`, `nostr-relay`, `nostr-transport`
- **NIPs:** [NIP-REG](../../nostr/nips/openagents/NIP-REG.md), [NIP-AGENT](../../nostr/nips/openagents/NIP-AGENT.md)
- **Also:**
  - [Nostr From Scratch](../../nostr/book/README.md)
  - [The OpenAgents NIPs](../../nostr/nips/openagents/README.md)
  - [The OpenAgents Protocol](../../docs/protocol/openagents-protocol-spec.md), section 8, and [`proto/`](../../proto/README.md)
- **Schemas:** [`proto/`](../../proto/README.md) defines every wire shape, by the [protocol specification](../../docs/protocol/openagents-protocol-spec.md); [`proto/registry.json`](../../proto/registry.json) lists each shape and its one codec
- **Promises covered** ([audit](../../docs/audits/2026-11-10-episode-289-port-audit.md#1-what-episode-289-promised)): P12

## Source listing

Run the Chapter 62 example in [`openagents/examples/nostr-signed-relay-flow.coax`](../examples/nostr-signed-relay-flow.coax) (committed transcript [`openagents/examples/nostr-signed-relay-flow.out`](../examples/nostr-signed-relay-flow.out)):

<!-- source: openagents/examples/nostr-signed-relay-flow.coax nostr-signed-relay-flow -->
```jai
nsrf_run :: () -> bool {
  // 1. Read & BIP-340-verify a real captured relay EVENT frame + wrap a client EVENT frame.
  line1 := nm_line(COAX_RELAY, 1);
  frame1 := COAX_RELAY[line1.from..<line1.to];
  m_ev := nostr_message_read(frame1);
  scratch: [4096] u8;
  ev := nostr_message_event_verify_vartime(frame1, scratch[..], false);

  ev_out: [4096] u8;
  ev_n := nostr_message_write_event(NE_FIXTURE, ev_out[..], false);
  ok_frame := nostr_message_read("[\"OK\",\"b1a649ebe8b435ec71d3784793f3bbf4b93e64e17568a741aecd4c7ddeafce30\",true,\"\"]");
  print("verify ok=% verified=% kind=% sub=% client_frame_bytes=% ok_accepted=%\n",
    m_ev.error == NM_OK && ev.error == NE_OK, ev.verified, ev.kind,
    nm_text(m_ev, m_ev.sub), ev_n, ok_frame.error == NM_OK && ok_frame.accepted);

  // 2. Drive the #authority Relay state machine: subscribe, publish, ack OK, deliver, dedup, EOSE.
  r: [1] Relay;
  sub_slot := rl_subscribe(r[..]);
  pub_slot := rl_publish(r[..], ev.id[..]);
  gen := rl_start(r[..], 100);
  opened := rl_opened(r[..], gen, 200);
  out_req := rl_next(r[..], 200);
  out_pub := rl_next(r[..], 250);
  sub_buf: [24] u8;
  sub_id := rl_sub_text(sub_slot, 1, sub_buf[..]);

  d_ack := rl_input(r[..], gen, RlIn.{kind = RL_IN_OK, id = ev.id, accepted = true});
  d_ev1 := rl_input(r[..], gen, RlIn.{kind = RL_IN_EVENT, sub = sub_id, id = ev.id, created_at = ev.created_at, verified = true});
  d_dup := rl_input(r[..], gen, RlIn.{kind = RL_IN_EVENT, sub = sub_id, id = ev.id, created_at = ev.created_at, verified = true});
  d_eose := rl_input(r[..], gen, RlIn.{kind = RL_IN_EOSE, sub = sub_id});
  print("relay open=% out=(req=%,ev=%) sub=% ack=% deliver=% dup=% eose=%\n",
    opened, out_req.kind == RL_OUT_REQ, out_pub.kind == RL_OUT_EVENT,
    sub_id, d_ack == RL_ACK, d_ev1 == RL_DELIVER, d_dup == RL_DUPLICATE, d_eose == RL_EOSE);

  // 3. Fail-closed refusals: tampered event ID, unverified/forged relay event, stale gen,
  // unknown sub, and refusal OK without machine prefix.
  tampered: [1024] u8;
  tn := line1.to - line1.from;
  for i: 0..<tn { tampered[i] = COAX_RELAY[line1.from + i]; }
  tampered[23] = '9'; // flip first hex nibble of event id
  ev_tamper := nostr_message_event_verify_vartime(cast(string) tampered[0..<tn], scratch[..], false);

  forged_id := ev.id;
  forged_id[0] ^= 1;
  d_forged := rl_input(r[..], gen, RlIn.{kind = RL_IN_EVENT, sub = sub_id, id = forged_id, created_at = ev.created_at, verified = false});
  d_stale := rl_input(r[..], gen - 1, RlIn.{kind = RL_IN_EVENT, sub = sub_id, id = forged_id, created_at = ev.created_at, verified = true});
  d_unk := rl_input(r[..], gen, RlIn.{kind = RL_IN_EVENT, sub = "s9.1", id = forged_id, created_at = ev.created_at, verified = true});
  bad_ok := nostr_message_read("[\"OK\",\"b1a649ebe8b435ec71d3784793f3bbf4b93e64e17568a741aecd4c7ddeafce30\",false,\"no prefix\"]");
  print("refuse tamper_id=% forged=% stale=% unk_sub=% bad_ok=%\n",
    ev_tamper.error == NE_ID && !ev_tamper.verified,
    d_forged == RL_FORGED && r[0].forged == 1,
    d_stale == RL_STALE, d_unk == RL_UNKNOWN, bad_ok.error == NM_MESSAGE);

  return m_ev.error == NM_OK && ev.error == NE_OK && ev.verified && ev_n == NE_FIXTURE.count + 10 &&
         ok_frame.error == NM_OK && ok_frame.accepted && pub_slot == 0 && opened &&
         out_req.kind == RL_OUT_REQ && out_pub.kind == RL_OUT_EVENT &&
         d_ack == RL_ACK && d_ev1 == RL_DELIVER && d_dup == RL_DUPLICATE && d_eose == RL_EOSE &&
         ev_tamper.error == NE_ID && !ev_tamper.verified &&
         d_forged == RL_FORGED && r[0].forged == 1 && d_stale == RL_STALE &&
         d_unk == RL_UNKNOWN && bad_ok.error == NM_MESSAGE;
}
```

Output (`coax/bin/coax run openagents/examples/nostr-signed-relay-flow.coax`, identical in `--release`):

```text
verify ok=true verified=true kind=30617 sub=repo client_frame_bytes=412 ok_accepted=true
relay open=true out=(req=true,ev=true) sub=s0.1 ack=true deliver=true dup=true eose=true
refuse tamper_id=true forged=true stale=true unk_sub=true bad_ok=true
ok=true
```
