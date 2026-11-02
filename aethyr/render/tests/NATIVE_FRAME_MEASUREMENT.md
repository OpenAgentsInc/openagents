# Native frame measurement

`measure-native-frame.coax` stages a complete retained RFI and executes the
pre-emitted Coax RenderHost and RenderTexture backends through the native
Vulkan effects. It records the raw frame; a separate image comparator decides
agreement with a CPU or browser image.

Prepare locally, without SSH or GPU work:

```sh
COAX=/path/to/coax coax/web/tool.sh aethyr/render/tests/measure-native-frame.coax \
  --work-dir /tmp/new-native-preparation \
  --coax /path/to/coax --compiler-source-digest /path/to/source-digest \
  --frame /path/to/frame.config --package /path/to/package \
  --capture-root /path/to/retained-capture --native-dir /path/to/emitted-native \
  --prepare-only --format rgba16f
```

The native directory must contain `RenderHost.c`, `RenderHost.h`,
`RenderTexture.c`, `RenderTexture.h`, `emission-profile.json` and
`texture-emission-profile.json`. Capitalization matters: generated
`RenderHost.h` and the native effect `render-host.h` are separate interfaces.
Emission profiles bind the generated C and their complete current source maps;
the actual supplied headers have separate byte identities. The selected compiler
must match its recorded and current compiler source digest. There is no stale
compiler fallback or shader emission in this tool.

The snapshot copies every declared physical buffer and texture plus the whole
package. It rewrites copied references relative to `inputs/`, verifies the source
draw count covers the entire retained draw packet, inventories every staged file,
then checks both the archive inventory and hashes. Bounds are 384 files, 64 MiB
per physical input, 1 GiB per closed stage/archive and 64 MiB per raw readback.
`rgba8` and `rgba16f` are supported. An admitted preparation fixes its format;
`--format` selects the format when creating a new preparation.

Remote execution requires an explicit host and `--measure`:

```sh
COAX=/path/to/coax coax/web/tool.sh aethyr/render/tests/measure-native-frame.coax \
  --prepared /tmp/new-native-preparation --work-dir /tmp/new-native-measurement \
  --coax /path/to/coax --compiler-source-digest /path/to/source-digest \
  --measure --host existing-gpu-host
```

Use fresh empty work directories and immutable input storage. A preparation is a
source-fenced local snapshot, not an authenticated artifact from another party.
Its recorded compiler, tool image, source files and inputs must remain available
and unchanged for admission. Do not rebuild over a pinned tool image while it is
being measured. Concurrent writers are outside this profile.

Coax owns SSH/SCP ordering, deadlines, paths, archive admission, SDK identities,
compiler and execution arguments, retention and cleanup. Remote shell text only
executes a quoted compiler/native command and redirects its two owned log files.
Each remote operation has a bounded deadline; measurement work stops at 780
seconds, with cleanup bounded by 810 seconds overall. The exclusive remote
namespace is removed only after every available created output is retained.
Incomplete retention leaves that namespace named in the failed receipt.

The receipt binds the remote GCC executable, validation manifest and library,
NVIDIA ICD manifest and linked Vulkan loader. It retains full created compiler
and native logs, the ELF, raw image and native JSON, checking remote hashes before
and after download. It checks source, compiler, SDK and staged inputs again at
completion. A validation error or warning in the native record, or any validation
message in the complete execution logs, fails the measurement. Transport output
remains bounded if a remote SDK fails before owned log creation.

Local controls are `check-native-frame-measure.coax` (complete installed metadata,
bounded output, malformed JSON and exact byte-cap boundaries) and
`check-native-frame-prepared.coax` (admitted snapshot, corrupt archive, manifest,
header and compiler digest). They execute no GPU work. The pure policy has its
own checks and a zero-survivor mutation gate.

The retained Everglade measurement used all 190 source draw rows, eight physical
textures including the 36-layer atlas and its ten mips, and the current e8 Coax
generated backends. The RTX 4080 produced a 320×180 `rgba16f` frame with zero
validation errors or warnings. This records one raw execution; numerical image
agreement is a separate gate. The earlier admitted-count header and the earlier
SDK metadata capacity refusal remain recorded failures. Detailed identities and
the full executed source snapshots are in
[`aethyr-native-frame-measurement-2026-11-02.json`](../../../docs/verification/aethyr-native-frame-measurement-2026-11-02.json).
