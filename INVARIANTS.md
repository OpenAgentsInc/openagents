# Invariants

## Psionic rank-five original inventory and tensor access

`psionic/lib/model-io/rank5.coax` validates every BF16/F32 tensor, retaining up
to five dimensions and requiring unique names, disjoint ranges and complete
payload coverage. Public inventory is published only after the entire file is
valid. `access.coax` checks an expected name, dtype, shape, shard and byte range
before returning an original byte view; finite bounded decoding stages output.
Inspection does not authenticate the file. Callers must authenticate and retain
immutable originals and metadata, and provide valid, disjoint writable storage.

Check: `psionic/check.sh` includes authority mutation and native plain/sanitized
checks. See the [rank-five record](docs/verification/psionic-rank5-2026-10-29.md)
for exact evidence and the separate bundle-authentication boundary.

## Psionic text rotary positions are deterministic and staged

`psionic/lib/qwen-rope.coax` generates text-only Qwen rotary coefficients with
64 rotated dimensions and theta 10,000,000. It checks positions and capacities,
requires disjoint writable buffers and stages coefficients before publication.
Caller pointer validity, extent, alignment and exclusive access are obligations.
This does not implement image-position rotary semantics.

Check: `psionic/check.sh` includes zero-survivor authority mutation and native
plain/sanitized reference checks. See the
[hybrid executor record](docs/verification/psionic-hybrid-executor-2026-10-29.md).

## Psionic hybrid execution publishes a complete block

`psionic/lib/qwen-model.coax` runs a caller-supplied ordered layer sequence.
Linear layers process tokens causally and full-attention layers process the
whole block against their live prefix. Coax validates matching state and layer
shapes and stages activations, recurrent/convolution state and KV additions;
a failed layer cannot publish partial output or state. Unused KV tails remain
unchanged. Weight mapping, model identity and rotary positions remain caller
obligations. The C adapter observes nested spans only; valid pointers, extents,
alignment, immutable weights and exclusive access remain caller obligations.

Check: `psionic/check.sh` includes independent nonzero linear and cached
full-attention references, authority mutation and native plain/sanitized tests.
This bounded executor alone does not establish complete model execution.

## Psionic decision blocks fork all live hybrid prefix state

`psionic/lib/qwen-decision.coax` prepares every Q/K/V row in a current block
before full attention. Queries see every prior-prefix and current-block key;
sibling blocks are processed from separate snapshots. DeltaNet remains causal.
The full block stages outputs and new KV entries until the entire block succeeds.

`psionic/lib/qwen-prefix.coax` snapshots all convolution histories and recurrent
matrices plus the live KV prefix of every full-attention layer. It requires an
exact shape and matching nonzero caller-supplied 32-byte model identity; it does
not authenticate that identity or the source history. Destination publication
is atomic on success, scratch may be dirty on refusal, and unused KV tails are
left untouched. Caller pointer, extent, alignment, state identity, lifetime and
exclusive-access obligations remain explicit; C observes address ranges only.

Check: `psionic/check.sh` includes independent nonzero block/fork fixtures,
debug/release zero-survivor mutation and native plain/sanitized checks. See
[chapter 8 verification](docs/verification/psionic-ch08-2026-10-29.md) for the
CoderOS CPU scope and remaining whole-model and upstream-reference gates.

## Psionic Qwen-style causal layers publish complete state transitions

`psionic/lib/qwen-block.coax`, `qwen-linear-layer.coax` and
`qwen-full-layer.coax` compose offset-RMS, original BF16/F32 projections,
convolution/DeltaNet or gated full attention, residuals and SwiGLU. The bounded
CPU profile admits the target Elephant and Mammoth per-layer dimensions;
it does not claim full-checkpoint execution or upstream bitwise parity.

Each linear layer stages its convolution history, recurrent matrix and output
until the full layer succeeds. Each full layer stages its next KV entry and
output. Invalid shape, length, address observation, non-finite input and
arithmetic failures do not publish those outputs or states; workspaces may be
dirty. The full layer's present path is single-token causal. Callers retain
valid allocation/lifetime/alignment, exclusive access and consistency of state,
position, weights and supplied rotary coefficients.

Check: `psionic/check.sh` contains debug/release authority mutation, independent
full-layer fixtures and native plain/ASan+UBSan checks. See the
[chapter 7 record](docs/verification/psionic-ch07-2026-10-29.md) for tested
limits and the separate CoderOS CPU results. Decision masks and full model
execution remain future work.

## Psionic hybrid primitives stage attention and recurrent work

`psionic/lib/delta.coax`, `qwen-attention.coax` and `qwen-matvec.coax` own
bounded Qwen-style convolution/recurrence, split-half rotary and gated grouped
attention, and original BF16/F32 row projections. Bounds include 16 key heads,
48 value heads, 128-wide recurrent dimensions, 32 full-attention query heads,
256-wide full heads, 16,384 visible positions and 32,768 matrix rows/columns.
The delta `A_log` upper bound is 80. Shape admission precedes multiplication.

The delta step stages the next convolution history, recurrent matrix and output
before publishing any of them. Full-attention preparation and matvec publish
only complete outputs; masked attention stages output in scratch. Finite input,
coefficient, mask, capacity and overlap checks are explicit; scratch may change
on arithmetic failure. Callers must keep valid aligned allocations, truthful
extents and exclusive access; native wrappers only observe address ranges.

Check: `psionic/check.sh` includes debug/release authority and mutation gates,
independent scalar fixture generators and native plain/sanitized runs. The
[hybrid verification](docs/verification/psionic-hybrid-2026-10-29.md) defines
the scope; these primitives do not yet run a complete model or establish
upstream numerical parity.

## Psionic causal attention publishes a complete block and cache entry

`psionic/lib/attention.coax` owns a bounded causal dense block with learned RMS
scales, row-major Q/K/V/output and SwiGLU matrices, grouped-query heads,
caller-supplied interleaved rotary coefficients and a position-major KV cache.
Width is at most 16 with an even head width, hidden width at most 32 and cache
capacity at most 16. Shape checks precede multiplication. Each score and
intermediate vector is narrowed to binary32; products and reductions use the
chapter 3 compensated binary64 profile without FMA or reassociation.

Inputs, weights, coefficients and the live cache prefix must be finite; rotary
coefficient magnitudes are at most one. The caller owns the chosen rotary
convention and ensures cache positions are sequential and correspond to the
same immutable shape, weights and table. A step stages output and the new cache
entry; failure preserves both. A full pass stages the entire sequence, and a
failure preserves its public destination. Unused writable tails are unchanged.
The native adapter observes aliases; valid aligned allocations, truthful
extents, lifetimes and exclusive access remain caller obligations.

Check: `psionic/check.sh` includes debug/release Coax and zero-survivor
mutation, independent full-sequence fixtures, native debug/release and
ASan/UBSan checks, causal isolation and book examples. The chapter 6 record
scopes the evidence: this small causal profile does not establish Qwen3.5,
block-bidirectional decision masks or Elephant/Mammoth execution.

## Psionic text requests publish complete bounded byte/token layouts

`psionic/lib/text.coax` and `psionic/lib/text/` own `qwen35-text-v1`: frozen
tagged Base Labs choice/noul/score prompts, original Qwen byte-BPE/added-token
data, Unicode-17 NFC and split classes, and shared token-prefix matching over
complete branch encodings. Original and packed tokenizer identities are pinned;
callers admit the exact table SHA-256 once and keep those bytes immutable and
alive for subsequent calls. Layout parsing is not a fresh identity admission.

State strings preserve whitespace; instructions/descriptions use Python
Unicode stripping. Structured entries preserve insertion order and exact integer,
boolean/null/string values with two-space JSON formatting and a 4,300-digit
integer limit; duplicate keys and
floating/exponent JSON numbers refuse. Choices admit 1–255 options, noul has
yes/no slots, and the pinned score schema admits 1–10 levels. Question IDs never
enter the prompt. The exact disabled-thinking assistant prefix is preserved.

Each input/rendered branch is at most 65,536 UTF-8 bytes, each candidate/full
branch at most 16,384 tokens, and a split has 1–32 branches. BPE pieces are at
most 1,024 bytes, with a sum-of-squared-piece budget of 16,777,216; the encoder
reports this conservative bound as `work`, zero on failure. Combining runs
admit at most 128 nonzero classes. JSON has depth 16 and 256 members per
container. No truncation, option pruning or silently changed template is used.

Public prompt bytes, token IDs and split metadata change only on success;
workspaces may change on refusal. Split intervals refer to flattened complete
encodings; `answer` is the final token position within its complete branch.
Native C observes every writable and nested read-only span; callers retain
pointer/extent/alignment/lifetime/immutability and exclusive-access duties.

Check: `psionic/check.sh` runs debug/release checks and zero-survivor mutation,
original-data token/request fixtures, independent Unicode/regex/BPE oracles and
real native storage in plain and ASan/UBSan builds, plus previous chapters and
the book gate. See `docs/verification/psionic-ch05-2026-10-29.md`. The retained
corpus uses an owned oracle; broader upstream-runtime parity and whole-model
execution remain separate qualification gates.

## Psionic original-weight admission binds complete immutable byte inventories

`psionic/lib/model-io/` owns `original-weights-v1`: rank 0–4 BF16/F32
Safetensors inventories with exact positive shape/byte intervals, no overlap
or unindexed payload, unique names, bounded JSON and explicit unsupported
refusals. Limits are 64 GiB per raw weight file, 128 GiB per weight bundle,
16 shards, 4,096 tensors, 1 MiB per header/index and 128-byte printable
unescaped ASCII identifiers. Header metadata has at most 64 string pairs.
These artifact budgets are separate from CPU execution budgets.

Bundle admission binds exact original config/tokenizer/index/shard SHA-256
hashes and shard sizes to caller-provided trusted pins, complete expected
name/dtype/canonical-shape/shard inventory and exact shard-index coverage.
Labels/hashes do not authenticate their source or establish model semantics.
Names borrow immutable original headers; callers retain those bytes unchanged
and alive until the descriptors die. Full inventory is published only on
success; drafts may change on refusal. Config/tokenizer remain opaque bytes.

Decoding reparses the file, validates a named window of at most 1,048,576
values, and checks every selected encoding for finiteness before converting
BF16/F32 to F32 scratch and then destination. Every refusal preserves BOTH
float spans and reports zero written/bytes; metadata workspaces may change.
Success preserves unused writable capacity. Native wrappers observe all
writable spans against each other and every nested read-only view; pointer
validity, truthful extents, alignment, immutability, lifetime and exclusive
access remain caller obligations. There is no concurrent transaction claim.

Check: `psionic/check.sh` runs the authority/import closure in debug/release
with zero-survivor mutation, independent owned original-file/hash fixtures,
all BF16 encodings and F32 representation/alias/truncation cases under native
debug/release and ASan/UBSan, plus earlier chapters and the book gate. See
`docs/verification/psionic-ch04-2026-10-29.md`. These checks do not qualify
full Elephant/Mammoth execution, every Safetensors dtype or a larger backend.

## Psionic CPU numerical operations publish only complete admitted results

`psionic/lib/ops-contract.coax` owns `CPU-f32-wide-v1` operation/resource
admission; `psionic/lib/ops.coax` owns its checked data boundary and kernels.
The profile has contiguous binary32 row/matrix storage, explicit binary64
products and compensated reductions, round-to-nearest arithmetic without
FMA/reassociation, and one narrowing per output. Shapes/typed counts reuse the
1,048,576-element cap; matmul also admits at most 1,048,576 multiply-add pairs
by a division-before-multiplication check. It does not inherit M0's magnitude
cap or silently change that trainer's arithmetic.

All float operands, including masked logits/unselected rows, must be finite.
Masks are bytes 0/1 with an allowed value in every row. Indices, canonical
parameters, exact read-only view counts and disjoint writable capacities are
validated before staging. Unscaled RMSNorm/LayerNorm put positive finite
epsilon inside the root. Add/sum/dot/matmul refuse a computed wide value
outside the finite binary32 interval. Every failure reports zero written;
pre-execution refusal preserves all spans, arithmetic failure may dirty scratch
but leaves destination unchanged. Success preserves read-only storage and
unused writable tails. Typed native adapters only observe ranges; C callers
retain pointer/extent/alignment/lifetime and exclusive-access obligations.

Check: `psionic/check.sh` runs each authority root's debug/release checks and
zero-survivor mutation gate, Decimal(120) fixtures independent of the Coax
algorithms, analytic cancellation/magnitude cases, actual short/aliased typed
storage and a maximum-work matmul in plain/sanitized debug/release builds,
plus all earlier chapters and source-linked book/transcript checks. See
`docs/verification/psionic-ch03-2026-10-28.md`. This qualifies a bounded CPU
profile, not learned affine layers, arbitrary layouts, gradients or devices.

## Psionic tensor storage admits shapes before touching caller data

`psionic/lib/tensor.coax` owns contiguous binary32 shape/index/copy admission:
rank 0–4, positive active dimensions, zero inactive dimensions, a scalar count
of one, and at most 1,048,576 elements / 4,194,304 logical data bytes. Product
limits are checked by division before multiplication. Failed layout/index
results have zero payload fields; callers must inspect status.

The synchronous staged copy requires an exact logical source view and sufficient
disjoint destination/scratch capacities, including their declared tails. Every
source value must be finite; chapter 1's magnitude cap is not inherited. All
refusals precede any write and report zero written elements. Success preserves
source bits and unused writable capacity on the tested Apple LP64 CPU profile.
The shared native `psionic/platform/spans.h` observes address ranges; Coax owns
admission. Native callers still own pointer validity, truthful allocation
extent, alignment, lifetime and exclusive access during the call.

Check: `psionic/check.sh` runs the chapters' Coax debug/release checks and
zero-survivor authority mutation gates, independent native layouts/indices,
short allocations, aliases/non-finite representations and a maximum-size copy
in plain/sanitized debug/release builds, plus source-linked book/transcript
checks. See `docs/verification/psionic-ch02-2026-10-28.md`. Chapter 3 now adds
a separately qualified dense CPU profile. Autodiff and GPU execution remain
planned; these checks are bounded evidence,
not general memory safety or concurrent atomic publication.

## Psionic's first trainer publishes only completed bounded work

`psionic/lib/learn.coax` owns the one-parameter CPU model, its mean squared
loss/analytic gradient, fixed `1/8` updates, prediction and admission. Its
`#authority` checks cover 1–16 paired samples, 1–64 updates, two caller trace
buffers, and finite input/target/weight magnitudes at most 16. The native
adapter only observes span overlap. Input storage and the published loss
trace stay unchanged on refusal; scratch can contain partial work after an
update leaves the weight domain. C callers still own pointer validity and
lifetime. This is a bounded learning example, not tensor, autodiff or GPU
support, nor a claim that arbitrary admitted data has decreasing loss.

Check: `psionic/check.sh` runs authored checks and a zero-survivor mutation
gate, independent native oracles and real alias/non-finite/short-buffer cases
in debug/release under ASan/UBSan, and exact book source/transcript/status
checks. See `docs/verification/psionic-m0-2026-10-28.md` for evidence scope.

## Native debug maps and performance reports are scoped observations

Coax 0.13.0 can map emitted C statements to original physical Coax files.
Schema-1 maps retain source hashes and authored procedure/variable names;
the optional LLDB presenter refuses stale files and distinguishes separate
libraries by physical owner. Plain C emission stays unchanged without the
debug option, preserving exact seeds. Native debug objects must be retained
where the linker references them. This is not a Coax expression evaluator,
complete type encoding, shader map or device-support guarantee.

Driver clocks, file reports, native compilation and linking are host effects,
never new `#run` capabilities. The checked Coax fingerprint implementation owns
map SHA-256. Timings and source maps do not certify compiler correctness.

Check: `coax/tests/debugging.py` uses actual Mac LLDB, native diagnostics and
sanitizer stacks, including multi-statement lowering and loaded units.
The live Simulator cast breakpoint reaches Coax through the Swift renderer.

The R0 workload and statistical policy live in `verse/benchmarks.coax`,
with authority checks and a zero-survivor mutation gate. The external harness
pins historical native source, its explicit reader adaptation and the real
app corpus before comparison; timing policy is separate from output and
allocation assertions. A successful correctness run must not be reported as
meeting a failed proposed timing budget. Measured allocation hooks have a
documented scope; Simulator clocks are not physical-phone performance.

Check: `verse/benchmarks/run.py` compares every player palette element,
independently recomputes ranks/medians, observes kernel allocation calls and
runs under ASan/UBSan. `R0UITests` drives the normal Grid/Metal path; inspect
retained frame counts and command errors too. Reference-adapter review,
independently enforced adoption gates and device/tail budgets remain open.

The pose rewrite retains the frozen reference, reader, asset, workload and
threshold. Coax owns clip/skeleton preparation, bounded decoded-row caches and
math; admitted parent-before-child order ensures every cached parent is written
before use. All accesses remain checked, zero-weight interpolation still
normalizes, and public quaternion defaults/target ABI are unchanged. Raw
median/tail evidence and explicit stack costs stay separate from adoption claims.

E0383 is a bounded checker diagnostic for direct local-array view/string
returns and simple aliases/slices/casts/conditional arms. Borrowed parameter
returns remain valid. Conservative false positives and interprocedural,
aggregate-contained, loop and nested-block lifetime gaps are documented; this
is not general ownership safety. Check: checker authority/mutation and seeds,
focused native/sanitizer/negative cases in `coax/verification-local-borrows.md`.

## One protocol for all OpenAgents surfaces

Root `proto/` and the OpenAgents Protocol specification define interchange for
all surfaces, including Verse. Coax is the product implementation target;
Verse is the engine name (formerly Metareal). Profiles select imports from one source tree,
not separate definitions per app, target or implementation language.

Check: `scripts/proto.sh check` compiles actual roots and profile closures.
Coax descriptor ingestion is checked by `coax/tests/proto_import.py`. Product
migration and domain/admission validation remain explicit consumer gates.

## Published contracts and signed originals retain identity

All `proto/compatibility/` snapshots and vendor source/license bytes are
immutable. Published names, numbers, types and presence must pass every frozen
descriptor baseline. Nostr/HOST/REACH and other signed/framed protocols retain
their original bytes and authority owners. A decoded DTO grants no authority.

Check: recovery hashes, compatibility mutation tests, isolated vendor builds,
and alpha missing-versus-zero/limits tests in `scripts/proto.sh check`.
These checks do not claim that deleted runtime admission code is installed.

## Engine rename has explicit readers and versions

Legacy IDs live in the migration registry, frozen evidence and byte-exact
recovered or upstream NIP source specifications. Current
asset catalogs use canonical IDs; their other required data is unchanged.
Migrations preserve original input, unknown fields and signatures, reject
collisions and never reinterpret an old signature under a new domain. Other
record/bridge/signature migrations remain gated on actual consumers.

Check: real frozen asset catalog round trip, idempotence, collision tests and
signature-domain refusal in `proto/tools/test_contracts.py`.
The NIP recovery source/tree identity is recorded in `proto/nostr/recovery.json`;
restoring historical text does not restore its retired implementation mandates.
Current official/external mirrors may advance beyond that historical tree only
with exact upstream pins and per-file hashes; `nips/openagents` is not synced.
Check both online pin parity (`scripts/check-nips-pin.sh --latest`) and offline
inventory/hash parity (`--offline`), including non-Markdown fixtures.

## No GitHub-hosted CI

Protocol and compiler checks run locally or on owned infrastructure. Do not
add GitHub Actions workflows. A schema check needs no running service,
production credentials or network download.

## The reduced iOS home has one product implementation

Coax owns its scene, animation, layout, reusable HUD touch policy and bounded
protobuf admission. Native adapters own platform lifecycle, serialized opaque
storage handles, packet transfer, font rasterization and Metal bindings. A
screen does not acquire chat or device permissions through this port, except
the microphone, which only the voice toggle asks for (see the voice section
below). The only network uses are the Spark wallet below and the voice host.
The home imports the `ios-home` descriptor profile for generated
HomeConfiguration and HudInvocation codecs. It accepts only its documented
schema-1 resource subset, rejects unknown/duplicate fields and non-minimal
varints, and emits existing HudInvocation bytes. Native adapters only transfer
bytes; the resource and invocation have no second hand-written wire codec.
The home is the Grid seen from high above, slowly orbiting, with only the Start
bar over it (`verse_home_hud_frame`). Start dollies the camera down to land
behind you; `verse/camera.coax` owns the overview orbit and the dolly path
(eased, from behind and above, never below where it lands, a cut under
reduced motion; a tap hurries it). Controls, the hotbar and the balance appear
only once it lands. The balance sits top right as whole sats with no grouping
(`₿1234`, or `₿–` with the reason spoken while there is none) and opens the wallet.
`verse/grid.coax` owns its projection, line art and
move/look/jump touch policy; `verse/player.coax` owns walking and the
camera; `verse/cast.coax` admits the bundled `Cast.bin`, blends your
character's clips and poses you and the robots. The native adapter only routes
the Start invocation there, passes numbers between the libraries each frame
and binds the buffers Coax admitted. A cast whose offsets, indices, joints or
floats fail admission is refused, not drawn. You collide with the robots
through `player.coax`: a capsule per robot bone, placed by `cast.coax` from
the same palette the robots are drawn with, pushes you out sideways and keeps
the camera in front of it. Robot line art takes its width and light by
distance from `light.coax`. A terminal (aethyr's desk and screen, from
`grid.coax`) stands 4 m ahead of the spawn; its boxes are both drawn and
solid, so `player.coax` stops you at the desk, lets you onto it only by
jumping, and keeps the camera out of it. The hotbar is aethyr's HUD slot:
`grid.coax` places it and claims touches on it before the sticks, and
`light.coax` draws it. Its one slot toggles the dance (`cast.coax` plays
Dance_Loop while you stand); walking or jumping ends it. The Grid has no
buildings, peers or network use yet.

Check: `coax/build.sh test`, `coax test verse/grid.coax`, `verse/camera.coax`,
`verse/player.coax` and `verse/cast.coax`, `verse/test_cast.sh` (the
real Cast.bin under ASan/UBSan, with corrupted copies refused, and you
walking into the Foreman's leg and stopping),
`PROTOC=... python3 verse/test_home.py`, the sanitizer harness and the iOS
UI tests (Start opens the Grid, you stand idle, the stick walks you forward,
a jump plays its clips, the Foreman stands by the start and its leg stops
you, the hotbar's slot makes you dance and a step ends it, the terminal's
desk stops you, rotation and
backgrounding keep it). Metal layout/stage
bindings remain an explicit native boundary; the home retains its initial
flat-storage graph while decoded configuration views use bounded Coax scratch
buffers. Labels are copied into resident storage before those views expire.
There is no claim of a full engine migration or general protobuf compiler.


## The iOS wallet seed is read, never written; sends go through Coax

`apps/openagents-ios/OpenAgents/Wallet/` opens the person's existing Spark
wallet from the Keychain item the earlier apps wrote (generic password,
service `com.openagents.app.spark`, account `seed-v1`, 16 or 32 bytes of
BIP-39 entropy, default access group). The app has no code that adds,
updates or deletes that item, and never makes a seed. A missing, locked or
wrong-length item is reported on screen and left as it is. The entropy
becomes English BIP-39 words only to open Breez Spark on mainnet with no
passphrase; the words are never logged, stored elsewhere or passed to Coax.
The word list is checked against its published SHA-256 before use.

The wallet sends and receives (Lightning invoices and addresses, LNURL,
on-chain, Spark). `Wallet/SparkHost.swift` is the only file that imports the
SDK and hands the screens plain values. `verse/wallet.coax` is
`#authority` and compiled into the app, so its checks run on every build. It
decides each send: `verse_wallet_may_send` passes a quote only while its
payment is quoted, before it expires, with the fee no more than the review
screen showed, and with amount plus fee within what is spendable. The payment
moves only through `verse_wallet_step`:
- one `sendPayment` per quote, under one idempotency key;
- an unknown outcome waits for the SDK and is never shown as a failure;
- a finished payment stays finished until the person closes it.

The phone's owner check (Face ID, Touch ID or passcode) comes last, before
the send. SDK errors are shown in words, never as the SDK's own text. Backup
of the words and exit data are later work (`docs/lightning/`).

Model-boundary exception: Debug builds launched with
`OPENAGENTS_THROWAWAY_WALLET=1` (the wallet UI test only) open a fresh wallet
from random words held in memory for that launch, in its own folder, never
the Keychain. Release builds compile this path out.

Check: `apps/openagents-ios/tools/test-bip39.sh` (published BIP-39 vectors);
`coax test` and `coax mutate verse/wallet.coax` (0 survivors); the iOS UI
tests (a phone with no wallet shows that and changes nothing, and
`WalletUITests` against mainnet with a throwaway wallet: every kind of
request is made, sends are parsed and quoted, and an empty wallet's send is
refused in words); and the owner's device showing the existing balance.

## Nostr projections cannot create authority or replace signed bytes

New `nostr.*.v1alpha1` values are source-pinned projections. A SignedEvent keeps
original JSON alongside ordered string tags and exact content; the published
common.v1 wrapper stays intact behind an explicit adapter. JSON null, missing,
zero and unknown fields remain distinguishable where the source requires it.
No DTO, parsed owner claim, label, command, revision link or decoded JWS counts
as signature verification, owner membership, relay identity, grant, CAS
acceptance or encryption. Decrypted memory/telemetry/metrics and identity
credentials must not be published, logged or placed in a public snapshot.

Check: `proto/tools/check_nostr_sources.py` and `scripts/proto.sh check` reject
stale mirror pins/hashes and schema inventory drift. Reference tests check real
event ID/original round trips, unknown fields, missing/null/zero cases, token
classes, byte bounds and selected-profile imports. Coax tests consume the actual
Event protobuf fixture through both the narrow reader and actual descriptor-
generated typed codecs. `coax/tests/proto_import.py` checks presence/oneofs,
repeats, unknown retention, writer capacity/alias errors, policy bounds and
native ASan/UBSan; its explicit protoc oracle checks interoperability. Codecs
borrow input/output storage, allocate no target memory, and reject unsupported
descriptors and duplicate known singular/oneof fields. Domain migration and all
cryptographic/relay admission remain explicit unimplemented gates.


## Check gates reject vacuous and uncovered success

`#forall` domains are finite and nonempty; range cardinality is computed with
widened arithmetic and values are enumerated lazily. `#authority` requires real
procedure execution during checks, not dead references. Empty suites,
uncovered/no-site mutation runs and malformed step budgets cannot report a
successful verification run. Coverage and mutation scores are not correctness
proofs or admission authority; equivalent-mutation exemptions remain reviewed
assumptions. JSON diagnostics retain concrete witnesses.

Check: `coax/tests/verify.py`, compiler ASan/UBSan signed-span checks and the
actual imported wire checks in `coax/tests/proto/runtime.coax`. Native aliasing
and non-finite representations remain in the target-side conformance suite.
Descriptor/importer/fingerprint-module/runtime hashes and generator/compiler versions are recorded
in generated C/headers. The authoritative pure runtime is
`coax/lib/proto/wire.coax`, with `#authority`; its checks and zero-survivor mutation
gate run before publishing a bootstrap compiler. `memory.coax` only observes
native storage overlap and delegates policy to the pure core. Physical addresses
remain a native ABI/sanitizer boundary, outside Coax check claims. Checks and
`#authority` are not copied into imported applications. Descriptor parsing and
generation live in `coax/lib/proto/importer.coax`, with `#authority`, bounded
checks and a zero-survivor mutation gate. Coax owns scratch capacities. Stage0
has proto imports disabled and compiles this actual Coax implementation to C11;
the final compiler links it and must regenerate identical importer C. There is
no C++ parser/generator fallback. `coax/lib/proto/fingerprint.coax` owns
SHA-256, runtime extraction, hexadecimal/provenance formatting and output
assembly, with its own authority checks and zero-survivor mutation build gate.
The native bridge only reads files, allocates Coax-requested buffers and copies
Coax results. It contains no hash or formatting fallback. The frontend, evaluator, C emitter, toolchain
and native host effects remain trusted; these gates are not a formal proof.
Shell byte copies embed the runtime from its authoritative Coax sources. Normal
compilation must not invoke Python, protoc or helper processes or depend on
adjacent runtime files. Explicit descriptor input reads are not permitted inside `#run`.

Check: legacy schema declaration hashes characterize output compatibility, not
correctness. Independent protoc vectors exercise interoperability. See
`coax/proto.md` for the evidence scopes. Relocated-binary/no-PATH iOS C/header
generation, native descriptor ASan/UBSan fuzzing and the real iOS simulator
flow in `coax/tests/proto_import.py` and app tests.


## The compiler lexer is Coax with an explicit generated seed

`coax/lib/compiler/lexer.coax` is the authored lexer, including keyword/operator
selection, locations, recovery, decoded strings, diagnostic text, integer
accumulation and decimal-to-binary64 conversion. `coax/src/lex.cpp` only supplies
owned storage and copies the Coax result into the native token ABI; it has no
native scanning or numeric-conversion fallback. Token ordinal ABI agreement is
checked when compiling the bridge.

A checked-in generated C/header seed builds the first frontend. Normal builds
must regenerate that seed exactly from the Coax source, pass its authority and
zero-survivor mutation gates, and reproduce the final compiler modules before
publication. Refreshing the seed is an explicit operation with a previously
trusted compiler. The seeds, C++ evaluator/emitter, native tree bridge, host/toolchain
and native ABI remain trusted; fixed points and finite checks are not proofs of
bootstrap security or of the whole compiler. Normal bootstrap and iOS code
generation remain Python-free.

Check: `coax test coax/lib/compiler/lexer.coax`, `coax mutate` on that file,
`coax/build.sh test`, and `coax/tests/lexer.py` (compiled Coax ABI/recovery,
independent exact-decimal and binary64 midpoint oracles, ASan/UBSan byte-pair and
random-input fuzzing). The real iOS Home/Grid/animation tests exercise product
code compiled through the Coax lexer.


## The language parser is checked Coax, with no native grammar fallback

`coax/lib/compiler/parser.coax` owns grammar/precedence, declaration lookahead,
syntax diagnostics and bounded indexed tree construction. Source proto import
syntax, top-level/Metal admission and prefix/count/path checks also live here;
the driver only resolves/reads/assembles their files. `src/parse.cpp` only
transports tokens and reconstructs arena objects/lists; native AST kind mappings
have compile-time ABI assertions. Raw token spelling lengths must survive
separately from decoded string content. The first syntax error is retained and
no partial tree is admitted. Node/message capacities come from Coax; recursion
admits at most 128 active grammar entries, reporting E0221 on exhaustion.

The generated parser C/header seed is explicit, refreshed with a trusted
compiler, and must reproduce exactly in the Python-free bootstrap. Its actual
Coax checks and zero-survivor mutation gate run before publication, followed by
final self-regeneration. C++ evaluator/emitter, toolchain, generated
seeds and native ABI remain trusted. Compatibility snapshots are not proofs.
Check: `coax test coax/lib/compiler/parser.coax`, mutation testing on that file,
`coax/build.sh test`, `coax/tests/parser.py` (independent tree assertions,
random expressions, bounds/stack limits, actual native bridge and kernel under
ASan/UBSan), plus the real iOS simulator tests from empty generated outputs.


## Type checking is Coax with no native semantic fallback

`coax/lib/compiler/checker.coax` owns semantic admission and diagnostics,
name/scope/type resolution, numeric folding/range/IEEE decisions, natural
layout, calls, control flow, checks and Metal admission. The C++ bridge only
supplies owned storage and reconstructs native records/references/lists after
success. The indexed input is the trusted parser's internal graph; no partial
typed tree is admitted on failure. E0381 bounds storage, native layout size
and 256 active semantic entries.

The explicit checker C/header seed reproduces in the Python-free bootstrap,
passes authority/check/zero-survivor mutation gates and self-regenerates in the
final compiler. Seed provenance, C++ evaluator/emitter, native ABI/storage and
toolchain remain trust inputs. Compatibility snapshots and finite mutation
scores do not prove semantic soundness or bootstrap security.

Check: `coax test coax/lib/compiler/checker.coax`, mutation testing on that
file, `coax/build.sh test`, `coax/tests/checker.py` (independent IEEE, natural
layout and policy expectations; actual emitted checker under ASan/UBSan),
complete compiler bridge sanitizers in `coax/tests/parser.py`, protoc
interoperability and the actual iOS Home/Grid/animation tests from empty
generated outputs.

## Haunted House is a native Verse realm

The Grid's lit arch at (-1.5, 0, 10) enters Haunted House by tapping its name
or walking through it. Each world owns the window's full Metal surface; it
is not a modal web view. Grid returns to the same app, with the home, cast
and wallet intact. The scene's decisions, geometry packets, motion, trigger
schedule, footsteps/spatial sound, dialogue, teaching/character behaviours,
quest, editing and protobuf documents are in `verse/haunt.coax`.
`hud.coax` supplies reusable hotbar layout; `haunt-light.coax` emits the
material math as Metal. Native code binds lifecycle, fixed-capacity memory,
GPU buffers, UIKit/accessibility forms, audio clips and local file/share I/O.

The original current content (source 69848ca, not the older frozen house)
keeps all 120 objects, 21 doors, 13 rooms and three outdoor areas, the plain
roofs, pine/dead-tree forest without ground cover, Mortimer/Agatha and the
ghost/cat voices. All nine scare zones, including delayed thunder, the brew
morph and sheet ghost, run locally. This introduces no new network service,
account, microphone permission or payment path. Local copies and invitations
are actual proto files, not claims of a public-world publish. The retired
web public-world/owner/change-offer APIs and cloud NPC endpoint remain explicit
service parity gaps; see `docs/protocol/verse-haunt.md`.

`proto/openagents/metareal/haunt/v1alpha1` defines resource metadata and
editable snapshots. Geometry is separately bounded; every vertex/index and
draw range passes the Coax `#authority` buffer gate before upload/drawing.
Unknown identities in an imported save grant no authority. The matching base
revision, bounded finite overrides and unique IDs are checked before a save
changes the live scene. The platform retains bytes for every borrowed view;
undo rebuilds the base, then applies a complete override snapshot. Background
stops audio/input and saves; returning discards the world's native allocations.

Check: `verse/test_haunt.sh` (Coax checks, 109/109 buffer-gate mutants killed,
ASan/UBSan and real assets; with `PROTOC`/`PROTO_PYTHON`, source content parity
and official protobuf writer interoperability in both directions), Grid/player
checks, `HauntUITests` (real Metal portal, conversation, placement, save/reenter,
cameras/map/rotation/reset) and the existing `HomeUITests`.

## Voice: audio leaves the phone only inside a turn, and only to our voice host

The schema is `proto/openagents/voice/v1alpha1/voice.proto`; one `Frame` is one
WebSocket binary message, encoded and decoded only by the generated Coax codec
(`verse/voice-wire.coax`), with no second hand-written codec.

- **Turns are Coax decisions.** `verse/voice.coax` decides when a turn starts
  and ends, barge-in and the media epoch. The phone sends uplink audio only for
  frames the engine marks as part of a turn (`OA_EV_FORWARD`) plus the turn's
  pre-roll. Silence between turns never leaves the phone.
- **Decisions are never awaited.** Audio flows under the last decision. A
  `FrameDecision` from any decider applies only within its stream and media
  epoch and before its deadline, and every numeric field is clamped. A slow or
  missing decider can fail to steer audio, but can't stall it.
- **Who may be heard and what speech may cost are `#authority`.**
  `verse/voice-floor.coax` (consent, input floor, routes by explicit binding
  only, never by reading the words) and `verse/voice-meter.coax` (free tier
  and price; no provider call for a turn the meter refuses) run with zero
  surviving mutants.
- **Only the voice host calls a speech provider.** The phone and web clients
  never hold Google credentials. The host never logs tokens or audio. Renders
  are cached by SHA-256 of profile, revision, text and rate.
- **The microphone is opt-in.** It opens only after the person turns on the voice
  toggle and sees that Google processes their speech.

Model-boundary exception: Debug builds accept `-OAVoiceFixture` (a bundled
recording through the same uplink path as the microphone) and
`-OAVoiceObserve`; Release builds compile both out.

Check: `verse/test_voice.sh` (ASan/UBSan; every frame kind round-trips and
decodes with protoc; turns, barge-in, decisions, jitter buffer, fixtures);
`coax test verse/voice*.coax`; `coax mutate` on floor and meter (0
survivors); `verse/voice-host/test.sh` (offline, and live Google);
`apps/openagents-ios/tools/voice-sim-test.sh` (the simulator UI test, offline
and live).

## Source composition preserves physical ownership

`coax/lib/compiler/sources.coax` owns literal load syntax, canonical identity
admission, cycle/budget rules and verification-scope decisions. Native code
resolves/reads regular files and transports owned storage. A canonical source
loads once; an active cycle fails; graph source reads are bounded before and
during I/O. Loaded units cannot close each other's delimiters. There is no
source-load interpreter effect or native syntax-policy fallback.

A physical unit's `#authority` requirements survive loading without spreading
to unmarked dependencies. The internal checker graph receives source-owned
scope annotations, never untrusted wire claims. Actual executed-check coverage
still gates authority functions, and mutation covers reachable helpers while
requiring entry/authority coverage. Compiler/evaluator/emitter, native ABI and
filesystem behavior remain trust inputs; finite checks are not proofs.

Check: source-kernel authority checks and zero-survivor mutation, explicit
`sources.c/h` seed identity in the Python-free bootstrap, final regeneration
from `lib/compiler/compiler.coax`, and `coax/tests/sources.py` (real driver,
cycle/diamond/symlink tests, scoped authority, relative descriptors, original
file diagnostics/traps, graph limits, helper-free relocated compiler, native
ABI and ASan/UBSan with an independent widened budget oracle). The book
capstone and real iOS HUD consume loaded policy/types. `coax deps` is a checked
path inventory, not a lockfile, content cache key or native build manifest.


## Release status has one authored source and explicit limits

`coax/release.coax` owns the current compiler version and reviewed feature
catalogue. Coax renders the embedded CLI metadata, JSON inventory, accepted
feature reference and explicitly managed current-version regions. The
`#authority` validation model checks metadata shape/identity; it does not prove
feature semantics or grant runtime authority. Implementation, inventory schema,
historical introduction and public protocol versions remain separate.

Generated snapshots are reviewed bootstrap inputs. Normal bootstrap compares
them with fresh output, then compares the final binary before publishing it.
App generation and book publication refuse drift; the publisher checks before
reading credentials or creating a draft. Unmanaged historical prose is never
rewritten by version-value substitution and still needs review. Normal builds
and generation remain Python-free.

Check: `coax test/mutate coax/release-model.coax` (zero survivors),
`scripts/coax-release.sh --check --binary coax/bin/coax`, and
`coax/tests/release.py` (snapshot repair, version-domain separation, invalid
metadata, managed regions, stale binaries, real acceptance probes and
publication rejection before credentials/network). Workflow and limits:
`coax/RELEASE.md`.

## CPU reflection lowers to bounded ordinary values

`type_info`, `field_info` and `enum_info` are implemented in the Coax parser
and checker. Their schema-1 records describe the bootstrap LP64 CPU layout;
indices use declaration order and enum bits are zero-extended from the
underlying width. Dynamic selectors use ordinary bounds-checked indexing in
both build modes. Expansion shares a bounded checker pool; no native compiler
object or host pointer becomes a target value. The bridge only transports
nodes. Stable IDs, general derivation/plugins and other target layouts are
not supplied by these queries.

Check: parser/checker authority and zero-survivor mutation gates,
`coax/tests/reflection.py` (independent native size/alignment/offset oracle,
ASan/UBSan, invalid categories/indices, cross-unit layout and expansion limits),
and `verse/test_haunt.sh`. The actual app's buffer admission consumes the
reflected Haunted House stride, layer offset and compile-time scalar table.
Metal packing and source binary compatibility remain explicit native checks.

## Coaxial pages draw everything in Coax; TypeScript only for browser crypto

Everything a person sees on a Coaxial page (`apps/openagents-web/`, any app
served by `coax/web/`) is drawn by Coax on the WebGPU canvas: words, the
graph, the 404 page, the access terminal, presentations, buttons and links.
No visible HTML elements, no CSS-styled overlays, no DOM-built UI. The page's
JavaScript is the generic host (`coax/web/host.js`): it owns the GPU, feeds
input (keys, pointer, text) into the Coax module and does what the module
asks (navigate, submit a code). TypeScript exists only where the browser's
crypto and Nostr plumbing need it (`coax/web/unlock/`: phrase checks, NIP-44,
NIP-59, NIP-42, key storage); it hands results to Coax as data and draws
nothing.

Allowed HTML, none of it visible to a WebGPU visitor:
- the `<canvas>`;
- text for search engines, agents and screen readers, clipped out of sight
  while the GPU draws (the `.gpu main` rule);
- the text fallback a browser without WebGPU sees instead (an owner
  decision, 2026-10-28);
- an invisible `<input>` over a Coax-drawn text field (the rect Coax reports),
  for the keyboard, paste and the context menu; it draws nothing itself;
- the development-only error note `coax serve` shows on localhost.

Check: `coax/web/check-visuals.sh` (run by `coax/web/deploy.sh` before every
upload) refuses visible elements, inline styling of UI, and DOM-built UI in
the app's HTML, the host and `coax/web/unlock/`.


## Private projects answer non-members with nothing

A private project's contents (`docs/private-projects.md`) reach only its
members and the agents they register, from oa-api with a session cookie or a
NIP-98 signature. Anyone else, and any slug nobody made, gets the same 404
bytes from the API and a generic locked page that names nothing. Answers are
`no-store, private`; the oa-api backends have Cloud CDN off. Tokens are
stored only as hashes; sign-in links are single use, 24 hours, bound to one
member and carried in the URL fragment; sessions last 30 days. State-changing
requests are POSTs with a same-site Origin or a signature, and rate limited.
Agents never change agreed goals, members or the title, mint links or register
agents. Contents never go to the public relay or the site bucket, and the
public repo holds only the mechanism, never a project's contents.

Check: `coax/web/api/test_private.py` (via `coax/web/api/test.sh`),
`coax test apps/openagents-web/private.coax`, `coax/web/smoke.sh`. Threat
model and residual risks: `docs/private-projects-threat-model.md`.

## Our own testing is not usage

The team's visits (any browser an admin logged in from, the players and
emails in `coax/web/api/team.conf`, and every device seen in one of their
sessions) are kept with `auto = 'team'`, at the source (`analytics.ingest`,
and triggers for imported rows and playtest joins) and by
`analytics.team_tag()`, which the API runs on every start to backfill. Every
visitor count that leaves out `auto IS NOT NULL` leaves them out;
/admin/analytics leaves them out unless OUR TEAM is on and says how many it
left out. Automation keeps its own mark. `team.conf` holds opaque ids and
email hashes only, never an email.

Check: `coax/web/api/test_team.py` (via `coax/web/api/test.sh`),
`coax test apps/openagents-web/admin.coax`.

## Analytics never lose visibility

Every page keeps sending the analytics contract
(`coax/web/analytics-contract.json`, the same rows as `analytics.contract` in
oa-api): the landing's session-start and page-view, the galaxy's page-view,
and the Haunted House's page-view, world-enter, load-engine, area, objective,
quest, click and world-leave; Ice Cream Man's page-view, world-enter,
load-engine and world-leave; and Dead Gears' page-view, world-enter,
load-engine, wave and world-leave. A release never promotes a build whose staging
walk stops sending one: `coax/web/goldens.ts` plays the journey in headless
Chrome, runs in through the house's front door, clicks and leaves, reads
every batch the pages send (a realm's after its own walk, then leaves it)
and names what is missing, and `release.sh` stops
before production. In production the Health card on /admin/analytics shows,
per surface, the last event, the last hour and day, and which contract
events no page sent in a day. Changing the contract changes both lists in one
commit.

Check: `coax/web/release.sh` (staging walk), `coax/web/api/test_health.py`
(via `coax/web/api/test.sh`), which also checks the two lists agree.
