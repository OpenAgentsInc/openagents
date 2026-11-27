# 66. The compliance rule

> **Status: partly implemented.** The wire compliance checker
> (`coax/web/tool.sh wire-check`, [`coax/web/wire-check.coax`](../../coax/web/wire-check.coax)),
> registry ([`proto/registry.json`](../../proto/registry.json), 490 rows with
> only 1 allowlist line left to burn down), `#authority` descriptor parser
> ([`coax/lib/proto/importer.coax`](../../coax/lib/proto/importer.coax)),
> JSON golden key enforcer ([`coax/lib/proto/golden-keys.coax`](../../coax/lib/proto/golden-keys.coax)),
> and pinned single-codec profile ([`everglade/shared/everglade-wire.coax`](../../everglade/shared/everglade-wire.coax))
> run on `cpu`, grounded by
> [`openagents/examples/wire-compliance-gate.coax`](../examples/wire-compliance-gate.coax)
> and [`openagents/examples/wire-compliance-gate.out`](../examples/wire-compliance-gate.out),
> and are enforced in [`coax/web/prepush.coax`](../../coax/web/prepush.coax),
> [`coax/remote/check.sh`](../../coax/remote/check.sh), and
> [`coax/web/release.sh`](../../coax/web/release.sh).
> Remaining: burning down the final allowlist line and publishing a standalone
> external conformance fixture kit.

## Premise

A schema that no code uses protects nothing. This chapter states the rule that
ties each schema to the bytes the code sends, and the check that enforces it.

## Outline

- **Four conditions.** A shape is compliant when it is registered, has one
  codec, is pinned by a fixture with exact bytes, and has a schema.
- **Three kinds of codec.** Generated binary. Generated JSON, once Coax can
  make it. Hand-written and pinned by a profile.
- **When hand-written is right.** Hot netcode. Bytes that are signed, hashed
  or stored. JSON, until the mappers exist. The Everglade session is the
  model: one shared codec file and a profile that lists every frame.
- **Other people's formats.** A coding agent's stream, MCP, a NIP, LNURL. We
  register each with a fixture and write no schema for it.
- **Golden bytes.** A record needs a fixture before any codec change when a
  signature, a hash or a stored file depends on its bytes.
- **The real risk is two encoders.** Several shapes are written by hand in two
  to six places. The first step is one codec for each, and it needs no new
  tooling.
- **A proto nobody reads is not compliance.** A schema with no consumer and no
  check is marked as such.
- **The check.** One command, `coax/web/tool.sh wire-check`, fails on an
  unregistered shape, a second encoder, or a route file that no row accounts
  for. Its allowlist only shrinks.
- **Where things stand.** The registry has a row for each shape. Eight comply
  today: agent grants, the served decision, the Everglade session and its
  latency record, the Orbit League frames, the Haunted House save, the voice
  frame and the iOS home configuration.
- **The rule for new work.** A new message between apps, the API, rooms or
  devices gets a schema in `proto/` first. A page or a route never invents a
  JSON shape ([docs/reuse.md](../../docs/reuse.md)).
- **Graduating.** A package leaves `v1alpha1` only when a real consumer reads
  and writes it through generated code and the conformance list passes
  ([spec 35](../../docs/protocol/openagents-protocol-spec.md#35-conformance-and-verification)).
- **What this asks of the book.** Every chapter that sends, signs, stores or
  publishes a record names its schema, its carriage and its status.

## What we build

Nothing of its own. This chapter states the rule, and the
[compliance plan](../../docs/protocol/2026-11-11-compliance-plan.md) builds the tools.

Built ([#13236](https://github.com/OpenAgentsInc/openagents/issues/13236)):

- `proto/registry.json`: one row for each shape, with its schema, its one
  codec file and its check.
- `coax/web/tool.sh wire-check`: the check, and `proto/wire-allowlist.txt`,
  the list of shapes still to move.

Planned:

- Generated JSON writers and readers in `#import_proto`
  ([#13237](https://github.com/OpenAgentsInc/openagents/issues/13237)).
- The steps that move Coder's and Everglade's shapes, one at a time
  ([#13250](https://github.com/OpenAgentsInc/openagents/issues/13250)).

It builds on what exists today:

- [`docs/protocol/2026-11-11-compliance-plan.md`](../../docs/protocol/2026-11-11-compliance-plan.md)
- [`docs/protocol/2026-11-11-protocol-review.md`](../../docs/protocol/2026-11-11-protocol-review.md)
- [`proto/registry.json`](../../proto/registry.json)
- [`coax/web/wire-check.coax`](../../coax/web/wire-check.coax)
- [`proto/modules.json`](../../proto/modules.json)
- [`proto/verse/everglade-session-v7.json`](../../proto/verse/everglade-session-v7.json)
- [`everglade/shared/everglade-wire.coax`](../../everglade/shared/everglade-wire.coax)
- [`coax/web/api/agent-wire.coax`](../../coax/web/api/agent-wire.coax)
- [`coax/proto.md`](../../coax/proto.md)

### The chapter's check

- **Source:** [`coax/lib/proto/importer.coax`](../../coax/lib/proto/importer.coax),
  [`coax/lib/proto/golden-keys.coax`](../../coax/lib/proto/golden-keys.coax),
  [`everglade/shared/everglade-wire.coax`](../../everglade/shared/everglade-wire.coax),
  [`coax/web/wire-check.coax`](../../coax/web/wire-check.coax),
  [`proto/registry.json`](../../proto/registry.json),
  [`proto/wire-allowlist.txt`](../../proto/wire-allowlist.txt),
  [`openagents/examples/wire-compliance-gate.coax`](../examples/wire-compliance-gate.coax),
  and [`openagents/examples/wire-compliance-gate.out`](../examples/wire-compliance-gate.out).
- **Trace:** parses a `proto3` `FileDescriptorSet` (`types=1`), validates exact
  single-object and array JSON goldens against `.test.M` (`exact=true`,
  `list=true`), and verifies pinned hand-codec compatibility bounds
  (`version=7`, `min=5`, `content_rev0=1`, `content_rev7=7`). On the full tree,
  `coax/web/tool.sh wire-check` scans 490 registry rows, 44 route files, 67 room
  files, 162 profile files, and 99 `.proto` files with `0 failed`.
- **Refusal:** an unknown JSON key fails with `a key is no json_name of the
  message`; a missing required field fails with `the object does not have each
  field of the message once`; an unknown message fails with `the descriptor has
  no such message`; a `proto2` descriptor fails with `only proto3 data
  descriptors are supported`; an empty descriptor fails with `no runtime
  messages in descriptor`; and `wire-check` fails rule 1 on unregistered schema
  IDs and rule 6 on unreasoned allowlist additions.
- **Independent check:** `coax/bin/coax run openagents/examples/wire-compliance-gate.coax`
  and `--release` match [`openagents/examples/wire-compliance-gate.out`](../examples/wire-compliance-gate.out),
  `coax/web/tool.sh wire-check` passes with `0 failed`, and generated codecs'
  fixtures decode independently under Python's protobuf library.

Planned paths: none. The rule is checked by `wire-check` and
[`proto/registry.json`](../../proto/registry.json).

## Sources

- **Episodes:**
  - [Episode 234, *Product Promises*](../../docs/transcripts/234.md): mark each claim as live, gated or withdrawn
- **NIPs:** none
- **Also:**
  - [The compliance plan](../../docs/protocol/2026-11-11-compliance-plan.md) and [the protocol review](../../docs/protocol/2026-11-11-protocol-review.md)
  - [The OpenAgents Protocol](../../docs/protocol/openagents-protocol-spec.md), sections 35 and 36
  - [What to reuse](../../docs/reuse.md)
- **Promises covered** ([audit](../../docs/audits/2026-11-10-episode-289-port-audit.md#1-what-episode-289-promised)): P12

## Open questions for the owner

1. Does a chapter reach "implemented" only when its records are compliant?
2. Do we publish the fixtures as a kit that a second implementation can run?

## Source listing

Run the Chapter 66 example in [`openagents/examples/wire-compliance-gate.coax`](../examples/wire-compliance-gate.coax) (committed transcript [`openagents/examples/wire-compliance-gate.out`](../examples/wire-compliance-gate.out)):

<!-- source: openagents/examples/wire-compliance-gate.coax wire-compliance-gate -->
```jai
wcg_run :: () -> bool {
  // 1. Parse a proto3 FileDescriptorSet (package test; message M { int32 x = 1; })
  // and hold hand-codec JSON goldens to the descriptor's exact json_name fields.
  desc_v3 := [47] u8.[
    10, 45, 10, 10, 116, 101, 115, 116, 46, 112, 114, 111, 116, 111,
    18, 4, 116, 101, 115, 116,
    34, 17, 10, 1, 77, 18, 12, 10, 1, 120, 24, 1, 32, 1, 40, 5, 82, 1, 120,
    98, 6, 112, 114, 111, 116, 111, 51
  ];
  types: [8] D_Type; enums: [8] D_Type; fields: [64] D_Field; values: [16] D_Value;
  groups: [16] string; files: [8] string; deps: [8] string;
  arena: [4096] u8; output: [1] u8; symbols: [256] string;
  st := [1] D_State.[D_State.{
    types = types[..], enums = enums[..], fields = fields[..], values = values[..],
    groups = groups[..], files = files[..], deps = deps[..],
    arena = arena[..], output = output[..], symbols = symbols[..]
  }];
  D_parse(st[..], cast(string) desc_v3[..], "CG");
  nodes: [32] JvNode;
  ok_exact := pk_text_keys(st[..], ".test.M", "{\"x\":1}", true, false, nodes[..]);
  ok_list := pk_text_keys(st[..], ".test.M", "[{\"x\":1},{\"x\":2}]", true, true, nodes[..]);
  print("descriptor err=% types=% exact=% list=%\n",
    st[0].error == "", st[0].nt, ok_exact == "", ok_list == "");

  // 2. Hand-packed pinned single-codec rule (everglade/shared/everglade-wire.coax):
  // content revisions derive distinct pins under WIRE_CONTENT and keep a compatibility window.
  p0 := wire_pins_at(0);
  p7 := wire_pins_at(7);
  pins_ok := WIRE_VERSION >= WIRE_MIN && p0.content == WIRE_CONTENT && p7.content != p0.content;
  print("pinned_wire version=% min=% content_rev0=% content_rev7=% ok=%\n",
    WIRE_VERSION, WIRE_MIN, p0.content, p7.content, pins_ok);

  // 3. Fail-closed compliance refusals: unknown key, missing required golden field,
  // unknown message name, proto2 descriptor, empty descriptor, and bad prefix.
  ref_unk_key := pk_text_keys(st[..], ".test.M", "{\"rogue\":1}", false, false, nodes[..]);
  ref_missing := pk_text_keys(st[..], ".test.M", "{}", true, false, nodes[..]);
  ref_no_msg := pk_text_keys(st[..], ".test.Missing", "{\"x\":1}", false, false, nodes[..]);

  desc_v2 := desc_v3;
  desc_v2[46] = 50; // "proto2"
  st2 := [1] D_State.[D_State.{
    types = types[..], enums = enums[..], fields = fields[..], values = values[..],
    groups = groups[..], files = files[..], deps = deps[..],
    arena = arena[..], output = output[..], symbols = symbols[..]
  }];
  D_parse(st2[..], cast(string) desc_v2[..], "CG");
  ref_proto2 := st2[0].error == "only proto3 data descriptors are supported";

  st3 := [1] D_State.[D_State.{
    types = types[..], enums = enums[..], fields = fields[..], values = values[..],
    groups = groups[..], files = files[..], deps = deps[..],
    arena = arena[..], output = output[..], symbols = symbols[..]
  }];
  D_parse(st3[..], cast(string) desc_v3[0..<0], "CG");
  ref_empty := st3[0].error == "no runtime messages in descriptor";
  ref_prefix := !D_input(44, "bad-");
  print("refuse unk_key=% missing=% no_msg=% proto2=% empty=% prefix=%\n",
    ref_unk_key == "a key is no json_name of the message",
    ref_missing == "the object does not have each field of the message once",
    ref_no_msg == "the descriptor has no such message",
    ref_proto2, ref_empty, ref_prefix);

  return st[0].error == "" && st[0].nt == 1 && ok_exact == "" && ok_list == "" && pins_ok &&
         ref_unk_key == "a key is no json_name of the message" &&
         ref_missing == "the object does not have each field of the message once" &&
         ref_no_msg == "the descriptor has no such message" &&
         ref_proto2 && ref_empty && ref_prefix;
}
```

Output (`coax/bin/coax run openagents/examples/wire-compliance-gate.coax`, identical in `--release`):

```text
descriptor err=true types=1 exact=true list=true
pinned_wire version=7 min=5 content_rev0=1 content_rev7=7 ok=true
refuse unk_key=true missing=true no_msg=true proto2=true empty=true prefix=true
ok=true
```
