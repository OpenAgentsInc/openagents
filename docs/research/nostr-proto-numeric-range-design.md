# Versioned Nostr kind range enforcement

Scope: #ef04. The implemented `v1alpha2` event profile opts into the
`max_uint32` option and retains the existing v1alpha1 descriptors and behavior.
The remaining notes record the design and verification requirements; broader
consumer migration and event authentication remain separate work.

## Existing evidence and compatibility

NIP-01's event kind domain is 0..65535. Existing `Event.kind`,
`UnsignedEvent.kind`, `EventRef.kind` and `KindList.values` are uint32. The
existing event projection already compares with a parsed Nostr original whose
kind is bounded; standalone generated v1alpha1 values still admit the entire
uint32 range. `KindList.values` is repeated and may use packed, unpacked or
mixed protobuf encodings. Empty/absent repeated filters and optional field
presence must not be turned into required fields.

The current published v1alpha1 descriptors and frozen baseline hashes must be
preserved, with prior behavior documented. A stricter constraint is a semantic
compatibility change even though the protobuf wire shape does not change.
Provide an explicitly versioned constrained contract/profile and migration;
legacy readers must continue to rely on explicit Nostr domain admission.
Block protocols widely import v1alpha1 types: migrate dependencies deliberately.
Using both v1alpha1 and v1alpha2 message names in one descriptor changes importer
leaf-name disambiguation, so generated Coax symbol names must be characterized,
not assumed. Decode/encode byte compatibility is not equivalent to semantic
compatibility or signature/identity approval.

The importer currently recognizes extension numbers 51000..51008 and refuses
unknown policy options; its old test explicitly uses 51009 as unsupported. A
new opt-in extension must use an unambiguous unused number, be documented in a
versioned options contract, and remain unknown/fail-closed to the old compiler.
Update the old unsupported-extension test to another still-unknown option,
without altering frozen old descriptor/output fingerprints. Protoc interprets
and preserves the custom option, but does not enforce its semantic range itself.

## Focused design proposal

An optional inclusive `max_uint32` policy with an explicit presence bit has an
unambiguous 0 bound and covers these fields without pretending to support
signed, float, enum or u64 ranges. Admission rejects a maximum outside u32,
duplicate or malformed option encodings, wrong descriptor field types,
unsupported placement and conflicting policy before generating any code. A
policy result is a value error; its error precedence against wrong wire,
duplicate and budget limits must be written down and tested.

Apply the check to the already-decoded `f.raw` only after wire and u32 overflow
admission. The `G_constraints` generator already handles both unpacked fields
and each member of a packed repeated field. A setter must reject an out-of-range
value and must not return a successfully published value; any scratch output
mutation on refusal must follow the existing codec contract. Missing optional
fields stay absent, and an explicitly present 0 remains valid.

## Checks to write first

- Descriptor without the option keeps old behavior and old frozen bytes.
- Explicit max=0 permits 0 and refuses 1; omitted max is not interpreted as 0.
- Event/UnsignedEvent/EventRef kinds: absent where optional; 0 and 65535
  accepted; 65536 and 0xffffffff refused. Required Event.kind missing refuses.
- KindList: empty, each boundary, multiple values, packed/unpacked/mixed
  encoding, invalid value first/middle/last and an empty packed segment.
- Reject raw 10-byte negative uint32 encodings and values above uint32 before
  truncating. Check duplicate known fields, wrong wire, malformed/truncated,
  nonminimal varint in normal/minimal modes and nested messages; ensure unknown
  fields round-trip without allowing them to replace known validation.
- Reject unsupported field types, singular/repeated policy placement if not
  intentionally supported, out-of-range maximum, duplicate extension option,
  incompatible exact/max policy, bad wire type, unknown future option and
  ambiguities from an explicit option value of zero.
- Test mutation for inclusive boundaries and both packed/unpacked generated
  branches; require full authority mutation with no waivers, exact old/new
  compiler and generated-source hashes, independent protoc encodings, checked
  and release native/Wasm, relevant iOS target, Python-free bootstrap and
  independent source review. Do not mark #ef04 done until all acceptance passes.
