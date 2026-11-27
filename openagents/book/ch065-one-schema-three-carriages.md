# 65. One schema, three carriages

> **Status: partly implemented.** `#import_proto` binary protobuf codecs
> ([`coax/web/api/agent-wire.coax`](../../coax/web/api/agent-wire.coax),
> [`coax/lib/proto/wire.coax`](../../coax/lib/proto/wire.coax)), the `#authority`
> protobuf JSON mapper runtime ([`coax/lib/proto/json.coax`](../../coax/lib/proto/json.coax)),
> and `#authority` descriptor provenance fingerprinting
> ([`coax/lib/proto/fingerprint.coax`](../../coax/lib/proto/fingerprint.coax))
> run on `cpu`, grounded by
> [`openagents/examples/schema-three-carriages.coax`](../examples/schema-three-carriages.coax)
> and [`openagents/examples/schema-three-carriages.out`](../examples/schema-three-carriages.out),
> and are consumed by [`coax/lib/compiler/compiler.coax`](../../coax/lib/compiler/compiler.coax),
> [`coax/lib/repo/wire.coax`](../../coax/lib/repo/wire.coax),
> and [`apps/openagents-cli/agent.coax`](../../apps/openagents-cli/agent.coax).
> Remaining: graduating all spec-sketch NIP records onto `.proto` schemas and
> wiring end-to-end Release-to-signed-Nostr-event carriage fixtures.

## Premise

Chapters 62 and 64 describe two ways a record travels. This chapter says what
the record is: one protobuf message in `proto/`, carried three ways. It then
follows one record from its schema to a signed event and back.

## Outline

- **The rule.** Each record is defined once, as a protobuf message under
  [`proto/`](../../proto/README.md). Nothing else defines a wire shape
  ([spec 8](../../docs/protocol/openagents-protocol-spec.md#8-one-message-three-carriages-nostr-http-and-rpc)).
- **Carriage one: a signed event.** The message's JSON form is the content of
  a Nostr event or of an encrypted envelope. That JSON is what gets signed
  (chapter 62).
- **Carriage two: the API.** The same message is an HTTP JSON body. When a
  record is both an event and an API body, the JSON is the same (chapter 64).
- **Carriage three: binary.** The same message as protobuf bytes: frames on a
  socket, files, and calls across a native boundary. We use it where both ends
  are ours, or where size and speed matter. A binary form never replaces a
  NIP's wire without a new version that both sides agree on.
- **How a record becomes an event.** Build the message. Map it to the NIP's
  JSON: its keys, enum spellings, amounts as decimal strings and its rule for
  null. Put the JSON in the content with the kind and tags. Hash and sign.
  Send it to relays.
- **Reading goes the other way.** Check the signature over the bytes you
  received, then parse. Never check a signature over bytes you wrote again
  ([spec 35](../../docs/protocol/openagents-protocol-spec.md#35-conformance-and-verification)).
- **When a hash covers a record,** the bytes are canonical, and the rule is
  written down ([spec 7.4](../../docs/protocol/openagents-protocol-spec.md#74-canonical-bytes-and-hashing)).
- **Public and private.** Each message has a data class. A private record
  rides in a kind 3188 envelope, encrypted with NIP-44 ([spec 6](../../docs/protocol/openagents-protocol-spec.md#6-data-classes-descriptor-options-and-authority),
  [spec 8.3](../../docs/protocol/openagents-protocol-spec.md#83-encrypted-carriages)).
- **Names and versions.** A package starts as `v1alpha1` and reaches `v1`
  when it passes the conformance list. A published name, number or JSON
  spelling stays frozen ([spec 5](../../docs/protocol/openagents-protocol-spec.md#5-naming-versioning-and-stability)).
- **What Coax generates today.** `#import_proto` gives typed views and bounded
  binary codecs. JSON mappers are planned
  ([#13237](https://github.com/OpenAgentsInc/openagents/issues/13237)). Until
  then each NIP's JSON is written by hand, which chapter 66 allows when the
  bytes are pinned.
- **What exists.** The table below. The spec's "Implemented" labels describe
  code from before the repository was reset, so this book reads status from
  [`proto/registry.json`](../../proto/registry.json) and the compliance plan.
- **The money rule is in the schema.** An amount is millisats or a Taproot
  asset amount, and the list of rails is closed ([spec 26.1](../../docs/protocol/openagents-protocol-spec.md#261-the-money-rule)).

## What we build

A plan for what this chapter ships once the outline is approved:

- One worked example with a fixture: a release, from message to signed event
  and back, byte for byte.
- The status column of chapter 63's `PROTOCOL.md`: for each record, its
  schema, its carriages and its codec.

It builds on what exists today:

- [`proto/README.md`](../../proto/README.md)
- [`proto/registry.json`](../../proto/registry.json)
- [`docs/protocol/openagents-protocol-spec.md`](../../docs/protocol/openagents-protocol-spec.md)
- [`coax/proto.md`](../../coax/proto.md)
- [`coax/lib/proto/importer.coax`](../../coax/lib/proto/importer.coax)
- [`coax/web/api/agent-wire.coax`](../../coax/web/api/agent-wire.coax)
- [`coax/lib/nostr/event.coax`](../../coax/lib/nostr/event.coax)
- [`coax/lib/nostr/jcs.coax`](../../coax/lib/nostr/jcs.coax)

Where the schemas stand today:

| Status | Meaning | Packages |
| --- | --- | --- |
| In use | A schema file, one generated codec, a fixture, and a product path that calls it | `openagents.account.v1alpha1` (agent grants), `openagents.decision.v1alpha1` (on the server), `openagents.metareal.home.v1alpha1`, `openagents.metareal.haunt.v1alpha1`, `openagents.voice.v1alpha1` |
| In use, hand-packed | One shared hand-written codec, pinned by a profile with exact bytes | The Everglade realm session ([`everglade-session-v7.json`](../../proto/verse/everglade-session-v7.json)), its latency record, and the Orbit League frames |
| Schema file | A `.proto` in `proto/`. It has no product consumer, or the code still writes other bytes by hand | `nostr.*` mirrors, `openagents.pylon.v1alpha1`, `openagents.psionic.*`, `openagents.host.v1`, `openagents.reach.v1`, `openagents.engine.v1`, `openagents.common.v1`, the Coder and CLI records |
| Spec sketch | Drafted in the spec only. The NIP's JSON, written by a Coax library, is the definition today | Most of this book: `chat`, `capability`, `policy`, `run`, `trace`, `knowledge`, `extension`, `eval`, `xp`, `market`, `labor`, `payment`, `receipt` |
| Planned | Named, not drafted | `openagents.sync.v1alpha1`, `openagents.session.log.v1alpha1`, `openagents.verse.doc.v1alpha1` |

### The chapter's check

- **Source:** [`coax/web/api/agent-wire.coax`](../../coax/web/api/agent-wire.coax),
  [`coax/lib/proto/json.coax`](../../coax/lib/proto/json.coax),
  [`coax/lib/proto/wire.coax`](../../coax/lib/proto/wire.coax),
  [`coax/lib/proto/fingerprint.coax`](../../coax/lib/proto/fingerprint.coax),
  [`openagents/examples/schema-three-carriages.coax`](../examples/schema-three-carriages.coax),
  and [`openagents/examples/schema-three-carriages.out`](../examples/schema-three-carriages.out)
  (remaining: full NIP-EXT `Release` three-carriage fixture across Nostr event + HTTP + binary).
- **Trace:** encodes and decodes a `27`-byte binary protobuf
  `openagents.account.v1alpha1.GrantRequest` (`per_day=50000`, `per_payment=10000`,
  `days=90`), writes the canonical JSON carriage with 64-bit sat amounts quoted
  as decimal strings and bytes as padded base64 (`"sats_per_day":"50000"`,
  `"digest":"AP8BPg=="`), parses the quoted 64-bit integer back into `4` bytes
  of protobuf scalar wire, and seals a `400`-byte `coax-proto-v4` descriptor
  provenance header.
- **Refusal:** truncated protobuf bytes fail with `AG_BAD_WIRE`; negative,
  fractional, plus-prefixed, and null JSON scalars fail with `PJ_VALUE`; node
  exhaustion fails with `PJ_LIMIT`; short provenance buffers fail with
  `fingerprint output budget`.
- **Independent check:** `coax/bin/coax run openagents/examples/schema-three-carriages.coax`
  and `--release` match [`openagents/examples/schema-three-carriages.out`](../examples/schema-three-carriages.out),
  alongside `#authority` checks in [`coax/lib/proto/json.coax`](../../coax/lib/proto/json.coax)
  and [`coax/lib/proto/fingerprint.coax`](../../coax/lib/proto/fingerprint.coax),
  and NIP-EXT vector checks in [`ext_release_vectors.py`](../../coax/tests/nostr/ext_release_vectors.py).

Planned paths: none. Schemas go in [`proto/`](../../proto).

## Sources

- **Episodes:**
  - [Episode 289, *OpenAgents*](../../docs/transcripts/289.md): 2:30, signed JSON over WebSockets
- **NIPs:** [the OpenAgents NIPs](../../nostr/nips/openagents/README.md)
- **Also:**
  - [The OpenAgents Protocol](../../docs/protocol/openagents-protocol-spec.md), sections 3 to 8 and 12
  - [The protocol review](../../docs/protocol/2026-11-11-protocol-review.md) and [the compliance plan](../../docs/protocol/2026-11-11-compliance-plan.md)
  - [`#import_proto`](../../coax/proto.md)
  - [Nostr From Scratch](../../nostr/book/README.md)
- **Promises covered** ([audit](../../docs/audits/2026-11-10-episode-289-port-audit.md#1-what-episode-289-promised)): P12

## Open questions for the owner

1. Which record is the worked example: a release, which is public, or a grant,
   which is private?
2. Do chapters show schema text, or only link to `proto/`?

## Source listing

Run the Chapter 65 example in [`openagents/examples/schema-three-carriages.coax`](../examples/schema-three-carriages.coax) (committed transcript [`openagents/examples/schema-three-carriages.out`](../examples/schema-three-carriages.out)):

<!-- source: openagents/examples/schema-three-carriages.coax schema-three-carriages -->
```jai
s3c_run :: () -> bool {
  // 1. Binary protobuf carriage: generated #import_proto codec for agents.proto.
  q: AwRequest;
  q.name_n = aw_text(q.name[..], "Ledger Agent");
  q.mask = ag_bit(AG_WALLET_SPEND);
  q.per_day = 50000;
  q.per_payment = 10000;
  q.days = 90;
  bin_wire: [AW_MAX] u8;
  bn := aw_request_write(q, bin_wire[..]);
  dec := aw_request_read(bin_wire[0..<bn]);
  print("binary bytes=% verdict=% per_day=% per_payment=% days=%\n",
    bn, dec.verdict, dec.per_day, dec.per_payment, dec.days);

  // 2. JSON carriage: 64-bit sat amounts as decimal strings + binary digest as padded base64,
  // then parsed directly back into protobuf wire scalar bytes.
  jbuf: [256] u8; o: [1] Jo;
  jo_start(o[..], jbuf[..]);
  jo_obj(o[..]);
  jo_key(o[..], "agent_name"); jo_str(o[..], cast(string) dec.name[0..<dec.name_n]);
  jo_key(o[..], "sats_per_day"); pj_u64(o[..], cast(u64) dec.per_day, true);
  digest := [4] u8.[0, 255, 1, 62];
  jo_key(o[..], "digest"); pj_b64(o[..], digest[..]);
  jo_end(o[..]);
  jtext := jo_text(o[..]);

  nodes: [32] JvNode; d: [1] Jv;
  perr := pj_parse(d[..], jtext, nodes[..]);
  sats_node := jv_get(d[0], 0, "sats_per_day");
  sats_num := pj_integer(d[0], sats_node, 4); // protobuf uint64 (type 4)
  pb_field: [32] u8;
  pb_n := pj_scalar(d[0], sats_node, 4, pb_field[..], 0, 1, false);
  print("json=% parse_err=% sats_u64=% scalar_bytes=%\n",
    jtext, perr, sats_num.raw, pb_n);

  // 3. Reproducible descriptor provenance envelope (coax-proto-v4).
  env: [1024] u8;
  fp := F_package("account-agents.pb", "AW", "coax0 0.52.0", "wire", "importer", "fingerprint", "BODY", env[..]);
  print("provenance ok=% header_bytes=% total_bytes=%\n",
    fp.error == "", fp.provenance, fp.written);

  // 4. Fail-closed refusals: corrupt wire, negative uint64, fractional int, node limit, short provenance buffer.
  bad_wire := [2] u8.[0x0a, 0x7f];
  ref_wire := aw_request_read(bad_wire[..]).verdict == AG_BAD_WIRE;
  bad_nodes: [8] JvNode; bd: [1] Jv;
  _ := pj_parse(bd[..], "[\"-1\",1.5,\"+5\",null]", bad_nodes[..]);
  ref_neg := pj_integer(bd[0], jv_at(bd[0], 0, 0), 4).error == PJ_VALUE;
  ref_frac := pj_integer(bd[0], jv_at(bd[0], 0, 1), 4).error == PJ_VALUE;
  ref_plus := pj_integer(bd[0], jv_at(bd[0], 0, 2), 4).error == PJ_VALUE;
  ref_null := pj_scalar(bd[0], jv_at(bd[0], 0, 3), 4, pb_field[..], 0, 1, true) == -PJ_VALUE;
  tiny_nodes: [3] JvNode;
  ref_limit := pj_parse(bd[..], "[1,2,3,4]", tiny_nodes[..]) == PJ_LIMIT;
  ref_fp := F_package("account-agents.pb", "AW", "coax0 0.52.0", "wire", "importer", "fingerprint", "BODY", env[0..<64]).error == "fingerprint output budget";
  print("refuse wire=% neg=% frac=% plus=% null=% limit=% fp_budget=%\n",
    ref_wire, ref_neg, ref_frac, ref_plus, ref_null, ref_limit, ref_fp);

  return bn > 0 && dec.verdict == AG_OK && dec.per_day == 50000 && !o[0].bad &&
         perr == 0 && sats_num.error == 0 && sats_num.raw == 50000 && pb_n == 4 &&
         fp.error == "" && fp.written > fp.provenance &&
         ref_wire && ref_neg && ref_frac && ref_plus && ref_null && ref_limit && ref_fp;
}
```

Output (`coax/bin/coax run openagents/examples/schema-three-carriages.coax`, identical in `--release`):

```text
binary bytes=27 verdict=0 per_day=50000 per_payment=10000 days=90
json={"agent_name":"Ledger Agent","sats_per_day":"50000","digest":"AP8BPg=="} parse_err=0 sats_u64=50000 scalar_bytes=4
provenance ok=true header_bytes=400 total_bytes=405
refuse wire=true neg=true frac=true plus=true null=true limit=true fp_budget=true
ok=true
```
