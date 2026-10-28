# Descriptor imports in coax0 0.6.0

`#import_proto` generates real Coax declarations from a binary
`google.protobuf.FileDescriptorSet`. Root `proto/` remains the schema source.
The checked-in `proto/descriptors/*.pb` files include each profile's dependency
closure; `scripts/proto.sh` regenerates and checks them with pinned protoc.

```jai
#import_proto "../../proto/descriptors/nostr-event.pb" as N;
```

The path is relative to the importing source file. The required prefix keeps
imports separate. Every top-level source file may import up to eight profiles.
The compiler expands imports before normal parsing/checking; `coax c` exposes
the resulting implementation, and `coax header` exposes exported C wrappers.
`coax/examples/nostr-proto.coax` is a runnable presence/repeated-field example.

The importer is built into coax0 in C++20. `coax/src/proto.cpp` parses the
bounded binary descriptor directly; `coax/src/proto_runtime.inc` embeds the
Coax runtime as a raw string literal in the compiler binary. There is no
interpreter, protoc invocation, protobuf library, subprocess, temporary file,
or installed helper/runtime-file dependency. A relocated compiler works with
an empty `PATH`. The retired `COAX_PROTO_TOOL` and `COAX_PROTO_PYTHON` variables
have no effect. Generated C11 also needs no protobuf or interpreter runtime.
No schema code executes during import; `#run` still refuses host I/O.

To inspect or save exactly the generated Coax declarations without compiling
an application:

```sh
coax/bin/coax proto proto/descriptors/ios-home.pb --prefix Home -o /tmp/home-codecs.coax
```

## Generated API

A unique message leaf `Event` becomes `N_Event`. When multiple packages contain
the same leaf, each uses its flattened fully-qualified name. Comments in the
generated source record the full name. Enums generate `N_Enum_MEMBER`
constants and `N_Enum_known(s32)`; fields retain open enum values as `s32`.
Collisions, unresolved dependencies and unsupported shapes are errors.

| Declaration | Behavior |
| --- | --- |
| `N_Event { wire: [] u8; error: u32; }` | Borrowed message view, never an owning object |
| `N_Event_new()` | Empty builder; required fields may be added incrementally |
| `N_Event_decode(bytes)` | Validate the complete message and nested children |
| `N_Event_decode_minimal(bytes)` | Additionally require minimal key, length and scalar varints recursively, including packed elements, plus 0/1 booleans |
| `N_Event_get_kind(msg)` | `{value, present, error}`; missing differs from explicit zero |
| `N_Event_iter_tags(msg)` / `N_Event_next_tags(iterator)` | Iterator with `value`, `has_value`, `done`, `error`; first call selects the first value |
| `N_ClientFrame_case_payload(msg)` | Presence-bearing field number of the selected oneof member |
| `N_Event_set_kind(msg, value, output)` | Replace a singular field; set-oneof replaces its siblings |
| `N_Tag_append_values(msg, value, output)` | Append one repeated value |
| `N_Event_clear_kind(msg, output)` | Remove all occurrences of the selected field |
| `N_Event_encode(msg, output)` | Validate completeness, then copy the exact wire bytes |
| `N_Event_unknown(msg, cursor)` | Next unknown field with `number`, `start`, `next`, `done`, `error`; resume at `next` |
| `N_Event_FIELD_kind` | Stable protobuf field number |

Strings and bytes borrow storage. Nested messages are views into that same
storage. Iteration preserves order, duplicates in repeated fields, and empty
strings/messages. Repeated numeric readers accept packed and unpacked input;
append writes a legal unpacked value. Recursive message definitions work
because generated structs contain wire views rather than embedded values.

Writers require a separate caller-owned output buffer. Input bytes, string or
message payloads must not overlap the output, including partially overlapping
slices. Alternate two buffers for successive edits. A successful returned view
borrows the output; retain it until all readers finish. Never mutate backing
storage while a message or iterator is in use. Native C/Swift adapters must
lend valid pointers with truthful counts; this is not a pointer validator.

Always check `.error` before using a result. On failure the returned view is
empty. A capacity or alias failure is detected before writing; a schema-policy
failure can leave output bytes unspecified. No result owns or allocates target
memory. Children passed to setters must be complete; only the current builder's
required-field check is deferred until decode/encode.

## Validation and deliberate limits

The first importer accepts proto3 data descriptors: every scalar wire type,
nested messages, open enums, repeated fields, explicit optional presence and
oneofs. It rejects proto2, editions, groups, maps, extension fields/ranges,
uninterpreted options and unknown custom options. RPC services are not generated.
The annotation definitions `google/protobuf/descriptor.proto` and
`openagents/options/v1alpha1/options.proto` are metadata, not generated product
message types. Standalone `.options` files are not read by this importer.

Default runtime limits are 1 MiB per message, 64 nested edges and 524,288 total
field/packed-element visits across a validation. Descriptor limits are 8 MiB,
400,000 visited wire metadata items (40,000 per descriptor object),
256 files, 256 messages, 256 enums with 8,192 values total, 256 fields
per message, 2,048 fields total, 32 nested declaration
levels, 256 bytes per identifier/prefix, 4 KiB package/file names,
32 package components, 16 KiB metadata text values, 256 oneofs per message,
8 MiB generated source per import and 16 MiB per compilation. Descriptor
policies can tighten runtime bounds. `max_message_bytes`, `max_bytes`,
`max_count`, `exact_bytes` and `required_value` are enforced. Strings require
valid UTF-8; `nonempty-no-nul` and finite `rad` in [-pi, pi] are implemented.
Other unit labels and data-class/contract annotations are descriptive, not
additional domain validators.

**This reader is deliberately stricter than protobuf's merge semantics.**
Duplicate known singular fields and multiple encoded members of a oneof return
`DUPLICATE`; nested message fragments are not merged. Unknown supported wire
fields survive decode/encode and edits byte for byte. Groups are rejected even
when unknown. `decode_minimal` also requires known booleans to encode 0 or 1. This is an
opt-in resource policy, not a canonical serialization
or signing scheme; normal decode continues to accept legal non-minimal varints.
Encoding preserves original bytes and field order except edited
fields, which are appended. It is not a canonical serializer or hash function.

| Error | Value |
| --- | --- |
| OK | 0 |
| WIRE: malformed/unsupported wire type | 1 |
| LIMIT: size, count, depth or capacity | 2 |
| DUPLICATE: singular/oneof ambiguity | 3 |
| UTF8 | 4 |
| REQUIRED | 5 |
| ALIAS: overlapping buffers | 6 |
| VALUE: invalid scalar/grammar/unit value | 7 |

Generated constants use the prefix `N__pb_`, for example `N__pb_REQUIRED`.
Compiler diagnostics E0501, E0502 and E0504 report syntax, descriptor and target
errors (external-tool error E0503 is retired) at the importing directive, including machine-readable `--json` output.
Generated declarations are appended so original user-source line numbers stay
stable. Importing codecs into a Metal library is explicitly rejected.

The runtime uses five small CPU intrinsics: `bytes_overlap(string|[]u8,
string|[]u8) -> bool`, `f32_bits`, `f64_bits`, `f32_from_bits`,
`f64_from_bits`. Bit conversion uses memcpy semantics, preserving runtime
NaNs/infinities and signed zero without aliasing violations. Compile-time float
values must remain finite; address-overlap queries cannot run at compile time.

## Evidence and remaining gates

```sh
coax/build.sh test
PROTOC=/path/to/protoc-29.3/bin/protoc python3 coax/tests/proto_import.py --oracle
```

Tests compile the actual Nostr/external and existing OpenAgents profiles. Generated
Coax is compiled to C11 and exercised through its generated C ABI. Tests cover
the real source-derived Event fixture, unknown retention and field edits,
External identity/agent/channel types, every scalar type, full-width numbers,
packed repeats, nested recursion, null/missing/zero, oneof replacement, limits,
invalid input, buffer alias/capacity and descriptor rejection. Generated native
code runs under ASan/UBSan. The oracle checks both wire directions with pinned
protoc and reproducibility of the synthetic scalar descriptor fixture.

This ships descriptor ingestion and its codec layer. Product migration,
JSON/JWS/Nostr conversion, canonical hashing, cryptographic verification,
admission, HOST/REACH runtime behavior, transport clients and domain validators
remain separate work. A decoded Event is not verified or authorized. Existing
hand-written mobile home/protocol adapters remain until consumer parity is
proven; this change does not deploy or replace the app.

## Native importer audit boundary

Descriptor imports are explicit compiler input reads, separate from `#run`.
Generated C/headers carry the descriptor and embedded runtime SHA-256 values,
native generator ID and compiler version in a `coax-proto-v2` comment. The
compiler binary/toolchain must still be pinned independently; fingerprints
are not signatures, authority or a hermetic-build guarantee.

The native reader borrows descriptor byte spans only for the duration of an
import. It validates wire types, duplicate metadata, UTF-8, names, dependency
closure, options and supported shapes before generation. Work, nesting, input
and output budgets bound the import. Errors retain the originating import's
location, including JSON diagnostics; generated declarations do not invent
user-file line numbers past EOF.

`tests/proto/import-baselines.json` pins the pre-port generated source for all
11 existing profiles plus the scalar fixture. The native CLI must emit the
same body byte for byte. Tests also relocate the compiler alone and generate
the actual iOS C/header with an empty PATH, poisoned legacy tool variables and
an unusable temporary directory. Native descriptor fuzzing and SHA padding
vectors run under ASan/UBSan; target-side protoc/ABI tests remain independent.

`coax test coax/tests/proto/runtime.coax` executes the generated wire routines
over all two-byte varint/UTF-8 inputs and wide boundary vectors. File-level
`#authority` still cannot exclude generated APIs or native-address operations
from its coverage requirement. Python is used by test/oracle and schema
maintenance scripts, not normal compilation or the iOS app build.
