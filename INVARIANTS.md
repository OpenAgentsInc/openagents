# Invariants

## Pylon restarts reserve a durable provider-bound beacon epoch

`psionic/lib/pylon/generation.coax` admits only canonical, provider-bound
counter records and exact JSON integers through 2^53-1. Fresh state begins at
2 for migration from the legacy executable's generation 1; restart advances
the durable counter before any relay publication. A lifetime lock on a separate
sibling inode excludes a second process sharing the state while the counter
itself is atomically replaced. File and parent-directory fsync must both succeed
before Coax permits the epoch. Corrupt, foreign, exhausted, unreadable or
uncommitted state fails closed, without connecting to the relay.

The owner supplies one authoritative persistent state directory per provider;
deleting/restoring an old counter while reusing its key defeats that premise.
The counter is public metadata, not signing-key custody, remote attestation,
durable request deduplication or synchronization across cloned identities.
Host C performs file/lock effects; all parsing, binding, increment, exhaustion
and commit admission are Coax. Checks: the focused generation gate and the
local encrypted task/restart runner in `psionic/tests/pylon/`.

## Psionic layer-major prefill preserves route order and complete chunk publication

`psionic/lib/deepseek/prefill*.coax` owns `deepseek-prefill-cpu-wide-v1`.
Stable expert groups retain original token/route ordinals and their weights.
Each active expert packet is staged once per layer/chunk; gather/expert/scatter
restore those ordinals. Eight-row matrix tiles share decode while every output
keeps chapter 63's increasing-K TwoSum/correction reduction and final F32 cast.
Activation grids, clamp/SiLU and pre-down route weighting remain unchanged.
Combine narrows the routed sum before adding the shared result exactly once.

Grouping/FFN/combine outputs stage before publication. The cursor orders
attention, routing, all expert groups, shared and combine for each layer before
chunk commit; wrong/failed/stale receipts refuse. It does not authenticate
model/history identity or synchronize callers. Intermediate hidden/contribution/
cache state stays private until the integrating engine publishes the chunk.
Native effects observe spans and read files; all arithmetic/decisions are Coax.

Chunk size is the largest count fitting an explicit measured-cost estimate and
workspace inventory. Grouped mode rejects a maximum of one and refuses a
singleton budget while multiple tokens remain. Only a final tail or explicitly
requested serial mode uses one. Cost estimates are not hard deadlines; actual
elapsed time, calibration/context drift and other live memory need accounting.

Caller duties include valid immutable typed allocations/extents and source/model/
recipe identities, successful unmodified plans/borrowed keys, truthful receipts,
correct live-memory/calibration inputs, linear cursor ownership and exclusive
disjoint writes. Whole-model request fencing, cancellation and publication remain
engine work. Owned causal-stack and original layer-0 MoE batch qualify CPU
components, not complete original-model prefill or learned attention.

Generated CUDA products/flags/publication execute on the RTX 4080 with exact
CPU parity: FP4/FP8 ragged tiles, numerical refusal and all three original routed
projection shapes. This qualifies the product primitive, not a complete GPU
expert FFN, model prefill or device performance.
`psionic/tests/run_deepseek_prefill_cuda.sh` is the strict device gate;
`--compile-only` does not imply execution.

Check: `psionic/tests/check_deepseek_prefill.sh` covers independent grouping,
measured-cost chunks, exact serial/layer-major causal state, bounded file reuse,
native debug/release plain/ASan/UBSan and original MoE evidence replay.
`psionic/check.sh --only deepseek/prefill --jobs 8` adds zero-survivor mutation.
Evidence and scope: [chapter 65 record](docs/verification/psionic-ch65-2026-10-29.md).

## Psionic exact hot-expert residency changes only between steps

`psionic/lib/deepseek/hot*.coax` owns `deepseek-hot-exact-cpu-v1`:
profile counts seed fixed-point heat while slots remain empty until copied.
A finished step adds 1,024 units per unique selected key; every 256th step
then floors 9/10 of all heat. Replacement requires `4*hot > 5*cold + 4096`.
Incoming ties choose lowest expert ID; victim ties choose lowest slot.
Each slot changes once per plan; evicted IDs wait until the next plan.
Frozen caches fill holes but retain populated slots. Integer placement heat
is its own deterministic policy, not the old engine's floating heat profile.

Active-step dispatch leases a stable map. Gate-selected next-layer misses can
prefetch into private storage without changing residency. Only idle state
accepts installation, and model/step/revision tickets, old occupants, complete
copy receipts and all proposed bindings validate before any public byte or
slot changes. Late failures preserve all public data/metadata; private work
may change. Finish stages the complete next heat and returns the next header.
This is a single-owner value continuation, not synchronization between callers.

Packets concatenate all six original routed FP4 code/scale spans losslessly.
The native adapter observes addresses and performs mapped/direct I/O; Coax owns
range, placement, arithmetic and publication. Every file read stages before
packet publication. The CPU expert adapter binds unchanged packets and calls
chapter 63's existing arithmetic. No route, route weight or combine order is
altered. Resident coefficient budget excludes private incoming/I/O work and
other model/OS/device allocations; the caller budgets those independently.

Callers own complete source/model/packet authentication, matching immutable
file and packet lifetimes, correct dimensions/identity, successful unmodified
plans and truthful completed-copy receipts, valid typed allocations/extents,
recorded uses, linear header ownership and exclusive disjoint writes.
Device arenas, GPU arithmetic, event fencing, overlap, complete model execution
and measured cache acceleration remain unqualified integration work.

Check: `psionic/tests/check_deepseek_hot.sh` covers independent phase-shift
traces, all heat/maps/plans, file/host/resident FFNs, original expert evidence,
43x384/116 metadata topology and native debug/release plain/ASan/UBSan.
`psionic/check.sh --only deepseek/hot --jobs 8` adds zero-survivor mutation.
Scope and source identities: [chapter 64 record](docs/verification/psionic-ch64-2026-10-29.md).

## Psionic Engram lookup binds the normalized map and reads only planned rows

`psionic/lib/deepseek/engram*.coax` owns `deepseek-engram-lookup-v1`:
original 129,280-ID tokenizer bytes derive 99,092 normalized classes, with the
complete little-endian map authenticated before use. Compressed vocabulary
size determines fixed per-layer multipliers. The original 48 distinct primes,
cumulative ranges and odd multipliers select 24 rows each at layers 1 and 14.
Three compressed history IDs, including dead markers, cross chunk boundaries.
A dead/missing lookback pads itself and every older lookback with ID 2.

Each selected row has 256 E4M3 codes and eight per-row/per-32 UE8M0 scales.
Independent 64-bit tensor extents and whole-file length are checked before
Coax publishes read ranges. C observes actual addresses/file length and performs
lazy read-only mapping or pread; it does not decide hashes, ranges or arithmetic.
Decoded BF16 words and hash IDs publish only after complete success. Late bad
rows or I/O failure preserve all public output and spare tails; private work
may change. The adapter allocates bounded range metadata, never whole tables.

Callers own valid immutable allocations/extents, successful unmodified map
metadata, authenticated immutable files and matching table/model/history
identity, FD lifetime and exclusive disjoint writes. File-size observation is
not authentication; changing/truncating a mapped file violates its lifetime.
Owned sparse files qualify bounded I/O at original shard sizes and offsets.
They do not qualify original embedding payloads, learned lookup projections,
gated residual injection, GPU behavior or complete model execution.

Check: `psionic/tests/check_deepseek_engram.sh` covers independent original
normalization/map, full-prefix hashes/history, range layouts, mmap/pread,
BF16 rows and native debug/release plain/ASan/UBSan failures. The unit
`psionic/check.sh --only deepseek/engram --jobs 8` adds zero-survivor authority
mutation. Actual evidence: [chapter 62 record](docs/verification/psionic-ch62-2026-10-29.md).

## Psionic compressed attention keeps source ownership and causal prefixes explicit

`psionic/lib/deepseek/sparse-*.coax` owns `deepseek-sparse-cpu-wide-v1` at
projected-input boundaries: BF16 query/window vectors, F32 compressor KV/gate
projections and BF16 norm/index weights. Ratio 2 pools per channel and carries
one unfinished F32 pair; ratio 1 is ungated. Completed rows normalize before
rotary placement. Index keys derive from the pre-RoPE latent; compressed KV
and index keys use their distinct chapter 66 grids. V4 keeps adjacent complex
pairs, with pure-window and compressed YaRN profiles and inverse output RoPE.

The pinned 43-layer map distinguishes four KV/index-key owners, eight indexer
sources and candidate source 20. KV receipts may cover a later prefix, but
selected-position/candidate receipts must name this query. Declared receipts
do not authenticate model identity. Visible lengths exclude unfinished groups,
future rows and unfilled ring slots. Unselected compressed values are not read.
Candidate selection pins the newest partial block. CPU score ties use lowest
positions, with selected IDs returned in position order.

The CPU attention profile uses compensated wide reductions and an ideal stable
softmax with sink denominator mass. It does not reproduce the GPU kernel's
FP32/online/BF16 probability-tile rounding. BF16 index operation boundaries,
cache grids and final BF16 publication are explicit. Learned projection
binding, original-device distributions and full-model execution remain in 68.

Every public primitive stages its complete result before publication. Chunk
compression also stages all latents and both canonical tails together; a late
failure preserves them and spare tails. Private work may change. Callers own
valid immutable allocations/extents, matching model/history/projection state,
correct unmodified successful metadata and exclusive disjoint writes. Row
stores are individually atomic; larger cache/model commits need private shadows.
C observes actual span addresses only. The library allocates nothing.

Check: `psionic/tests/check_deepseek_sparse.sh` covers independent prefix
equations, all 43 pinned layer positions, packed caches/candidates/selected
positions, real whole/split compressor chunks, original 32x128 index and
64x512/128-window shapes, native debug/release plain/ASan/UBSan and late
failures. `psionic/check.sh --only deepseek/sparse --jobs 8` adds zero-survivor
authority mutation. Actual scope: [chapter 60 record](docs/verification/psionic-ch60-2026-10-29.md).

## Psionic packed V4.1 caches reconstruct the reference's BF16 grids

`psionic/lib/deepseek/cache*.coax` owns `deepseek-cache-bf16-grid-v1`:
window KV is FP8 E4M3 with per-32 UE8M0 scales; compressed KV is low-first
FP4 E2M1 with per-16 positive E4M3 scales; index keys/queries are low-first
FP4 with per-32 UE8M0 scales. Whole post-RoPE vectors, tails included, round
at the pinned source's quantization sites. F32 scale/division/product and
nearest-even BF16 publication boundaries are explicit. Inputs are BF16 bits.
NaN, invalid scales and overflow refuse rather than changing finite values.

Pack stages both code/scale spans and unpack stages all BF16 words. Public
outputs and spare tails preserve their previous values on any failure; private
work may change. Caller duties include valid immutable allocations/extents,
matching format/model/history identity and exclusive disjoint writes. Native
C observes addresses only. The codec allocates nothing and does not own the
attention ring, rotary operation, compressors or sharing state.

Persistent payload and chunk work have separate enforced limits. A chunk may
exceed the persistent window row count. The pinned Flash inventory counts four
KV/index-key producers, eight query-producing indexers, forty backbone windows
and up to three draft windows. Its reusable codec work includes the larger
32-head index-query row. It excludes other model/device memory and alignment.

Check: `psionic/tests/check_deepseek_cache.sh` covers all BF16 input patterns in
three group forms, every encoded scalar/scale pair, BF16 midpoint neighbors,
native debug/release plain/ASan/UBSan, row-chunk parity, atomic failures and an
independent pinned ownership/work inventory. `psionic/check.sh --only
deepseek/cache --jobs 8` adds zero-survivor authority mutation. This qualifies
CPU source-equation bits, not an original GPU capture or complete attention;
see the [chapter 66 record](docs/verification/psionic-ch66-2026-10-29.md).

## Psionic V4.1 expert formats retain separate scales and pre-down route weights

`psionic/lib/deepseek/expert*.coax` owns `deepseek-expert-cpu-wide-v1`:
original low-first FP4 E2M1 routed bytes with UE8M0 scales per row/32 K,
and shared FP8 E4M3 bytes with UE8M0 scales per 32x32 block. All finite
code/scale pairs decode exactly to binary64, including signed zero and values
above F32 range. Scale 255 and FP8 codes 127/255 refuse. Activations round
to the original per-32 FP8 grid with nearest-even ties and exponent-rounded
UE8M0 scales. Stored projections/activations are F32; products/reductions use
compensated binary64 with checked output narrowing.

Up clamps both sides and gate only from above. The F32 SiLU/up result is
multiplied by the F32 route weight before down-input quantization/projection.
Mixture evaluates selected FP4 experts and adds one shared FP8 expert once.
Every expert's metadata is checked; unselected coefficient values are not
evaluated. Complete output and spare tail preserve their previous values on
any failure, including a late down/shared failure; private work may change.

Caller duties include trustworthy tensor/route identity, consistent shapes,
immutable borrowed bytes, valid typed extents/allocations and exclusive
disjoint writable storage. C observes native addresses and performs effects.
The library owns all format, validation, numerical and publication decisions.

Check: `psionic/tests/check_deepseek_expert.sh` assembles exact bit/grid and
independent FFN fixtures, native debug/release plain/ASan/UBSan, original
expert/6-of-384 MoE evidence replay and book output. The original configuration
selects six of 384; the component admits up to eight unique routed IDs.
`psionic/check.sh --only deepseek/expert --jobs 8` adds zero-survivor mutation.
Actual checks and original component scope: [chapter 63 record](docs/verification/psionic-ch63-2026-10-29.md).

## Psionic hyper-connections carry the previous pre-mix with all four streams

`psionic/lib/deepseek/hyper*.coax` owns `deepseek-hyper-cpu-wide-v1`:
four F32 residual streams, F32 coefficient projections and stored intermediates,
compensated binary64 products/reductions, RMS epsilon 1e-20 and coefficient
epsilon 1e-6. Combination uses source/destination axes, row softmax plus
epsilon, then one column normalization and 19 row/column pairs. Every
normalization denominator includes epsilon; finite iteration sums need only
match the declared equations, not perfect stochasticity.

Prepare computes current coefficients but collapses with the previous
sub-block's pre-mix. Finish expands the sublayer output and returns the current
pre-mix for the next sub-block. Initial pre is one-hot; final readout collapses
using the last returned carry. Boundary state contains both all four streams
and that carry. Outputs publish only after complete success, including late
overflow; private scratch may change. A failed finish returns no valid next
carry. Valid immutable allocations/extents, trustworthy parameter identity,
matching source-major stream layout, unmodified successful mixes, the
initial/returned carry and exclusive disjoint outputs remain caller duties.
C observes borrowed-span addresses only.

Check: `psionic/tests/check_deepseek_hyper.sh` assembles independent Decimal/
scalar fixtures, debug/release, native plain/ASan/UBSan, all seven handoffs
in a six-sub-block chain, final collapse and original-width projections.
`psionic/check.sh --only deepseek/hyper --jobs 8` adds zero-survivor mutation.
Actual checks/source identities and the component scope are in the
[chapter 61 record](docs/verification/psionic-ch61-2026-10-29.md).

## Psionic complete V2-Lite execution commits every layer together

`psionic/lib/deepseek/v2*.coax` owns `deepseek-v2-lite-cpu-wide-v1`:
original BF16 coefficients, F32 activations/caches and compensated binary64
products/reductions, with original V2 YaRN and full-softmax routing. Routed
weights apply after each down projection; the shared FFN is added once.
Compressed and expanded caches have distinct F32 boundaries and references.

Complete SHA-256 admission checks all original files and exact coverage of
the 5,291 tensor intervals before binding. Public model metadata borrows
public block/expert storage and immutable files, never temporary drafts.
Caller-supplied profiles require trustworthy pins. Every token stages all
layer cache rows, residual diagnostics, routes and logits before publication;
failure preserves all public spans. Private work may change. Valid typed
allocations/extents, immutable lifetimes, matching model/cache form,
sequential history and exclusive disjoint writable storage are caller duties.
C observes nested address overlap and performs effects only.

Greedy ties use lowest IDs; EOS, token/context limits and cancellation remain
distinct. Text support is the fixed original Hi prompt and an admitted token
byte table, bounded to 4,096 IDs/65,536 answer bytes. BOS and unmapped IDs
refuse; only a final typed EOS is removed. This is not a general V2 encoder.

Check: `psionic/tests/check_deepseek_v2.sh` covers complete owned prefix
references, admission/ownership, native debug/release plain/ASan/UBSan,
current-source original-answer decoding and lossless original-value replay.
`psionic/check.sh --only deepseek/v2 --jobs 8` adds zero-survivor mutation.
Original execution and arithmetic scope: [chapter 58 record](docs/verification/psionic-ch58-2026-10-29.md).

## Psionic latent attention stages new rows and keeps rotary profiles distinct

`psionic/lib/deepseek/mla*.coax` implements shared latent-attention CPU
equations with F32 activations/caches, BF16 up-projection coefficients and
compensated binary64 products/reductions. Compressed rows store normalized
latent plus one shared rotary key; expanded rows store per-head non-rotary
keys/values plus one shared rotary key. Queries/keys arrive already rotated.
The cache form, shape, model identity and sequential prefix must match.

Attention reads exactly the supplied immutable history prefix and returns a
separate pending row. Public output and pending row publish together only
after all heads/value projections succeed; history is never modified. Caller
work/scores may change on failure. This permits a full decoder to stage every
layer before committing a token. It is not itself a complete model decoder.

`v2-rope.coax` separately owns the original V2-Lite adjacent-pair permutation,
fixed YaRN frequencies (factor 40, original 4,096 positions, beta 32/1), and
the two 0.707 mscale factors in attention scaling. It does not reuse R1's
rotary layout. Rotary query/key outputs also publish together. Valid typed
allocations/extents, immutable inputs and exclusive disjoint writable spans
remain caller duties; C observes addresses only.

Check: `psionic/tests/check_deepseek_v2_math.sh --mutate --jobs 8` covers
independent full-prefix compressed/expanded fixtures and original-parameter
YaRN vectors through position 163,839, native debug/release plain/ASan/UBSan,
atomic failures and zero-survivor mutation. Scope and outstanding complete
V2 execution gates: [component record](docs/verification/psionic-deepseek-v2-math-2026-10-29.md).

## Psionic DeepSeek routers separate selection bias from combination weights

`psionic/lib/deepseek/router*.coax` owns `deepseek-router-cpu-wide-v1`:
F32 logits/biases with binary64 scoring and reduction, bounded to 384 experts
and eight selected experts/groups. V2 retains full-expert softmax mass; V3
uses sigmoid, selection-only bias and top-two group sums; V4.1 uses global
square-root-softplus, selection-only bias, its k>1 epsilon normalization and
route scale. Lowest-ID expert/group ties are explicit. This is a CPU equation
reference; original model/device arithmetic and upstream top-k tie order
require separate qualification. The correction vector is supplied, not trained.

Scores and biased scores publish together only after complete validation and
successful normalization. Failures preserve both public arrays; spare tails
remain untouched. Each result records expert and group cutoff margins and
excluded IDs, with explicit flags when no excluded candidate exists. Comparison
records retain both complete reports and separate ordered-ID agreement, set
replacement and expert-matched weight L1. Comparisons require unmodified
successful results for the same request/model/configuration and authenticate
no identity. The caller owns valid typed allocations/extents, immutable inputs
and exclusive disjoint outputs; native C observes addresses only.

Check: `psionic/tests/check_deepseek_router.sh --mutate --jobs 8` covers an
independent Decimal/sort oracle at all original expert dimensions, tiny
probabilities and near ties, exact boundary checks, native debug/release
plain/ASan/UBSan and exact book output. See the
[chapter 59 record](docs/verification/psionic-ch59-2026-10-29.md).

## Native relay TLS verifies the peer before exposing plaintext

`coax/platform/tls-native.c` is the owner-approved TLS effect: OpenSSL 3 on
Linux, Network.framework and Security on Apple. `cxtls_connect` owns TCP and
TLS under one deadline, with TLS 1.2 minimum, SNI, chain and DNS-name checks.
There is no verification bypass or plaintext fallback. Test anchors require
a separate build macro; production ignores that environment variable.
The scalar bridge has one caller thread per process. Apple callback state is
serialized and generation-fenced, including pending reads after timeout and
callbacks delivered after cancellation. Buffers and connection slots are bounded.

Apple reports `CXTLS_END_UNVERIFIED` rather than claiming that opaque receive
completion proves TLS close_notify. `relay_conn_rules.coax` refuses that end
during ordinary polling; only a parsed authenticated WebSocket close permits
it during the closing handshake. `last_read` preserves the native evidence.
Coax owns URL/transport choice, framing, entropy policy and relay behavior;
the host owns encryption, trust evaluation, clocks and platform effects.

Checks: `coax/tests/net/tls.sh` and `relay_e2e.py` cover certificates, version
floor, exact stream bytes, timeouts/reuse and actual relay sessions under
ASan/UBSan. Apple pending-callback reuse also passes ThreadSanitizer. iOS
compilation is separate from runtime qualification. Producer hashes, actual
results, Linux regression and the owner-requested focused-check scope are in
the [Apple TLS record](docs/verification/2026-10-30-apple-tls.md).

## Psionic resident decisions have bounded lifecycle and private measurements

`psionic/lib/serve-run.coax` owns Part III's execution-payload admission,
observed-time statistics, private metrics, readiness and bounded restart
policy. C performs file/socket/clock/allocation/process/device effects.
One authenticated session and reserved arenas serve one active request.
Device execution allocates no redundant CPU model-state/work arenas; its
device/pinned/mapped payloads remain separate from the host execution budget.

Verified-file evidence is attached through `fstat` of the same descriptor
that was mapped. Path replacement cannot bind another inode to those bytes.
Local cached records are trusted prior evidence, not signatures. Checkpoint
files must stay immutable for the complete borrowed session lifetime.
`mincore` observes file-page warmth independently of verification-cache reuse.

CPU and device snapshots belong to one request. A hit needs the authenticated
identity and every prefix token to match; the retained prefix is separate
from mutable suffix state. Request boundaries clear metadata. One-question
device requests keep the joint path. No cross-request/tenant cache is implied.
Device safe points refuse busy connections; the bounded socket cleanup pool
never queues model work. Cancellation publishes no answer and the next request
starts fresh. Driver and caller lifetime obligations remain unchanged.

Readiness follows authentication, disjoint buffers and three retained golden
decisions under the pre-existing tolerance. Default restart policy allows five
restarts with 250 ms exponential backoff capped at ten seconds; owner stops
and clean exits do not restart. The native systemd readiness effect follows
the same gate. The first deployment is loopback-only and not boot-enabled.
Private metrics contain identities, outcomes, counts and clocks, never state,
question IDs, option/answer text or probabilities. They authorize no payment;
HTTP usage remains null. Peak process RSS is not aggregate host/GPU usage.

Check: `psionic/check.sh --only live-decision` runs focused Coax checks,
native debug/release plain/ASan/UBSan, transport/lifecycle and descriptor-race
checks and exact book outputs. Actual original Metal operations and managed
CUDA crash/re-warm/restart, paired measurements, producer identities and the
owner-requested omission of full mutation and original CPU inference are in
the [Part III record](docs/verification/psionic-partIII-2026-10-29.md).

## Psionic Responses serving preserves typed state and exclusive GPU access

`psionic/lib/responses/` owns `psionic-gptoss-responses-v1`: bounded HTTP/JSON
admission, bearer authentication, flat tool schemas, stateless Harmony mapping,
response lifecycle and semantic SSE. Original 20b generation uses the existing
resident CUDA profile, one exclusive request, 2048 positions and greedy output
limits. C owns sockets/clocks/threads/device effects; no tool executes here.
The fixture generator is absent from the production import root.

Raw reasoning is transported as a typed reasoning item, separately from final
text. Current tool-turn analysis survives replay; completed prior analysis is
removed. Provisional typed API text is streamable, while the CLI's complete-final
display gate remains unchanged. Argument deltas and incomplete calls grant
nothing. Every well-formed addressed record is a function-call item, whichever
channel carried it (`functions.NAME` and bare `NAME` name `NAME`; a dotted
built-in such as `browser.search` keeps its name and replays verbatim). A call
naming an admitted tool must satisfy its schema; any other name is surfaced
for the client to refuse and is never granted or run here. With function tools
the system message names the commentary channel, as the Harmony reference does.
Unsupported capabilities refuse before model effects; malformed model output
fails rather than fabricating success. This profile is not universal
Open Responses compliance or constrained GPU decoding.

HTTP bytes, item/node/UTF-8 work and event buffers are bounded. Serialization
state commits after a complete batch fits; failed sends cancel rather than
retry partial events. Disconnect/deadline decisions run between tokens and
committed prefill chunks. Earlier chunks remain committed; the next request
resets the cache. A device failure makes the worker unavailable. Immutable
admitted artifact/table lifetime, truthful native extents and exclusive spans
remain caller duties. The listener is loopback-only and logs no request content.

Check: `psionic/tests/check_responses.sh` covers owned HTTP, typed replay, SSE,
debug/release and native plain/ASan/UBSan. The existing device harness additionally
checks prefill cancellation preserves committed state. SDK/schema and actual
original GPU evidence, numerical scope and operational limits are in the
[Responses record](docs/verification/psionic-responses-2026-10-29.md).

## Psionic DeepSeek front-door boundaries survive tokenization and streaming

`psionic/lib/deepseek/front*.coax` owns `deepseek-front-v1`: a bounded
V4.1-style text recipe and DSML stream protocol, plus an explicitly separate
R1 tokenizer adapter. Ordinary message/tool strings never become control IDs.
Adjacent plain fragments join before BPE; only typed specials split runs.
R1 has no system/DSML control mapping and refuses those boundaries. Its
existing chapter 56 history/decoder contract is unchanged.

`front-v41*.coax` adds `deepseek-front-v41-text-v1`: the pinned original
129,280-entry V4.1 vocabulary and ranked byte-BPE merges, three ordered isolated
splits, Unicode 16 properties and no normalization. Only eight explicitly typed
controls map to original IDs; every other added ID refuses. Ordinary marker
spellings remain byte-BPE text. An ordinary run may span borrowed fragments,
including incomplete UTF-8 fragments, but the rejoined run must be valid UTF-8.
Each real control starts a new independently encoded run. Complete tokens/units
publish only after success. Caller-owned work may change on failure. Exported
low-level span probes require a validated UTF-8 slice, an in-range scalar start
and successful unmodified layout; checked text entry points establish these.

Callers must admit immutable derived tokenizer bytes against the committed
source/packed identities before use; the native gate uses Coax `pw_digest`.
Typed allocations/extents, disjoint writes and borrowed text lifetimes remain
caller duties. The original tokenizer and bounded recipe/actual-token stream
path qualify on CPU. Complete upstream recipe options, namespaces, tool schema
rendering/call-ID sorting, media and V4.1 model execution remain separate work.

Streaming consumes typed bytes/controls once, retaining only a fixed delimiter
window and incomplete UTF-8 scalar. It emits reasoning, content, tool names
and escaped string/raw argument fragments separately. Malformed framing and
UTF-8 refuse; raw JSON validity, schema checks and tool grants belong to a
consumer. A closed frame grants nothing. EOS requires a separate normal finish
event; transport EOF and truncated syntax remain incomplete. Failed feeds
preserve public events and return the supplied state; successful prior chunks
remain committed. Caller-owned staging may change. The caller supplies valid
types/extents, admitted immutable tokenizer data, live borrowed strings,
exclusive disjoint writable storage and only initial/returned parser states.

Check: `psionic/tests/check_deepseek_front_v41.sh` covers original V4.1 text,
whole-vocabulary and Unicode digests, bounded recipe IDs, actual-token stream
splits and native debug/release plain/ASan/UBSan. Its focused mutation unit is
`psionic/check.sh --only deepseek/front-v41 --jobs 8`. Evidence:
[CPU V4.1 tokenizer record](docs/verification/psionic-ch57-v41-2026-10-30.md).

Check: `psionic/tests/check_deepseek_front.sh` covers owned protocol oracles,
all 32,896 split pairs, all truncations, original BPE vectors, native
debug/release plain/ASan/UBSan and book output. `psionic/check.sh --only
deepseek/front --jobs 8` also runs zero-survivor authority mutation. Scope
and evidence: [chapter 57 record](docs/verification/psionic-ch57-2026-10-29.md).

## Psionic DeepSeek R1 distill owns a separate dense CPU profile

`psionic/lib/deepseek/` implements `deepseek-r1-qwen2-cpu-wide-v1` for the
pinned original R1-Distill-Qwen-1.5B BF16 artifact. Complete original-byte
SHA-256 admission binds the reviewed 339-tensor header and intervals;
the shared typed binder additionally checks complete payload coverage,
unique names, disjoint ranges and every expected shape before publishing.
Caller-supplied profiles require trustworthy pins. Borrowed immutable files,
model/layer storage and their lifetime remain the caller’s responsibility.

The decoder has learned RMS scales with epsilon 1e-6, biased Q/K/V,
full split-half RoPE at theta 10,000, causal GQA, ordinary SiLU gate/up
products and an independent vocabulary head. Products/reductions use the
wide CPU profile with F32 stored activations. Every token stages all new
KV rows, residual diagnostics and logits; failures preserve every public
cache/output span. Private work may change. The caller owns sequential
history, valid allocations, truthful extents, immutability and exclusive
access; native C observes nested address overlap and performs effects.

The separate original byte-BPE table reuses Unicode-17 NFC DATA and ranked
merging, with the original letter-only split profile. Typed text history
admits an optional first system message, alternating user/assistant turns
and a final user, then inserts tokenizer BOS 151646 once, rather than the
config’s conflicting 151643. Prior assistant reasoning is stripped after
its last closing marker. Ordinary strings cannot insert control IDs.
Public text/token outputs publish atomically. Decoding refuses model IDs
with no tokenizer entry. Reasoning and final intervals remain separate;
EOS, token/context limits and between-step cancellation remain distinct.
There is no tool execution, sampling, GPU, service or V4.1 claim.

Check: `psionic/tests/check_deepseek.sh` covers owned two-/three-layer dense
oracles, original tokenizer/recipe vectors, native debug/release plain and
ASan/UBSan checks, exact replay and book output. `psionic/check.sh --only
deepseek --jobs 8` also runs the authority mutation roots. The retained
original full-model comparison and current-source original slices, their
actual build identities and limitations are in the
[chapter 56 record](docs/verification/psionic-ch56-2026-10-29.md).

## Psionic original gpt-oss GPU sessions retain model state on device

`psionic/lib/gptoss/device-*.coax` owns `gpt-oss-cuda-f32-v1`: original
BF16/MXFP4 coefficient bytes relocate unchanged into aligned resident intervals,
f32 lane products and ordered f64 reductions feed the complete CUDA decoder.
Single-token and batched packed products use admitted tensor-relative u32
offsets with unchanged lane/fold arithmetic and view bounds.
Decode attention reuses the admitted 64-query/8-KV layouts. Decode softmax
keeps its ordered per-head f64 denominator in no-longer-live norm scratch, then
performs each original f64 division independently in the next ordered kernel.
There is no reciprocal substitution or change to sink mass/reduction order.
Coax admits shapes, byte budgets, tensor-relative u32 index bounds, all kernel
views and the ordered launch program; C performs effects. The native session
has no automatic CPU fallback. Authentication and upload happen once.
Synchronous decode may replay an ordered CUDA graph of that admitted program.
Coax compares semantic descriptors and lists dynamic node updates or a topology
rebuild. Each node depends on its predecessor; immutable bindings retain their
session lifetime. CPU emulation, asynchronous submission and per-kernel
instrumentation use ordinary launches. A control download waits for the step
before the next graph update; closing destroys graphs before buffers/modules.

Single-token steps publish cache/logits/diagnostics only after all numeric
flags succeed, then advance position. Batched prefill uses causal per-query
history, stable expert token groups and a private full cache shadow. Each
chunk publishes together, including wrapped window entries; earlier successful
chunks remain committed if a later chunk fails. Caller-supplied sequential
history, immutable admitted files, valid allocations and exclusive session
access remain required. Driver failures poison the session. Greedy device
generation uses Harmony framing and the shared final-only display gate;
sampled device generation, speculation and tools retain separate gates.

Check: `psionic/tests/check_gptoss_device.sh` uses focused owned fixtures,
debug/release, native plain/ASan/UBSan and exact book output. CUDA/original
runs are explicit opt-ins; original answers compare with retained independent
CPU vectors, never routine full-original CPU inference. Full mutation roots
remain excluded by the owner's request. Scope, hardware and measurements:
[device record](docs/verification/psionic-gptoss-device-2026-10-29.md).

The same programs and kernels run on an Apple GPU (chapter 45h) under
`gpt-oss-metal-f32-ff-v1`: `coax metal-compute` emits `device-kernels.coax`
unchanged, and `psionic/platform/gptoss-metal.c` implements the same `pg_*`
interface through `psionic/platform/metal-host.m`. It decides nothing. Every
original weight is resident in shared memory, and nothing streams. There is
no CPU fallback: without a device the result is `PC_ABSENT`. A failed command
buffer or a failed in-kernel Coax check poisons the session. The f64
reductions are APPLE-FF-v1 float-float pairs, and the YaRN table is built on
the host in binary64. CUDA graph replay has no Metal counterpart; each program
is one ordered command buffer. Check: `psionic/tests/run_gptoss_metal.sh` runs
the tiny model by default. With the original files it applies chapter 45b's
frozen 0.0005 rule and exact routes and top token. `PSIONIC_CPU_GREEDY=1`
requires the GPU's greedy token IDs to equal chapter 45's CPU engine's, and
`PSIONIC_DEVICE_QUALIFY=1` repeats layers 0 and 1 and the readout on the CPU,
launch by launch. [Metal record](docs/verification/psionic-gptoss-metal-2026-10-29.md).

## Psionic original gpt-oss execution and constrained continuations

`psionic/lib/gptoss/` owns `gpt-oss-cpu-wide-v1`: fixed independently pinned
20b/120b original BF16/MXFP4 profiles, complete SHA-256 admission and exact
parameter intervals, compensated f64 products with f32 storage, all-layer
staging and full-vocabulary readout. A model step commits cache and public
diagnostics only after complete success; work may change. Even-layer cache
storage stays window-bounded, while odd layers reserve the chosen full prefix.
The caller supplies model identity, immutable live inputs, sequential history,
truthful extents, valid allocations and exclusive disjoint writable storage.
C observes addresses and performs effects, never model or grant decisions.

Counter-based SplitMix64 sampling has explicit seed/counter progression,
lowest-ID ties, temperature/top-k/top-p and pre-choice masks. Return, call,
limit and between-step cancellation are distinct. Model and emission are
separate transactions. Speculation uses standard acceptance/positive residuals,
an all-match bonus and complete snapshots including overwritten ring entries.
Its serial CPU measurements do not establish acceleration or a batched backend.

`gpt-oss-json-enum-v1` is a finite root-object enum schema profile, not a
universal schema compiler. Vocabulary masks enforce prefix membership before
sampling; only a complete argument may offer call. A named commentary handoff
still needs the host's grant. Tool results are typed messages. Only recipient-free
assistant final turn bodies pass the display gate; analysis never does.
Shared Harmony record/identifier types are independent of tokenizer execution.

Check: `psionic/tests/check_gptoss.sh` runs focused changed-code checks, native
plain/ASan/UBSan and exact book transcripts. Full mutation reruns were stopped
at the owner's request on 2026-10-29; mutation slicing is separate work. Opt-in
original runs and their exact build/scope limits are in the chapter
[45](docs/verification/psionic-ch45-2026-10-29.md),
[46](docs/verification/psionic-ch46-2026-10-29.md),
[47](docs/verification/psionic-ch47-2026-10-29.md) and
[48](docs/verification/psionic-ch48-2026-10-29.md) records.

## Psionic Harmony text separates ordinary bytes from control IDs

`psionic/lib/harmony/` owns `gpt-oss-harmony-text-v1`: the pinned original
o200k byte ranks, a separate case-sensitive split pattern and Unicode-17
properties, without NFC normalization. The original-derived HST7 table is
SHA-256-admitted once and retained immutable; layout parsing is not identity
admission. The existing byte-BPE loop is shared through an independent pair
index and whole-piece lookup. Ordinary text cannot introduce formatting IDs.

Typed messages preserve supplied history. The three reasoning efforts use the
pinned default system template. Complete parsing recognizes all supported
roles/channels and recipients on either side of a channel; completion mode
continues past message ends and consumes the first return/call, leaving trailing
tokens untouched. Only assistant messages may return or call, with a named
recipient required for calls. Unsupported extensions and malformed input refuse.
Public token arrays, decoded bytes and byte-interval records publish atomically;
workspaces may change. No tool execution or display policy is granted.

Bounds are 64 messages, 16,384 IDs, 65,536 logical decoded bytes, 64-byte ASCII
identifiers, 256-byte headers, 1,024-byte pieces and a 16,777,216 sum-of-squares
piece budget. The model profile ends at ID 201,087; generic Harmony
ID 201,088 is outside it. Valid allocation/extents/alignment, input/table
immutability, lifetime and exclusive writable access remain caller duties.
C observes nested overlaps only. Incremental parsing, optional header and
multimodal extensions and whole-model execution remain separate gates.

Check: `psionic/tests/check_harmony.sh` and the main `psionic/check.sh` cover
owned original-rank/regex/heap oracles, debug/release, zero-survivor authority
mutation, native plain/ASan/UBSan and exact book output. See the
[chapter 44 record](docs/verification/psionic-ch44-2026-10-29.md).

## Psionic gpt-oss attention preserves sink mass and visible cache history

`gptoss-attention.coax` and `gptoss-math.coax` implement
`gpt-oss-attention-cpu-wide-v1`: 64-wide heads, 1–64 query/1–8 KV heads,
learned per-query-head sink logits in the denominator with no value, split-half
YaRN and a ring bounded by its sliding window. Sliding capacity equals the
window, at most 128, with absolute positions 0–131,071; full causal capacity
is separately bounded to 128. Only visible old cache entries are read.
Shape, position, exact input spans, writable capacities, address observation
and finite data are checked before publication. Rotation failure preserves
public cache and output; work may change. Success leaves unused public tails
unchanged. Caller duties include valid aligned allocations, truthful extents,
immutable inputs, exclusive access and matching shape/model/sequential history.

Math helpers stage split-half rotation and bounded biased projection; the
interleaved expert activation validates all pairs before writing and uses an
upper-only gate clamp, two-sided linear clamp, alpha 1.702 and a plus-one
linear term. Clamp diagnostics count changed lanes. Wide CPU arithmetic with
f32 storage is separate from original Torch/BF16 parity. Original checkpoint
binding, full-size projections, block composition and generation remain
separate qualification gates.

Check: `psionic/tests/check_gptoss_attention.sh` and the main
`psionic/check.sh` include independent scalar references, debug/release and
zero-survivor mutation plus native plain/ASan/UBSan gates. See the
[chapter 36 record](docs/verification/psionic-ch36-2026-10-29.md).

## Psionic bounded top-k mixtures preserve route and output semantics

`psionic/lib/moe.coax` implements `cpu-topk-affine-v1`: 1–128 finite logits,
1–4 selected experts, descending scores and lowest-ID ties, selected-only
softmax with wide accumulation and f32 coefficients. Tiny affine mixtures
have width 1–16, unique resident IDs and exact row-major matrix/bias extents.
Every resident entry counts against a logical payload budget; only selected
parameter values are numerically evaluated. Missing selected experts refuse
instead of changing the route. Output and the successful route publish only
after every selected expert and the bounded combination succeed; spare output
capacity is unchanged. This is not original BF16 gpt-oss arithmetic, a complete
expert layer, model identity admission or OS memory reservation. The native
adapter observes nested overlap; valid allocations, extents, alignment,
parameter identity, immutable lifetimes and exclusive access are caller duties.

Check: `psionic/tests/check_moe.sh` and the main `psionic/check.sh` include
independent Decimal fixtures, debug/release checks, zero-survivor mutation
and native plain/sanitized gates. See the
[chapter 35 record](docs/verification/psionic-ch35-2026-10-29.md).

## Psionic original MXFP4 decoding admits complete finite windows

`psionic/lib/mxfp4.coax` decodes the gpt-oss original-format low/high nibble
pairs with one scale per 32 values. The `gpt-oss-mxfp4-f32-v1` profile admits
positive complete rows, at most 1,048,576 output values and scale bytes 0–252.
It validates all counts and scales and observed overlap before any output
write. Admitted values are exact finite binary32, preserving signed zero and
subnormals; spare output capacity is unchanged. This decoder does not
inspect or authenticate files. Callers supply the correct tensor pairing,
shape, provenance, valid disjoint storage, alignment, lifetime and exclusive
access; the C adapter only observes addresses. Original checkpoint coverage
and other numerical/back-end profiles require separate evidence.

Check: `psionic/tests/check_mxfp4.sh` and the main `psionic/check.sh` cover
Coax checks, zero-survivor mutation, independent scalar bit fixtures and
native debug/release plus sanitizer checks. See the
[chapter 34 record](docs/verification/psionic-ch34-2026-10-29.md).

## Psionic registered operators retain typed IR semantics and code leases

`psionic/lib/operator-registry.coax` admits bounded unary same-shape f32
schemas, disjoint writes, versioned names, optional input VJPs and pointwise
batching. Registered IR tag 11 requires an attached admitted registry; absent
hooks or unknown IDs refuse before execution. Coax checks shapes, colocated
operands, backward capability and code/schema identities. Native hooks bind
trusted Coax CPU callbacks and generated GPU entries inside the existing
command stream; no operator-specific formula lives in core C dispatch.

Registry fingerprints include name/entry lengths and bytes, versions,
properties, compiler and supplied source/module digests. They are part of
`TiTarget.operators`. Compiled plans freeze registration tables through Coax
leases; derived plans inherit independent registry/module leases, and owners
cannot close early. Operator authors retain their declared memory/numerical
contract obligations; callback provenance is trusted producer metadata, not
sandboxing or attestation. See the focused `operator-registry` gate and
[chapter 26c evidence](docs/verification/psionic-ch26c-2026-10-29.md) for actual
CPU/CUDA/Metal inference, finite-difference VJPs, compiled MLP, batching and
scalar training. Full mutation runs are omitted under owner instruction.

## Psionic symbolic templates share code and specialize guarded plans

`psionic/lib/tensor-template.coax` resolves up to four guarded integer symbols,
a declared two-way branch and bounded unary unrolling, admitting at most 64
variants including control bindings. Every variant is checked at open. Input
storage/strides/offsets are derived as packed bindings; explicit view nodes own
striding semantics. Cached keys include shape and control bindings inside a
copied template and fixed compiler/backend/module identity. Coax chooses four
LRU slots and aggregate payload-budget evictions, and installs only successful
native specializations. Evictions can survive an allocation failure; outputs
remain unpublished. CUDA/Metal specializations borrow one compiled module;
Coax leases refuse unloading its owner before children close. No new backend
compilation occurs on a shape miss. Synchronous execution and numerical/driver
failure rules remain 26a's. See the focused `tensor-template` runner and
[chapter 26b evidence](docs/verification/psionic-ch26b-2026-10-29.md). Full
mutation runs are omitted under the owner's explicit instruction.

## Psionic tensor IR plans preserve SSA roots and publish whole results

`psionic/lib/tensor-ir*.coax` owns `tensor-ir-f32-v1`: an explicitly authored,
32-node straight-line graph, rank-0–4 f32 values and declared placement. Unknown
operations, non-f32 dtypes, foreign placement, future operands, invalid views,
noncontiguous reshapes, gather indices, scalar work and buffer budgets refuse
before tensor execution. Every materialized node owns a fresh logical root;
views alias read-only root-relative storage. Last-use analysis follows aliases
and pins the chosen output until publication. A slot can be reused only after
its root's last use is strictly before the current command, never over an
operand of that command. The full lowered command stream and persistent
workspace are owned by an opaque native object; there is no hidden CPU fallback.

The CPU reference uses chapter 3a's compensated f64 operator procedures and
wide LayerNorm equations. Planned CPU/CUDA/Metal use the same sequential f32
Coax arithmetic and distinct per-command numerical flags. Reused workspace
never erases earlier failures. A final logical gather stages the output;
publication first checks every flag, preserving caller output on all refusals.
Native address observations reject overlap with inputs or owned metadata and
host buffers. Host access is exclusive and synchronous. Transfer failure paths
attempt to drain submitted work; numerical failures permit reuse, driver
failures poison further replays, and an idle poisoned object can be closed.
Coax owns those lifecycle decisions; C performs allocation and driver effects.

Cache matching compares active graph descriptors exactly, including shapes,
strides, bindings, dtype and placement, and target/compiler/backend/module
identities. The trusted caller supplies the qualified compiler digest; Coax
hashes actual module bytes and observed backend descriptions. Objects copy
their graph and never execute a different caller descriptor on a stale plan.
This is process-local identity, not execution attestation or a persistent cache.

Check: `psionic/check.sh --only tensor-ir` and explicit CUDA/Metal runners;
authored debug/release, independent Decimal(100) graph oracles, observed
allocation counts/bytes and native debug/release ASan/UBSan. GPU memory is not
instrumented by host sanitizers. Metal uses the qualified release emission;
the failed checked candidate and the owner's omission of full mutation runs
are recorded in [chapter 26a verification](docs/verification/psionic-ch26a-2026-10-29.md).
Automatic graph extraction, dynamic shapes, narrow dtype IR execution, direct
`PtdTensor` binding, fusion and differentiation remain separate work.

## Psionic device kernels and plans own the device arithmetic and schedule

`psionic/lib/device/project.coax`, `layer.coax` and `warp.coax` are `coax cuda` kernels:
each output element belongs to exactly one work item, so a result does not
depend on scheduling, and the same source compiled to C reproduces the device
bit for bit except where the device math library's `exp` or `log` differs
or, in `warp.coax`, the tensor cores round f32 partial sums differently from
the Coax meaning of `cu_mma_bf16_16x8x16`; those are compared within frozen
tolerances.
`plan.coax` decides the workspace layout, which original weights stay resident
and which stream through two slots, and every copy, wait, release and launch;
`psionic/platform/cuda-host.c` executes those effects and decides nothing.
Authentication, the session's borrowed weights and their lifetime remain the
CPU session's (chapter 10); the host must keep a slot's previous launches
complete before refilling it, which the plan's wait and release operations
express. Check: `psionic/tests/check_device.sh` covers debug/release checks,
zero-survivor mutation, CUDA emission and the device projection; the
[chapter 26](docs/verification/psionic-ch26-2026-10-29.md) and
[chapter 30](docs/verification/psionic-ch30-2026-10-29.md) records hold the
device evidence and limits.

The same kernels run on an Apple GPU through `coax metal-compute`
(chapter 31): `psionic/platform/elephant-metal.c` performs the plan's
launches through `psionic/platform/metal-host.m` and decides nothing. On
unified memory every weight is resident; a plan that would stream is refused,
never silently degraded. There is no CPU fallback: no Metal device is
`PM_ABSENT`, and a launch in which any Coax check failed is `PM_CHECK` and its
results are discarded. Apple GPUs have no binary64, so `f64` is a float-float
pair (APPLE-FF-v1) whose arithmetic and `exp`/`log`/`sqrt` are qualified
against binary64 within frozen bounds (`psionic/tests/run_metal_kernels.sh`),
and the whole decision meets chapter 30's frozen 0.0005 rule against the CPU
reference (`psionic/tests/run_elephant_metal.sh`); the
[chapter 31 record](docs/verification/psionic-ch31-2026-10-29.md) holds the
evidence.

## Psionic complete model mapping publishes the layer stack together

`psionic/lib/model-io/model-map.coax` binds the authenticated inventory's
complete hybrid schedule, stages decoded parameter vectors and rebases layer
views onto caller-owned storage. Public layers and parameters publish only
after all layers bind; scratch may change on failure. Authentication, valid
disjoint allocations, immutable file bytes and lifetimes remain caller duties.
Check: `psionic/check.sh` covers authority mutation and native plain/sanitized
checks; the [model-map record](docs/verification/psionic-model-map-2026-10-29.md)
records the scope and original-file evidence.

## Psionic request blocks start fresh and publish atomically

`psionic/lib/qwen-request.coax` executes a caller-specified token stream in
prefix and suffix blocks, with a fresh linear state and deterministic rotary
positions. Live state and probabilities publish only on success; scratch may
change. Authentication, tokenizer and split identity, slots, allocation,
alignment, lifetimes and exclusive access remain caller obligations. Check:
`psionic/check.sh` covers authority mutation and native plain/sanitized checks;
the [request record](docs/verification/psionic-request-blocks-2026-10-29.md)
describes the tested cases and limits.

## Psionic original layer mapping binds validated views

`psionic/lib/model-io/layer-map.coax` maps the fixed hybrid schedule by exact
name, BF16 shape and finite decoded vectors. It stages owned parameters and
borrows matrices from caller-owned immutable files. Authentication of the
inventory, valid disjoint storage and its lifetime remain caller obligations.
Failed mapping does not publish a layer; scratch may change. Check:
`psionic/check.sh` covers Coax authority mutation and native plain/sanitized
checks; the [layer record](docs/verification/psionic-layer-map-2026-10-29.md)
describes original-file evidence and limits.

## Psionic original sessions and typed text decisions

`psionic/lib/decision-session.coax` admits a pinned target, authenticates the
original artifacts and packed tokenizer, binds the exact inventory and maps
the complete text stack. Names, tensor bytes and bound layer storage remain
borrowed and immutable for the session lifetime. `decision-prepare.coax`
turns one typed request into a staged token stream and a safe shared-prefix
boundary; the caller must associate that result, the tokens and option names
with the authenticated session. `decision-answer.coax` uses pinned slot IDs,
resets request state, runs prefix and suffix and maps the chosen index back to
an option. The caller supplies capacity, temperature and valid, disjoint,
live storage; native address checks do not establish allocation validity or
exclusive ownership. Workspace can change on failure, but session, request
output, answer probabilities and live state publish only on success. The text
profile does not admit images or provide a service or calibration.

Check: `psionic/check.sh` covers debug/release checks, authority mutations,
book examples and native plain/sanitized adapters. The chapter 10 verification
record describes the independent original-weight comparisons and limits.

## Psionic decision plans size caller storage for one pinned request

`psionic/lib/decision-plan.coax` derives a pinned target's session workspace,
state and request workspace from its own dimensions and shards, using the same
layouts `ps_open` and `ps_answer` check. It refuses unknown targets, unbounded
capacity and token or prefix counts that the request would refuse. A plan is
arithmetic, not an allocation or an admission of host memory: checkpoint
mapping, page cache and allocator behavior remain the caller's concern. Check:
`psionic/check.sh` covers authority mutation and native plain/sanitized checks;
the chapter 11 verification record gives the measured Mammoth run.

## Psionic token blocks stage model state through full-vocabulary readout

`psionic/lib/qwen-tokens.coax` gathers original-format embedding rows, runs
caller-bound hybrid layers, applies the final offset RMS norm and projects all
vocabulary rows in chunks before reading the requested slots. It stages state
until readout succeeds; errors preserve public state and probabilities. The
caller owns authenticated immutable weights, complete layer mapping, token and
slot identity, rotary positions and state history. `qwen-rope.coax` supplies
the pinned 64-dimensional text-only Qwen rotary coefficients. Native adapters
observe addresses, while valid allocations, extents, alignment, lifetimes and
exclusive access remain caller duties. These primitives alone do not implement
a complete request or a service.

Check: `psionic/check.sh` includes debug/release authority mutation and native
plain/sanitized checks, including a vocabulary larger than one projection
chunk. Original model and independent runtime qualifications are recorded
separately.

## Psionic decision-model targets use pinned original identities

`psionic/lib/decision-models.coax` exposes only the pinned Elephant and Mammoth
text profiles, including exact revisions, sidecar and shard identities,
architecture dimensions and bounded state capacity. The target metadata alone
does not authenticate supplied bytes or qualify execution. Bundle admission
must check every original and callers retain valid immutable storage.

Check: `psionic/check.sh` includes zero-survivor authority mutation and native
plain/sanitized checks; see the [target record](docs/verification/psionic-targets-2026-10-29.md).

## Psionic rank-five bundles bind complete original identities

`psionic/lib/model-io/rank5-bundle.coax` authenticates caller-pinned config,
tokenizer, index and every shard, validates all rank-five inventory metadata
and binds the complete caller-supplied tensor manifest before publishing it.
The sharded index must map each tensor exactly once and its declared total
bytes and optional total parameters must match the scanned originals. The
caller supplies trustworthy pins and keeps borrowed files and names immutable
and alive; C observes address ranges only. The rank-four v1 contract remains
separate.

Check: `psionic/check.sh` includes zero-survivor authority mutation and native
plain/sanitized tests; the [bundle record](docs/verification/psionic-original-bundles-2026-10-29.md)
distinguishes original-file evidence from model execution.

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

## Psionic interchange carries inert weights and explicit storage leases

`psionic/lib/interchange.coax` owns `psionic-interchange-v1`: 1–16 uniquely
named nonempty rank 0–4 F32/BF16 tensors, at most 1,048,576 elements per tensor,
4 MiB aggregate payload and 16 KiB header. It writes canonical lexicographic
Safetensors maps without dtype/layout/name conversion and rejects nonfinite
encodings. Inspected names are owned metadata; payload offsets borrow the
file. Refusals preserve public output and report zero written. This separate
profile leaves the original bundle contract below unchanged.

`psionic/platform/interchange.h` exposes fixed-width versioned ABI v1 on LP64
little-endian binary32 hosts, without generated Coax types. DLPack accepts
legacy and versioned 1.0–1.2 CPU:0 contiguous f32, including scalar/empty tensors;
exports are read-only versioned 1.0 with explicit strides. Coax owns layout,
address-observation admission and reference transitions; C performs allocation,
byte copies and managed-deleter effects. Raw transport preserves f32 bits;
weight/numerical admission remains stricter.

A recognized nonnull carrier and empty nonnull output transfer ownership,
including validation refusal; earlier argument errors do not consume it. A
versioned major mismatch reads only its stable prefix and calls its deleter.
Successful imports pin the producer until the last explicit lease; exports
own another lease. Opaque capabilities, truthful pointers/extents, producer
lifetime for null deleters and exclusive coordinated access remain caller
duties. No GPU stream, arbitrary graph or optimizer conversion is claimed.

Check: `psionic/check.sh --only interchange` exercises debug/release Coax,
native plain/ASan/UBSan and real NumPy producer/consumer lifetimes, with an
independent external-weight to Coax-training to portable-inference round trip.
Full mutation is omitted under owner instruction. See
`docs/verification/psionic-ch4a-2026-10-29.md`.

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

## Psionic owned tensor placement keeps device policy and arithmetic in Coax

`psionic/lib/tensor-placement.coax` and `tensor-device-host.coax` own the
`tensor-device-v1` bounded device/dtype contract. Opaque native sessions and
handles hold authoritative Coax records; metadata snapshots do not retain
allocations. Views retain a root, copies own a new root, and ready storage is
immutable. Host API access is exclusive. Storage/session IDs never repeat.
Pending outputs and borrowed inputs cannot be freed. Explicit wait/cancel
drains work before releasing input reader claims; aliased input roots count
both reads, and independent pending outputs release only their own claims.
Cancellation invalidates its output, numerical failure invalidates only the
result, and a driver error poisons further session execution. Idle references
can still be released; close refuses live allocations. There is no implicit
wait, implicit placement or CPU fallback.

`tensor-device-kernels.coax` supplies the actual CPU/CUDA/Metal finite packed
casts, dtype-grid uniform generator and add/ReLU/dropout program. Integer RNE
preserves finite f16/bf16 encodings, signed zero and subnormal cast bits.
Equal dtypes retain their type; mixed pairs produce f32. Program arithmetic
uses f32 then one output narrowing; numerical flags prevent publication of a
partial/nonfinite result. Counter32-v1 uses explicit seed/stream/logical ranges
without wrap; allocation or scheduling consumes no hidden random state. It is
a reproducibility primitive with no cryptographic/training-quality claim.

Native code allocates Coax-admitted resident/temporary buffers and performs
copy/driver effects. Export stages a complete disjoint logical readback;
GPU copies/programs run the requested generated kernels. CPU work completes
eagerly but waits logically; CUDA queues immediately; Metal commits its batch
at wait. Tensor payload budgets exclude driver/module/allocator overhead and
separately bounded host transport scratch. This small profile does not extend
all 27 CPU operators to GPUs or supply in-place updates/autograd.

Check: `psionic/check.sh --only tensor-device`, with explicit `--metal` and
`--cuda` device harnesses, qualifies independent integer/format references,
every finite half/bfloat16 encoding, RNE boundary vectors, all dtype pairs,
strided transfers, completion/lifetimes, failure preservation and native
ASan/UBSan in debug/release. Numerical bounds/device behavior and the owner's
omission of full mutation runs are recorded in
[chapter 3b verification](docs/verification/psionic-ch03b-2026-10-29.md).

## Psionic tensor operators infer support and publish complete value results

`psionic/lib/tensor-ops*.coax` owns `CPU-f32-wide-views-v1`: 27 bounded CPU
operators with pure shape/dtype inference and explicit analytical value VJPs
for 25 of them. The machine-readable `psionic/fixtures/tensor-ops/operators.json`
records exact ranks/options, f32 values/masks, i64 indices/auxiliaries, CPU
backend and differentiation status. It does not widen chapter 3's old profile.
Operands reuse chapter 2a's admitted signed-stride views and allocation epochs.

Forward work and direct VJP traversal pairs have separate 1,048,576-unit
bounds. Products/reductions use f64 and ordered compensation before checked f32
narrowing; FMA/fast-math are disabled. Empty identities, arithmetic domains,
last-update-wins behavior and stable first-index subgradients are explicit.
Only supported convolution stride one is admitted. Every logical operand is
finite, including unselected/overwritten values. Public values/indices or both
gradients publish only after all staging succeeds; arithmetic refusal may dirty
scratch but preserves all public outputs and returns zero counts.

C only observes address ranges, including metadata and integer buffers. Native
caller obligations include truthful aligned/living spans, immutable inputs,
exclusive access and fresh unpublished results. Registered tensors are replaced
through chapter 2a's versioned scatter, and saved graphs must validate their
version tokens before a functional VJP. Returned gradients use logical input
coordinates; shared-backing/view accumulation and automatic tape integration
remain separate. No dtype promotion, general operator options, GPU execution or
large-model gradient throughput is inferred from this bounded CPU reference.

Check: `psionic/check.sh --only tensor-ops`; authored debug/release checks,
independent scalar and central-difference references for 38 cases in three
layouts, explicit tie conventions, actual aliases/work/domain/late-publication
refusals, native plain/ASan/UBSan and the exact book transcript. See
[chapter 3a](docs/verification/psionic-ch003a-2026-10-29.md).

## Psionic CPU tensor views share one authoritative storage version

`psionic/lib/tensor-view.coax` adds a separate bounded borrowed-f32 view layer;
chapter 2's contiguous/nonempty contract is unchanged. Views have rank 0–4,
nonnegative extents, signed element strides, absolute origins and allocation
identity/epoch. Extents, absolute strides, nonempty logical element counts and
backing counts are bounded by 1,048,576. Empty tensors dereference no element.
Every public boundary re-admits metadata and proves all reachable offsets fit.
Writable layouts require a sufficient injectivity proof; broadcast views are
read-only, and interval alias queries conservatively retain possible overlap.

Gather/scatter validate exact owner/backing extents, capacities, finite selected
or source values and disjoint declared writable/read spans before any writes.
Refusal preserves all spans and returns zero written. Scatter increments the
single authoritative version exactly once on a successful nonempty write;
saved tokens detect any alias write and refuse retired/reused allocation epochs.
Empty writes do not increment; version overflow refuses before staging.
The C adapter only observes ranges, including the owner record. Caller-owned
storage, truthful valid/aligned extents, fresh identities/epochs, one live
record per allocation, mutation through this boundary and exclusive synchronous
access remain obligations. This is not automatic ownership, unforgeable native
handles, concurrency, a backward engine or device placement.

Check: `psionic/check.sh --only tensor-view`; independent native integer address
and uniqueness enumeration, actual aliases, finite-bit copying, short buffers,
zero-element null views and maximum-size transpose, plus debug/release native
ASan/UBSan and the exact book transcript. The dated record is
[chapter 2a](docs/verification/psionic-ch002a-2026-10-29.md).

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

## Inference credit is bought only with payments our own Spark wallet received

`POST /api/v1/inference/credit` (`coax/web/api/inference.c`) sells prepaid
inference credit under NIP-X402's `openagents:spark-receiver:1` profile.
Credit is added only when all of these hold, checked by the server, never
taken from the client:

- the invoice was issued by this server for this exact request: its row in
  `inference.topups` binds the payment hash, the invoice, the pubkey, the
  msat amount and the request digest, and the retry recomputes the digest
  from the actual request;
- the server's own Spark wallet reports that exact invoice received and
  complete, for at least its amount, with a preimage whose SHA-256 is the
  payment hash; a payer's preimage, when sent, must equal it, and alone
  never credits;
- the replay key `lnbtc:000000000019d6689c085ae165831e93:<payment_hash>` is
  inserted into `inference.used_payments` in the same transaction that adds
  the credit, so one payment credits once (`duplicate_settlement` after);
- the amount is within the top-up bounds; mainnet only, Bitcoin only.

`/api/v1/inference` admits admins, the allowlist, and any signed key whose
credit is above zero. A paying key's call is debited in msat at the cached
BTC/USD rate when it is metered; with no rate younger than an hour, paid use
is refused (503), never priced at a guess. The daily caps still apply to
every key. The wallet process (`coax/web/api/spark/`) is the only code that
imports the Breez SDK; it issues invoices, reports receives and reads the
rate, and sends nothing. Its seed exists only in Secret Manager. The wallet
is hot and holds small balances; the owner sweeps it.

Check: `coax/web/api/test.sh` (the `credit:` and `drawdown:` checks in
`test_inference.py`, against a fake wallet: unbound, unpaid, wrong amount,
wrong preimage, replayed and out-of-bounds top-ups refuse; one payment
credits once; no rate refuses paid use); `coax test coax/lib/x402/x402.coax`.

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
still gates authority functions, and default mutation covers reachable helpers
while requiring entry/authority coverage. Optional `--root-only` selects
mutation sites using the procedure's physical declaration provenance; it does
not filter baseline checks or weaken authority coverage. Scoped scores do not
qualify dependency mutation coverage. Shard receipts bind scope even when
filtered and default inventories coincide; merging modes refuses. Compiler/evaluator/emitter, native ABI and
filesystem behavior remain trust inputs; finite checks are not proofs.

Check: source-kernel authority checks and zero-survivor mutation, explicit
`sources.c/h` seed identity in the Python-free bootstrap, final regeneration
from `lib/compiler/compiler.coax`, and `coax/tests/sources.py` (real driver,
cycle/diamond/symlink tests, scoped authority, relative descriptors, original
file diagnostics/traps, graph limits, helper-free relocated compiler, native
ABI and ASan/UBSan with an independent widened budget oracle). The book
capstone and real iOS HUD consume loaded policy/types. `coax/tests/mutation_scope.py`
checks default frozen-compiler parity, imported baseline failures, physical
scope selection, worker/shard parity and scope-separated receipts. `coax deps` is a checked
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

## The public Pylon board verifies original events and labels live observation

`apps/openagents-web/pylon.coax` displays read-only Pylon state. Its private
C byte bridge passes the identical original event to the shared Coax
`landing-nostr.coax` reader: beacon projection requires the event ID,
BIP-340 signature, decoded-content SHA-256, provider binding, schema and
freshness checks. Activity requires the same original event verification.
No public Wasm export accepts an admitted flag or a projected machine.
Owner authorization, hardware claims and encrypted outcomes are not proved
by a self-signed beacon. The agent guide states these limits.

Kinds 26910/27010 are ephemeral. The board counts only live observations
from its opening time, for keys with an admitted beacon, with duplicate IDs
suppressed. Its bounded 64-ID window freezes explicitly when full; it never
evicts IDs and silently recounts replays. Expired/non-online beacons are
labelled offline. Newer generations cannot be replaced by older ones.
`pylon-policy.coax` is an authority-marked display-policy unit with a
zero-survivor mutation gate. `coax/web/tests/pylon_page.test.ts` exercises
the actual page Wasm and C bridge with independent signed fixtures, including
tampered signatures/content, invalid beacons, old activity and duplicates.
Desktop/phone page-start and golden checks cover `/pylon`; release smoke
also requires its Markdown guide and the verbatim audience join script.

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
- the development-only error note `coax serve` shows on localhost;
- a realm's loading screen (`coax/web/loader.js`, an owner decision,
  2026-10-30): the realm's name, a bar moved only by what really loads
  (engine bytes, compile, GPU, shader pipelines, asset bytes, textures, the
  world's first frame) and the line naming it, in plain HTML and CSS so it
  shows at the first paint, before the engine can draw. It is gone on the
  world's first frame; on a failure it says what failed with one working
  action (no WebGPU: the realm's text page; anything else: try again). It is
  only on realm pages, written by `realms.ts` (a made realm's by the host),
  and holds nothing else.

Check: `coax/web/check-visuals.sh` (run by `coax/web/deploy.sh` before every
upload) refuses visible elements, inline styling of UI, and DOM-built UI in
the app's HTML, the host and `coax/web/unlock/`; the loading screen's block
may hold only its own elements, and `loader.js` styles only marked lines.


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
load-engine and world-leave; Dead Gears' page-view, world-enter,
load-engine, wave and world-leave; and Hecate's Cemetery's, the Sandbox's,
the Grid's, Everglade's and Fairy Forest's page-view, world-enter, load-engine
and world-leave (migration 0069). What people do inside a realm is recorded
with `analytics.coax`'s track_act, track_once, track_tally and track_walk:
counted, batched and sent at most every 30 s for the frequent ones, never
per frame, no words but a chat's length. A release never promotes a build whose staging
walk stops sending one: `coax/web/goldens.ts` plays the journey in headless
Chrome, runs in through the house's front door, clicks and leaves, reads
every batch the pages send (a realm's after its own walk, then leaves it)
and names what is missing, and `release.sh` stops
before production. The journey through the house is walked in full on
every release; a realm the walk skips as unchanged since production's last
staged release (its golden comparison only) still opens, starts, draws and
sends its contract. Production gets a brief start check by default and the
full walk with `release.sh --thorough`. In production the Health card on /admin/analytics shows,
per surface, the last event, the last hour and day, and which contract
events no page sent in a day. Changing the contract changes both lists in one
commit.

Check: `coax/web/release.sh` (staging walk), `coax/web/api/test_health.py`
(via `coax/web/api/test.sh`), which also checks the two lists agree.

## Coax group kernels share memory only through owned writes and barriers

`coax cuda` and `coax metal-compute` admit `for thread: 0..<T` loops (T a
multiple of 32, at most 1024) as group kernels: one block or threadgroup of T
threads per item, every thread loop fenced by a barrier on both sides
(`__syncthreads()`, `threadgroup_barrier(mem_threadgroup)`). A thread loop
writes a shared array only at elements its thread owns (`thread*k+c`,
0 <= c < k, plus nonnegative multiples of T*k), code between thread loops
reads shared arrays and never writes them or the views they feed, and a
kernel's shared arrays fit in 32 KiB; anything else is E0700/E0702 with its
own reason. A launch with the wrong group size traps (CUDA) or fails with
`PM_CHECK` (Metal), never computes. The Apple matrix primitives
(`coax/lib/compute/apple.coax`) are Metal-only, group-kernel-only, and keep
the CPU meaning written next to them. On the CPU a thread loop is an
ordinary loop, so the C build is the oracle the device must match bit for bit.

Check: `coax/lib/compiler/compute.coax` (`coax test`, zero-survivor
`coax mutate`), `coax/tests/cuda.py`, `coax/tests/metal_compute.py`,
`psionic/tests/run_group_kernels.sh` (Metal locally, CUDA on `coderos-4080`).

## GGUF descriptors publish only after bounded structural admission

`gguf-v3-le-v1` (`psionic/lib/gguf.coax`) accepts little-endian v3, pinned
GGML layout IDs and explicit header/file/count/value budgets. Every dimension
product, encoded block size, aligned offset and payload end fits before use;
keys/names are unique, UTF-8 is valid, booleans canonical, ranges disjoint and
header padding zero. Zero-tensor upstream headers may omit final padding.
The immutable header owns borrowed strings. Public descriptor arrays change
only on success; separate scratch may change on refusal. C observes address
spans only. Reading structure does not authenticate or execute tensor bytes.

Check: `psionic/tests/check_gguf.sh --mutate --jobs 8` (CPU only, independent
struct oracle, pinned original headers, debug/release and ASan/UBSan).

## Tokenizer families and bounded Jinja retain their named profiles

`psionic/lib/tokenizer-families.coax` owns bounded ASCII BERT WordPiece and
T5 SentencePiece-style Unigram wrappers, plus normalized UTF-8 Unigram.
Immutable unique sorted pieces have explicit count/byte/ID/finite-score limits.
WordPiece rolls a failed word back to one unknown; Unigram maximizes score,
retains first-encountered ties and fuses adjacent unknown IDs. Original ASCII
normalization DATA is pinned; full Unicode BERT/T5 normalization and added-token
recognition are separate work. `chat-template.coax` admits its entire bounded
Jinja subset before rendering, never interprets message content, bounds loops,
condition depth and output, and refuses unsupported syntax even in inactive
branches. Value results own their storage; errors return zero written payload.

Check: `psionic/tests/check_tokenizer_families.sh --mutate --jobs 8`: CPU-only,
290 full original-vocabulary vectors, 12 independent Jinja outputs including
TinyLlama's actual template, debug/release and native ASan/UBSan.

## The byte bigram trains and generates under an explicit bounded CPU profile

`psionic/lib/byte-lm.coax` admits 2–4096 bytes, 1–64 full-batch SGD updates,
rate (0,64], complete 256x256 f64 tables and disjoint public/scratch/trace spans.
It trains from zero using only the training partition, normalizes gradients by
all transitions, and publishes only the completed table and loss trace.
Evaluation uses the full 256-byte softmax and finite logits in [-4096,4096].
Generation uses `splitmix64-top53-v1`, one seed/counter draw per byte, ascending
strict CDF buckets and a final residual bucket. Length and counter exhaustion
are checked before output writes. Licensed synthetic whole-document splits
are fixed and disjoint; their likelihood does not claim general language quality.
C observes spans only; Coax owns arithmetic and admission. Valid pointer/view
lifetimes and exclusive synchronous output access remain caller duties.

Check: `psionic/tests/check_byte_lm.sh --mutate --jobs 8` (CPU only): independent
full-table SGD/likelihood/sample capture, finite differences, real aliases,
debug/release, native plain/ASan/UBSan and exact book output.

## Learned byte BPE keeps training and inference boundaries identical

`psionic/lib/bpe-train.coax` owns `byte-bpe-nanochat2-u17-v1`: the pinned
nanochat two-digit regex over Unicode-17 properties, no normalization or added
tokens, and 256 byte IDs followed by at most 64 merge IDs. Highest overlapping
pair count wins, ties choose the lowest pair of IDs, and replacements are
disjoint left to right. No pair crosses a piece or document boundary.
Up to 16 documents / 4096 UTF-8 bytes and 64-byte expanded tokens are admitted;
late growth failure returns an empty model. PST5 packing validates rule order,
references and uniqueness, checks capacities/actual aliases before writes and
publishes only a completed table. The new encoder shares chapter 5's layout,
span reader and ranked BPE core, with its own explicit splitting profile.
Inputs/class tables retain immutable valid storage and recorded identities.
C observes spans and performs verification file effects only.

Check: `psionic/tests/check_bpe_train.sh --mutate --jobs 8` (CPU only): full
64-merge independent recount, exact persisted PST5, 147 Unicode/ID/byte round
trips, 20 tiny training traces, debug/release and ASan/UBSan.

## The homepage publishes verified live observations with explicit freshness

`/` and `/new-land` use the same Coax homepage. Public event ID/signature,
beacon digest/freshness, subscriptions, bounded history repair and project fold
remain in shared Coax. C bridges effects and scalar byte buffers; JavaScript
supplies generic sockets and a wall clock. No runtime Nostr SDK or signing key.
Incomplete project reads preserve the prior projection. Expired Pylon slots
become unknown; signed answer/status counts are deduplicated observations since
this tab's watch began, not completed-job or latency measurements. Worker labels
are project-authored claims. Featured identities/layout and Git trees are deploy
snapshots; no simulated people enter the production view.
The homepage PYLONS board shares that same verified watch and owned rows.
Its join/guide actions use the generic host; full model identities and public
keys wrap in Coax, and panel scrolling never launches an action or moves the Grid.

Frozen real data is used only for deterministic golden checks. Release readiness
also opens the ordinary homepage with real sockets in muted headless Chrome and
requires all six featured projects, the watch and beacon EOSE. A genuine empty
provider set is allowed. Scope and checks: [live homepage record](docs/verification/2026-10-30-live-network-homepage.md).

## Psionic fleet results retain the selected identity and attempt

`psionic/lib/fleet/` is the chapter 20 opt-in loopback JSON adapter. A verified
fresh v1 beacon routes only an online free slot advertising the configured
exact decision identity and free terms. Held generations do not roll back;
a newer withdrawal removes the old route. Unknown unrelated services never
consume route slots. The bounded 32-coordinate history lives until restart.

A remote result publishes only after relay acceptance and full event
verification, NIP-44 opening, exact provider/buyer/job/request/attempt binding,
matching service identity/door, valid asked answer shapes and a live budget.
An older generation's result cannot replace a current attempt. Local fallback
is explicitly loopback, completely framed HTTP200, and validates the same
exact identity and answer contract. Stop does not start new fallback work.
No timeout or lost ephemeral event is represented as a completed decision.

The signing identity is fresh, random and test-grade, in memory only. Public
beacons remain signed provider declarations, not remote model attestation.
This does not approve production keys, public ingress, owner-auth beacons,
v2 groups, payments, receipts, durable retries or production provider trust.
C owns socket/lifecycle effects only; Coax owns the protocol and routing.
Check scope and actual commands: `docs/verification/psionic-ch20-2026-10-30.md`;
the host has its separate Mac/Linux sanitizer record.

## Private child input uses an owned pipe and one write deadline

`cxproc_flush_timeout` is the bounded native write effect for private
checker input. It accepts only registered child stdin on a writable FIFO
fd >= 3, preserves unsent bytes after timeout/error, restores mutable flags
and never forwards opaque child output. Partial retries cannot replay an
already delivered prefix. The original blocking flush remains unchanged.
The caller in Coax controls the checker program, trust, payload bounds
and deadline. Native Mac/Linux sanitizer evidence is in
`docs/verification/2026-10-30-process-flush-timeout.md`.
