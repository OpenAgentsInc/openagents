# Engine Protobuf contracts

The repository's top-level
[`proto/openagents/engine/v1/scene.proto`](https://github.com/OpenAgentsInc/openagents/blob/main/proto/openagents/engine/v1/scene.proto)
owns the first shared engine contract:
scene axes, landmarks, prompt ranges, and collision boxes. This package defines
metadata, not engine commands, asset admission, signed bytes, or GPU layouts.
The existing Nostr and JSON document protocols are unchanged.

This crate consumes the language-neutral schema and holds its generators,
validators, fixtures and exported bindings. See [`proto/`](https://github.com/OpenAgentsInc/openagents/tree/main/proto)
for the schema layout. The setup follows LND's versioned packages and checked-in generated bindings,
and the LND SDK's regeneration tests. Rust uses Prost; C/C++ uses Nanopb with
fixed arrays and bounded strings for Wasm; Python supports asset tooling.
Normal builds need no compiler, network, or OpenAgents workspace dependency.
Nanopb's pinned source, hashes, and license are in `vendor/nanopb`.

## Generate and check

Install `protoc` 29.3 and install `tools/requirements.txt` into a Python virtual
environment. `toolchain.json` pins the generators. Select them with `PROTOC`
and `ENGINE_PROTO_PYTHON`, then run:

```sh
scripts/engine-proto.sh generate
scripts/engine-proto.sh check
cargo test -p engine-proto
```

The scripts use the shared Cargo target and source-keyed compiler state.
`check` regenerates C, Python, Rust, the descriptor, and conformance fixtures.
It also checks descriptor compatibility against `compatibility/scene-v1.descriptor.pb`.
That baseline is published history; generation never rewrites it. Keep field
numbers stable. Reserve removed names and numbers. New incompatible meanings
require a new package version. Run the compatibility regression tests with the
selected Python: `python -m unittest discover -s crates/engine-proto/tools`.

Rust `decode` checks the byte limit, required message presence, finite values,
identifiers, collection counts, ranges, and bounds. Other consumers must enforce
the same limits. `fixtures/cases.json` lists shared positive and negative inputs,
including unknown additive fields, invalid UTF-8, NULs, NaNs, and truncation.
Successful decode grants no permission. Unknown fields can be discarded; do not
round-trip signatures or content-addressed source bytes through these types.

## Consume from another repository

Commit the contract, then export a binding bundle:

```sh
scripts/engine-proto.sh export /path/to/new-bundle
```

The export needs only Python's standard library. Its manifest records the
OpenAgents commit and every file's SHA-256. A consumer pins that manifest and
checks the hashes when importing and building. Include `c/` and `nanopb/` in a
C/C++ build and define `PB_VALIDATE_UTF8=1`. Keep generation outside normal
builds. The bundle includes Python and Rust bindings and the shared fixtures.

## Grid and portable assets

`data/grid.json` owns the standalone Grid layout. Generation writes `data/grid.pb`;
Rust's Grid wall collision and Aethyr consume those same bytes. The Grid pack
exporter writes `grid-layout.pb` and `grid-assets.pb` beside its existing pack.
The pack's existing geometry and publication rules remain unchanged.

`AssetManifest` binds up to 32 exact files, with safe paths, lengths, SHA-256,
acyclic dependencies, and optional packed-buffer ranges. Its profile declares
the consumer's required files and formats. A valid unsigned manifest establishes
consistency, not distribution permission or a canonical signed identity.

`CompiledScene` carries affine placements, point-light approximations, cameras,
and a required spawn camera in Y-up meters. It permits 32768 placements, 2048
lights, and 64 cameras within 16 MiB. C decodes placements and lights through
bounded callbacks rather than allocating the maximum collection on the stack.
Rust validates with `portable`; asset tools use the exported `engine_portable`
Python module. `fixtures/portable` exercises the same limits across languages.
Source lighting and authored worlds retain their own semantics and formats.

Compatibility checking covers every frozen descriptor in `compatibility/`.
Add a new immutable snapshot when publishing additional messages, so subsequent
checks protect those messages as well as the original v1 scene contract.

## Phone agent contracts

The top-level `proto/openagents/engine/v1/agent.proto` defines durable tasks,
local coordinator operations, tool activity, dispatch and progress pages.
Swift uses Aethyr's C++/Nanopb codec through a narrow C ABI rather than adding
a second protobuf runtime to the app. Linux uses the generated Python module
and `engine_agent` envelope helper. Existing SSH authentication and Codex
protocols are unchanged. New phone tasks use the native HOST SDK described in
[`phone-host`](../phone-host/README.md); pre-HOST task records keep their original
SSH reader/cancellation path, with no new legacy submissions.

New runtime contracts use Nanopb callbacks so large repeated messages are not
allocated at their maximum size. `tools/contracts.py` derives C++ presentation
field tables and shared Python limits from descriptors and `.options`; neither
consumer authors a competing schema. Strings/counts, finite values, exact
integers, nesting and aggregate byte limits are enforced by consumers. Task
records reject unknown fields on read/edit/write; progress accepts additive
fields. This differs deliberately from signed bytes, which are never rewritten.

## Engine, HUD, state and profiles

The package also generates `hud.proto`, `control.proto`, `state.proto` and
`assets.proto`; the top-level proto index describes their consumers and
compatibility policy. State fields come from the engine's typed registration,
not an opaque JSON payload. Existing authored formats must be named explicitly.
World save containers have a new protobuf body and an old-format reader.

Consumer asset requirements are authored once in top-level
`proto/openagents/engine/v1/profiles.json`, generated to `data/profiles.pb`,
embedded in C++ and included by Rust. `manifest_for_profile` builds production
manifests from it; `validate_profile` checks exact files/formats/dependencies and
required buffer semantics. The low-level `manifest_for`/`manifest_files` helpers
remain for explicitly described imports and fixtures, rather than competing
production profile definitions. Normal builds need no generator or network.
