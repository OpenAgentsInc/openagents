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
