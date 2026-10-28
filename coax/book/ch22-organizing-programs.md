# 22. Organizing a larger program

The small programs in the first half of this book fit in one file. A
world, a compiler and an app do not. They need boundaries: who owns a
type, what crosses a boundary, and which parts a build is allowed to
replace together.

This chapter separates the facilities `coax0` has from the module system
Coax plans. You can use the organization here now without writing syntax
that the compiler cannot accept.

## Three ways of sharing code today

| Mechanism | Available now | What it shares |
| --- | --- | --- |
| `#import "Basic"` | Yes | Built-in printing support; not a package loader |
| `#import_proto "profile.pb" as Name` | Yes | Declarations generated from an explicit descriptor input |
| Compile each Coax source as a C library | Yes | Exported procedures and their generated C ABI |
| Concatenate sources before compiling | Yes, as a build operation | One flat declaration namespace and one file's check rules |
| General source modules, `#load`, module visibility | Planned | Source-level dependency and visibility boundaries |
| Package download, version resolution and lockfile tooling | Not provided by `coax0` | These require their own design and implementation |

A folder called `lib` does not make its files importable. For example,
`coax/lib/compiler/checker.coax` is an actual implementation module, but
today the bootstrap script composes it with the other compiler sources.
`#import "compiler/checker"` is not a working alternative.

Chapter 17 shows the C-library approach. The iOS app uses it for the
home, player, cast, camera, Grid, wallet and Haunted House. Each exported
boundary is deliberately small. An app can replace one implementation
without requiring Swift to understand its internal procedures.

Concatenation is useful for tightly coupled compiler modules. It does
not create visibility or isolation. Duplicate names collide; the combined
file has one `#authority` scope; relative descriptor paths resolve from
the combined file's location. Keep the input order explicit, use unique
prefixes, and place temporary outputs outside authored source folders.
The capstone in Chapter 26 shows how to compose sources without losing a
descriptor path.

## Decide what a boundary means

There are three different identities to keep straight:

- A Coax type name identifies a declaration in a compiled program.
- A C symbol and its header describe a particular native ABI.
- A protobuf package, message and field number describe an interchange
  contract that may outlive many builds of both programs.

These need not have the same spelling or layout. A packed network record
is not a native struct, and renaming a Coax local does not rename a
published protobuf field.

For a new subsystem, write down its state, commands, observations and
limits first. Put the pure transition rules in Coax. Export plain values
or caller-owned views. Give the host only the work that needs the
platform: allocating storage, delivering input, opening a file or socket,
and binding a GPU resource.

Use the root `proto/` tree when values cross a process, device, storage or
version boundary. Select an existing descriptor profile where possible.
Do not introduce a second app-specific definition of a shared contract.
An internal camera vector that never leaves one process does not need a
protobuf representation merely because it is a struct.

## Names at the C boundary

Generated headers currently preserve user-defined type names in the
native namespace, prefixed with `coax_t_`. Independently compiling two
unrelated `Vec3` declarations does not make them the same type. Including
both headers can fail even if their fields happen to match.

Use descriptive prefixes such as `PlayerVec3` and `CameraShot`. Prefix
exported procedures too: `metareal_camera_overview` tells you its owner
at the call site. Do not hand-edit generated headers to resolve a clash.

There is a second collision to watch for: fixed-array aliases generated
in separate headers. The current app's bridging header has an explicit
native compatibility alias for this case. Inspect the generated headers
and the actual
[Bridge.h](https://github.com/OpenAgentsInc/openagents/blob/main/apps/openagents-ios/OpenAgents/Platform/Bridge.h)
before composing independently generated libraries. Namespaced source
modules and stable cross-module type identity remain future work.

Changing a native struct can require rebuilding every consumer. Keep the
layout assertions emitted in its header enabled. A protobuf schema's
compatibility rules cannot validate a C calling convention or a Swift
imported layout.

## A small dependency policy

A useful initial dependency graph has four layers:

1. Pure domain types and transitions, with checks and no platform calls.
2. Representation adapters: protocol bytes in and out, with explicit
   capacities and borrowed storage.
3. The host: lifecycle, effects, storage and threading.
4. Presentation: the host draws the state and sends user commands back
   through the domain transitions.

An arrow from the first layer back into the host deserves scrutiny. If a
payment decision needs the current time, pass a time value. Do not make
it ask a hidden clock. If a task needs a network response, pass an event
when the response arrives. Chapter 23 develops that pattern.

The compiler cannot enforce this architecture yet. Review the imports,
`#foreign` declarations and build inputs. Reject source cycles by policy
rather than hoping a future module loader will resolve them. A dependency
that requires both sides to initialize each other is often two pieces of
one subsystem, or a missing shared contract.

For reproducible builds, record the compiler revision, descriptor bytes,
source inputs, host compiler and target SDK. Pin external dependencies in
the host's existing dependency mechanism. `#import_proto` consumes a file;
it does not fetch a registry or choose a compatible version for you.

## What counts as the standard library?

Coax does not yet ship a general standard library for collections, files,
networking or concurrency. The specification's allocator context, modules
and richer containers are designs, not APIs you can import today.

Here is the current practical inventory:

| Facility | Contract to rely on today |
| --- | --- |
| Scalar operators, checked casts, fixed arrays and views | Compiler-defined semantics; see Chapters 3 and 6 |
| Math intrinsics | Supported argument types are checked by the compiler; native results depend on the target math implementation unless a stronger mode is implemented |
| `Basic` printing | Host output, unavailable as a compile-time effect; not a structured logging or localization API |
| Generated protobuf codecs | Caller-owned buffers; borrowed results; capacity and wire errors returned as values; see Chapter 15 and `coax/proto.md` |
| Files, sockets, audio, clocks, GPU objects | Platform adapters, not Coax standard-library APIs |
| `coax/lib/compiler/` and `coax/lib/proto/` | Internal source with build and ABI obligations, not a promised stable public library |

When adding a reusable API, document more than its signature. Say whether
it borrows or copies, who can mutate the backing memory, when a result
expires, what it allocates, the maximum work it does, and how failure is
reported. State whether order and output are deterministic across targets.
For a view-returning parser, those statements are as important as the
fields in its result.

A sensible first module system should make these existing boundaries
expressible rather than obscure them. It needs explicit exports, unique
type identity, predictable cycle errors, recorded input dependencies and
diagnostics that retain the original source location. Until it exists,
small C ABIs and explicit build composition are the working path.
