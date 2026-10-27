# 14. The protocol boundary

> **Design.** Spec §14. No proto frontend or codec exists in Coax yet.

OpenAgents speaks a public protocol: protobuf schemas for agents,
payments, decisions and the Verse, plus Nostr events. Coax code will
produce and consume those messages constantly. This chapter is about
doing that without creating a second, drifting source of truth.

## 14.1 The `.proto` files stay in charge

Public `.proto` files and their published descriptors are the source of
truth for interchange. Coax consumes them:

```coax
#import_proto "openagents/engine/v1/state.proto";
```

The import generates checked Coax types and bounded codecs from the
existing contract. Coax source may *propose* a schema change, but that
change goes back through the owning repository's versioning and
compatibility review. A Coax file is never a second schema that someone
can edit quietly.

The engine's current bundle (`third_party/openagents-engine-proto` in
aethyr-cpp) holds frozen schemas, generated bindings and compatibility
evidence. Coax work doesn't move or overwrite it. It reads it.

## 14.2 Bounded decoding

Every generated decoder checks before it allocates:

- message size and nesting depth;
- each field's limits (string bytes, list lengths);
- presence rules and the unknown-field policy;
- units and revisions.

Input that breaks a limit is refused before it costs memory. That
applies on every target, WebAssembly included. A decoded record grants
no authority: it's data, and permissions come from elsewhere.

## 14.3 Signed bytes are sacred

Nostr events and other signed records are verified over their **original
bytes**. Coax never re-serializes a signed record to check its signature
or hash it. Protobuf's ordinary serialization isn't canonical, so
re-encoding can change bytes without changing meaning. The original JSON
or content-addressed bytes keep their own representation and validation.

## 14.4 Four kinds of version

| Axis | What changes | Example |
| --- | --- | --- |
| Protocol version | The public contract | A field added to `EntityState` |
| Native layout version | A struct's memory layout | A field reordered for cache |
| Save version | What's on disk | A world save written last month |
| Hot-reload version | Live state in a running host | A struct changed while the app runs |

These are different axes, and Coax keeps them apart. Renaming a field
never implies a safe migration on any of them. Pointers, `Any` and live
resources are never wire data.

## 14.5 What this buys

Today a protocol type exists as a `.proto` message, generated C or C++
bindings, an engine struct, a save format and an editor description,
kept in sync by people. With metadata in the language, the engine struct,
codec, inspector and migration can all be generated from one checked
declaration, while the public `.proto` stays the contract everyone else
reads.
