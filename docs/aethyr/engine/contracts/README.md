# Contract schemas

Status: proposed v1 wire fixtures, not implemented engine APIs. Read
[contracts](../contracts.md) before implementing a decoder.

- [Command](command.schema.json): closed typed instant transaction envelope.
- [World](world.schema.json): initial semantic building/entry-point wire slice.
- [Release](release.schema.json): exact artifacts, engine identity, entry points.
- [Channel](channel.schema.json): sequenced expected-previous release reference.
- [Signature statement](signature-statement.schema.json): exact signed identity
  statement; detached signature handling is specified in the contract document.

Schemas use JSON Schema Draft 2020-12. `$id` values under the reserved `.invalid`
TLD identify these local design documents; they are not deployed endpoints.
Every schema reference resolves locally. No network retrieval is required.

JSON Schema validation establishes shape and scalar bounds. Implementations must
also reject duplicate JSON keys, enforce encoded byte/nesting limits, validate
IDs/references/topology/authority, canonicalize using full RFC 8785 JCS, check
signatures and key status, verify artifact bytes, and enforce semantic limits.
Passing these schemas cannot establish that a release is runnable or admitted.

The initial world slice covers structural buildings and entries. The full target
component families and their qualification are specified in
[game systems](../game-systems.md). Additional schemas, bytecode, binary
artifacts, protocol versions, and migration fixtures are required before those
features ship. They must not be inserted as arbitrary untyped extension fields.
