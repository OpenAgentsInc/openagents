# Native renderer qualification tools

The catalog qualification runs in Coax:

```sh
coax/web/tool.sh aethyr/render/tests/check-catalog.coax \
  --work-dir /tmp/catalog-check --receipt /tmp/catalog-check/qualification.json
```

`--coax FILE` selects a compiler explicitly. `--compiler-source-digest FILE`
selects that compiler's retained source record. A recorded/current source
mismatch refuses before emission. `--explore-stale-compiler` allows a bounded
observation under that explicitly identified compiler; its receipt remains
`exploratory` and says the compiler does not match current sources. It does
not qualify the current compiler or production release.

The default modes are debug, release and ASan/UBSan. Select a subset with
`--modes debug`, for example. Emission is shared only between modes which use
the same checked/wrapping C. Every command and exact raw stdout/stderr is
retained, including trailing newlines. Source and executable identities are
checked again before the final receipt. An unchanged qualified source/compiler
inventory is refused rather than run again against the same receipt.

The expected 18-profile native catalog comes from the frozen declaration
snapshot and its authenticated historical Git blobs. It does not call the
product catalog to obtain its expected answer. Every field, binding, pass and
use is compared, and the seven existing producer-loss controls must disagree
with that whole answer in every native mode. Added, missing and reordered ABI
fields refuse.

The current realm list is a separate complete inventory. Each shared realm's
identity, page and asset declarations must match its known catalog entry;
listed frozen consumers cannot disappear. Anime Lab is explicitly reported as
a tracked legacy consumer until its shared profile is implemented and
qualified. An unknown realm cannot gain a renderer through a fallback. The
initial current result is 16 shared consumers and one tracked legacy consumer.

The port retains the existing `aethyr.render.catalog.inventory.v1` fields and
transcript paths and adds current consumer and qualifier provenance. It makes
no shader arithmetic, GPU image or complete playable-realm claim. Evidence,
the old Python comparison inputs and failed controls are retained under
`docs/verification/data/aethyr-ch11-2026-11-02/catalog-tool-port/`.

The package transport qualification also runs in Coax:

```sh
coax/web/tool.sh aethyr/render/tests/check-package.coax \
  --packages /tmp/render-packages --work-dir /tmp/package-check \
  --receipt /tmp/package-check/qualification.json
```

It uses the same strict compiler identity and explicit exploratory flags.
By default it checks all 18 frozen profiles in debug, release and ASan/UBSan;
`--profiles the-grid` selects a smaller input slice. Coax authenticates and
copies every supplied manifest/product, compares every native record field
with an independent transformation of its JSON, and checks fetched WGSL,
SPIRV and tree bytes for every entry. Six producer losses and 17 malformed,
catalog or corrupt-product controls must be detected in every mode. All six
native ABI records are inventoried. `--check-header FILE` exposes that same
inventory for added/missing-field controls.

The existing `aethyr.render.package.native.v1` receipt and mode fields remain;
raw command outputs and transcript hashes accompany source, compiler and
qualifier-image fences. The original Python expected-value and negative-case
functions matched all 18 profiles and all 14 exact negative JSON byte strings
before retirement. The unchanged Python source is a reference input inside
`docs/verification/data/aethyr-ch11-2026-11-02/package-tool-port/`'s archive,
alongside failed trials and the qualified native artifacts. The port parity
driver takes an optional historical `REFERENCE_ROOT`; it does not depend on a
maintained Python implementation. This boundary qualification makes no shader
arithmetic, full SPIRV validity or GPU image claim.

## Canonical package stripping

```sh
coax/web/tool.sh coax/web/strip-render-checks.coax /tmp/strip-render-checks
```

This replaces the two former `strip-render.test.ts` cases. It invokes the
existing `strip.ts` CLI, checking exact shader bytes and refusal before a
hash-changing overwrite under both `assets/build/render/` and
`site/build/render/`. It retains command transcripts, fixtures and a source
fence in the output directory. The strip implementation remains on the tool
port list; this command makes no whole-deployment claim.

## Build publication and independent product inventory

```sh
coax/web/tool.sh aethyr/render/tests/check-build.coax \
  --work-dir /tmp/build-check --receipt /tmp/build-check/qualification.json \
  --profile the-grid --faults
```

Coax owns the compiler/source gate, native builder compilation and provenance,
all frozen catalog expectations, exact command records and deadlines, product
and source hashes, cache repair and atomic publication controls. Default source
mismatch refuses before native compilation or shader emission. The same explicit
exploratory flags described above allow bounded observations under an identified
older compiler and keep the receipt `exploratory`.

The build qualifier compares all three builder enumeration tables with the
frozen declarations. An unchanged package must reuse its manifest with identical
bytes and modification time. `--cache-controls` corrupts and repairs WGSL,
SPIRV and tree products; `--faults` includes those repairs and all seven planted
compiler faults. Every failed build must retain the previous manifest exactly.
`--builder FILE --builder-receipt FILE` and the analogous `--fault-compiler`
pair reuse already compiled, source/compiler-bound tools. A mismatched source
receipt refuses rather than blessing an old executable.

The independent `spirv(data)` word/type/decorations/interface decoder remains
byte-for-byte unchanged in `spirv_inventory_oracle.py`, under its narrow reason2
header. Its CLI only reads one byte file and prints facts; it shares no compiler
or product parser. Coax checks those facts against declared resource contracts,
entry aliases, densely packed vertex fields, capabilities and derivative/control
requirements. The qualifier bounds products at 4 MiB and vertex attributes at
the existing 32-field reflection capacity; tree words must be decimal uint32.
`--verify-package DIR --work-dir DIR [--report FILE]` exposes the same independent
verifier for product poison controls without building shaders.

The existing `aethyr.render.build.qualification.v1` fields remain, including
`qualifier_sha256` as the qualifier source hash; the executable hash is an
additional field. The port compared all 18 fresh package reports, 58 unique
SPIRV fact outputs, 22 actual product poisons, eight malformed word streams and
all seven exact fault fixtures with the retained reference. Old packages whose
source hashes had become stale were refused and retained as failed trials;
their metadata was not rewritten. Evidence and original reference inputs are
archived in `docs/verification/data/aethyr-ch11-2026-11-02/build-tool-port/`.
These checks do not qualify GPU images or the current compiler's release.

## Captured CPU-frame transport

```sh
coax/web/tool.sh aethyr/render/tests/check-reference-frame.coax \
  --frame-config /tmp/captured/frame.config --package /tmp/render/render.json \
  --work-dir /tmp/cpu-frame --raster-rounding nearest-even --subpixel-bits 8
```

Use exactly one of `--capture FILE` (the producer capture JSON) or
`--frame-config FILE` (the shared whitespace-token transport). `--prepare-only`
authenticates and snapshots inputs without compiling or running the renderer.
It preserves every buffer observation, texture level, raw media payload,
package tree and selected provenance file. The observer inventory is bounded
at 4096 tokens, 2048 rows, 1024 referenced files and 32 tree rows; signed counts
retain their int64 limits and raw buffer observations are not reinterpreted.
The original source config is preserved alongside the canonical staged config.

The five native modes remain `checked-o2` (default), `debug`, `release`,
`sanitized` and `sanitized-o1`. Checked and wrapping C cannot cross backend
profiles. An exact checked emission can be rebuilt for another native mode;
executable reuse also requires identical native driver and loader hashes.
`--reuse-backend DIR` requires a completed stable receipt, exact compiler and
source identities and all three retained artifacts. Corrupt or incomplete
records refuse.

New receipts distinguish `renderer_source_sha256` from the complete observer
source inventory. Only the compiler's renderer dependency closure, including
descriptors, determines whether an emitted backend survives observer-tool
changes. Legacy receipts retain their original Coax-source fallback. Driver or
loader changes require rebuilding the executable. The observer still fences
its own Coax/native sources and executable before writing a measurement.

Default compiler mismatch refuses before input preparation or emission. The
explicit compiler/exploratory options used by the other tools are available
here too. A measurement always retains `qualification:false`; an older compiler
also records `qualification_status:"exploratory"` and
`compiler_sources_match:false`. Those receipts cannot authorize normal backend
reuse or satisfy release gates. Existing measurement/preparation fields and all
named raster, screen, work, filter and provenance options remain available.

The port qualifies identical 64-byte images and complete frame results in all
five modes using a synthetic 2×2 fixture assembled from exact retained uniform
bytes and an authenticated current-source package. It also retains exact
17-row snapshot/reference parity, the original ten tool tests, 14 legacy
backend refusals, observer/descriptor closure controls, both preparation paths
and eight actual CLI refusals. Evidence and the retired Python reference inputs
are under `docs/verification/data/aethyr-ch11-2026-11-02/reference-frame-tool-port/`.
No large realm CPU, raster, geometry or GPU-image gate was repeated for this
tooling port.
### Retained-frame profiling

`coax/web/tool.sh aethyr/render/tests/profile-reference-frame.coax
--measurement MEASUREMENT --work-dir OUT` instruments the exact retained
generated C without changing its returned values. `--pixels X,Y ...`,
`--fragments` and `--extent` preserve the original diagnostic modes.
`--driver FILE` must match the measurement's exact recorded native transport;
an unavailable historical transport is refused.

The selected observer compiler must match current compiler sources by default.
`--coax FILE --compiler-source-digest FILE --explore-stale-compiler` permits an
explicit frozen-artifact observation, recorded with `qualification: false` and
separate observer/renderer compiler hashes. It cannot qualify current sources
or a release. The full result and image bytes must match the original receipt.

The profiler port record is
`docs/verification/data/aethyr-ch11-2026-11-02/reference-profiler-tool-port/qualification.json`.
It retains exact transformation parity, all original unit methods, refusal
controls, the fresh 2×2 transport and frozen Galaxy profile. Watchtower's missing
historical driver remains a refusal. Original Python sources are reference
inputs in its verified archive, not current tools.

The same record documents a corrected shared inventory serializer: an empty
root now preserves the leading slash of an absolute path. Older receipts and
their source epochs remain unchanged; old empty-root keys with a missing slash
are not admitted as profiler inputs. The fresh 2×2 replacement has valid
absolute paths and the identical complete result and 64-byte image.
The small native input-effects check is
`coax/web/tool.sh aethyr/render/tests/check-frame-input.coax --out OUT`.
It runs the original 20 transport fixtures, including exact texture levels,
relative paths, signed-integer refusal and unparsed media bytes containing NUL.
It preserves the original C probe and parser header. The same explicit compiler
selection and exploratory limits apply. Its verified reference/evidence archive
is `docs/verification/data/aethyr-ch11-2026-11-02/frame-input-tool-port/`.
The bounded filter qualifier is
`coax/web/tool.sh aethyr/render/tests/check-filter-native.coax --out OUT`.
It preserves all three native modes, each with 58 positive observations,
232 compared channels, 72 refusals and seven producer-loss controls. Every
channel must be finite and within the original inclusive absolute bound
`1e-7` of the retained independent device observation. The existing IEEE codec
only decodes exact number words; Coax owns the comparisons and every verdict.

`--reuse-emission` requires the tool's exact compiler, source, C, header and
profile admission. `--prepare-only` observes emission admission without running
native filters. Corrupt admissions refuse before native compilation. The
default current-source gate and explicit exploratory limits apply; an existing
qualified output is retained instead of repeated. The evidence and original
reference bytes are in
`docs/verification/data/aethyr-ch11-2026-11-02/filter-tool-port/`.

The former geometry and pipeline tooling tests run as
`check-geometry-tooling.coax` and `check-pipeline-tooling.coax`. They construct
the same small ABI, interval, publication and pixel-word loss controls, calling
the unchanged independent equations for reference facts. They execute no full
producer or renderer gate. `check-pipeline-processes.coax --out OUT` separately
checks actual nonzero exits, bounded child-tree timeout, SIGINT and SIGTERM;
only observed absent or zombie processes count as cleaned up. The narrow
hand-pinned depth equation test remains an independent Python oracle.
