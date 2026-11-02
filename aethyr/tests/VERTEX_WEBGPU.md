The clip-output and Dead Gears VS diagnostic executors use the shared Coax
primitive runner. Coax owns their typed descriptors, fixture validation,
metadata, hashes and verdicts. The existing browser effects and CDP/session
helper are unchanged.

```sh
COAX=/qualified/coax coax/web/tool.sh aethyr/tests/primitive-webgpu.coax \
  clip-output PROBE.wgsl RESULT.json CASES.json
COAX=/qualified/coax coax/web/tool.sh aethyr/tests/primitive-webgpu.coax \
  deadgears-vs PROBE.wgsl RESULT.json CASES.json
```

The clip probe retains all eight jobs, the exact 48-byte uniform assembled
from each three-by-four clip matrix, rectangular fixture dimensions, readback
pixels and original IDs/instances. The VS probe retains all 96 jobs, both
opaque `u`/`objects` blobs, their declared byte lengths, the original layouts
and both bind groups, uniforms, selectors and all four readback channels.
Every output must remain finite. Same-input driver qualification compares
every raw u32 word exactly; no numerical tolerance or precision mask is added.

The retained VS case file names an earlier production source epoch:
`gpu/deadgears.coax` SHA256 `1065de3d…`. Current source is `fc2c7794…`, after
canonical capsule directions and box winding changes. Both old and new tools
refuse that historical fixture against current source before Chrome starts.
The fixture hash is preserved. To examine the matching retained snapshot,
the Coax tool accepts an explicit source root:

```sh
COAX=/qualified/coax coax/web/tool.sh aethyr/tests/primitive-webgpu.coax \
  deadgears-vs PROBE.wgsl RESULT.json CASES.json --source-root SNAPSHOT_ROOT
```

Every declared source hash still has to match; the result records the canonical
`source_root` and every source/resource hash. This qualifies the driver switch
on those exact historical inputs. It does not qualify today's production VS,
an application image or a current producer build. No shader was reemitted for
the switch.

Qualification used the original Dead Gears clips, the existing Ice Cream clip
dataset read-only, and the retained 96-job VS inputs: 112 jobs and 448 raw u32
channels agree, including the original metadata and browser/device fields.
The independent IEEE754 oracle recodes nested clip/ID metadata as exact
binary64 words; Coax owns the old/new equality decision.

The Coax loss harness checks a raw-word loss, changed clip-vector and selector
metadata, source/resource hash failures, a declared-byte-length mismatch and
an out-of-range clip pixel. Source/resource hash failures are also checked
against the frozen old executor. It builds no shader and starts no Chrome for
the preflight refusals.

Source/compiler identities, the 339/339 zero-survivor policy run, all 110
release checks, exact input joins, refusal controls and archive readback are in
`docs/verification/aethyr-vertex-webgpu-port-2026-11-02.json`. The explicitly
qualified older `1fac…` compiler is exploratory evidence, not represented as
matching current main.
