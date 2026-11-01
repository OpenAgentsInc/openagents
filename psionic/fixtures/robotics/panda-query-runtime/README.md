# Owned queries in the neural runner: build and admission checkpoint

The updated wrapper has an explicit `PANDA_QUERY_SOURCE=owned-shadow-v1`
build profile. It links the existing resident Coax neural provider to the
owned Panda controller-sample selector. The binary was built and linked;
**no new neural invocation, simulator execution or episode was run here.**
The earlier paired 500-step recorded-action experiment remains its separate
qualification in `../panda-query-source/` and was not repeated.

With the option unset or empty, the wrapper retains its original SDK route,
session aggregate, numeric flags, profile, record schema and public ABIs.
The new option belongs to the updated wrapper/build and its versioned binary;
it does not retrofit an already built private legacy binary. Every nonempty
option requests the new build. Its Coax descriptor accepts only the exact
`owned-shadow-v1` name; unknown names and a missing runtime choice refuse
before opening the neural context. An owned failure never selects SDK mode.

## Admit the model before using the owned math

The owned FK/Jacobian/inertia/bias implementation is a fixed coefficient
profile, not a model importer. Counts and raw telemetry cannot certify it.
The new authority gate therefore admits only the original qualified model:

| Input | Required SHA-256 |
| --- | --- |
| Rewritten `demo_000` scene | `95a4049c1454ffd77b2185f0a21966832c5be55af67f64c2b110cc32e617d02f` |
| Original reset | `e1c2d3ee5192158b0f896456c1e312ea8ed5eef001862cae3d8c551ffb26eddc` |
| Loaded MuJoCo 3.3.1 library file | `f8c9735ed948d6830d1d3d31d50216ec14ca6a6e95c7346a4dc8300e1b8ffed4` |

C reads files and identifies the loaded SDK implementation through its symbol
address; it invokes no SDK API for this preflight. Coax hashes the observed
bytes and checks their digests. Only after the model passes does it expose
the 85 unique asset paths from that exact authenticated XML. Every referenced
asset must match its original digest. All requested cases pass before the
expensive neural context opens. Directory basenames confer no authority.

The paths are borrowed fixed ranges for one admitted XML, not a generic XML
parser. `asset-profile.json` binds the original public acquisition manifest,
relative source attribution, exact byte ranges and hashes. Raw XML/assets and
generated binaries remain private; execution receipts retain paths and hashes.
This is local snapshot authentication under the declared immutable-file/build
contract, not remote attestation or protection against later file replacement.
The recorded build also binds the unchanged qualified SDK header inventory.

A real file-only preflight caught the old one-shot SHA API's 1 MiB limit.
The new Coax adapter admits files up to 16 MiB, plans full 1 MiB updates and
the remaining tail, and uses the unchanged streaming SHA implementation.
The largest referenced asset is 11,230,178 bytes. It stages the 32-byte digest
and refuses bad length/output/alias metadata atomically. Its source-visible
sized helper assumes admitted spans/chunk metadata; it is not another gate.
Private 112-byte state, fixed output and the checked total/update limits
satisfy the streaming functions' necessarily-OK internal preconditions.

## Preserve the two observation stages

Request observations and both camera captures retain the existing cached SDK
stage, before the next forward. The neural request still uses the SDK's
right-hand BODY quaternion and grip SITE position. An owned fresh sample is
not substituted into that mixed-age observation recipe.

The new private control helper executes:

1. SDK forward effect, then its Coax clock receipt.
2. SDK telemetry and the named nine-coordinate/nine-rate effect.
3. Coax `pqs_select(1,...)`, then `pk_sample`, then `pk_control`.
4. Existing scenario injection on the selected sample, then the existing
   controller in the common loop.

Every failed clock result is adopted before returning. The caller's sample
publishes only after the helper succeeds. Scenario4's bias fault remains
after selection, so owned construction cannot hide it. SDK integration,
contact, rendering and shadow geometry remain; this profile does not remove
SDK geometry queries. Source-visible helper/POD calls are not model-provenance
certificates. Entry preflight supplies that fixed-model admission.

The owned build explicitly adds scalar-libm sin/cos flags. Its sanitizer mode
adds fail-fast recovery control. The legacy flag recipe is unchanged. The new
combined policy/query profile and case schema distinguish trace identity:

```jai
panda-libero-mujoco331-gr00t-wide-fixed-noise-owned-query-shadow-v1
psionic-panda-owned-query-closed-loop-v1
```

## New evidence and reuse

- New authority roots: **102 + 186 + 41 = 329/329 mutants killed**, with no
  equivalent waiver. Dependency baseline checks remain part of entry runs.
- Independent original-XML/public-manifest native checks: **3,217 assertions**
  in debug, release and fail-fast ASan+UBSan, all 85 path/digest entries and
  2,816 individual digest-byte corruptions.
- New streaming adapter: ten independent `hashlib` vectors through 16 MiB
  in all three native modes; unchanged old crypto gates were not rerun.
- Actual file-only preflight: **88 successful file hashes**, including the
  SDK library and all 85 assets, with zero SDK API, policy-open or NN calls.
- Parent's mocked effect-order proof: 13 control contexts and two preflight
  refusals, **551 assertions per native mode**, no SDK/NN/steps. It covers
  ordering, fail-stop adoption, atomic sample publication and fault overlay.
- Actual full neural-runner build/link under owned fail-fast sanitizer flags,
  reusing unchanged policy sources, shader bundle and Metal host object. The
  linked executable was never run.

Historical receipts remain unchanged. `current-binding.json` binds every old
function variant, header declaration/layout and non-function C global/preamble
to the refreshed aggregate. The only identifier substitution is one source-
private generated spatial helper and its caller; concrete signatures and
helper bodies match exactly. No numeric, diagnostic or string normalization
is used. New hash/display functions match their executed native proof.
The original control/current helper bodies are unchanged.

`native.json` and `effect-order.json` retain their executed original C/header
identities. `hash.json`, `file-preflight.json`, `current-binding.json`,
`wiring.json` and `build.json` compose the current checkpoint. Initial failed
file preflight and tooling-only failures are identified in `qualification.json`.
No passing numerical/model/SDK/mutation corpus was repeated for confirmation.

## Build only

With separately acquired, immutable inputs and the qualified shader cache:

```shell
PANDA_QUERY_SOURCE=owned-shadow-v1 PANDA_BUILD_ONLY=1 \
  PANDA_SHADER_CACHE="$QUALIFIED_SHADERS" \
  psionic/tests/robotics/run-panda-loop-metal.sh \
  "$GROOT_SHARDS" "$COSMOS_PST" "$NOISE_F32" "$PANDA_PAIRS" \
  /tmp/new-owned-query-build 1 0 000
```

This stops after linking. A genuine neural-plus-owned-query execution is the
next unqualified integration step; neither task success, hardware validity,
solver/contact replacement nor whole compiler boundedness/clock guarantees
are established by this build and mocked-order checkpoint.

## First executed neural/owned-query prefix — 2026-11-01

The sections above retain the original build/admission checkpoint verbatim. Their zero-execution fields are historical evidence and are not rewritten. A [separate executed-prefix receipt](../panda-owned-query-neural/README.md) now qualifies the already-linked `owned-shadow-v1` binary on the exact original `demo_000` reset/assets with genuine GR00T and the original OSC. No build, old model gate or standalone verification preflight was repeated. The product performs its required 88-file input admission on entry.

The first prescribed invocation completes nine action ticks, two model calls and 225 live SDK steps. The mandatory 225-step recorded-control replay matches every one of the 45 F64 state words exactly. There are no holds, refusals or torque-clipped coordinates; fail-fast sanitizer stderr is empty. Task success was not required and was not observed.

Independent stdlib auditing checks 225 canonical records, all numeric fields, the initial seed and every hash/footer link, clocks to 3.331e−16 seconds and the serial-1-to-2 transition across the eight-row request boundary. The first eight consumed actions match the saved first decoded output. Its frame-0 hash match is diagnostic only; the full second decoded array is not persisted.

The frozen source path implies four offscreen capture calls, 452 explicit SDK forwards including resets and 226 robot-state/owned-sample paths. These are source-backed counts, not newly instrumented per-function telemetry. The 450 integrated SDK steps comprise 225 live and 225 mandatory replay updates. Cameras, request geometry, contacts, actuation and integration remain in the SDK. This is separate from C/D scheduled PD and owned Euler. It establishes no task completion, LIBERO benchmark, hardware or real-time guarantee. The original 125-source inventory excludes this README; that inventory and all original receipts remain unchanged.
