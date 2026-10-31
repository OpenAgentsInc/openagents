# Original Cosmos text prefix on Metal

`groot-text-metal-plan.coax` binds the 178 active original text tensors for
GR00T-N1.7-LIBERO's 16-layer Cosmos prefix: the vocabulary embedding, eleven
tensors per decoder layer, and final RMS norm. The independent native oracle
reads both committed original shard headers through Coax `gca_header`, then
checks every selected range against a separate Python reading of the pinned
header metadata. The full headers contain 1,031 tensors; the unused LM
readout is outside this decision-feature graph.

Coax chooses every operator and scalar/view argument. The graph has 18
stages and 325 micro-op launches, supports checked 137–512-token prefixes,
and returns `[tokens,2048]` features. It uses per-head Q/K RMS normalization,
128-channel interleaved mRoPE, grouped-query attention with sixteen query and
eight KV heads, causal attention, a gated SiLU MLP, residuals, and the three
DeepStack additions after the first three decoder layers. It does not expose
LM logits, autoregressive generation or a KV decode interface.

The current mixed precision profile keeps original BF16 matrix payloads,
F32 projections/storage/affines/attention, and qualified wide RMS norm
statistics, centering and division. `gaw_norm` uses the existing float-float
f64 execution profile on Metal; it is not native GPU IEEE binary64. It has
the same ABI as the prior F32-statistics norm, so changing its kernel name
and library preserves every buffer extent and graph argument. That change
has its own zero-survivor plan checks and actual GPU integration evidence;
unchanged native tensor-binding/ABI and primitive gates are reused.

The focused native oracle passes 1,196 cases in debug, release and ASan/UBSan:
all 178 original bindings, typed argument byte extents at 137, 140 and 512
tokens, all eighteen ordered cursor transitions, device/check error receipts,
and atomic refusal for nonfinite raw F32 words. Its independent header facts
match bit for bit in all three builds. All 836 entry-root mutants were
killed. `text-metal-plan-qualification.json` records these metadata checks,
source hashes and the earlier emitted C artifact hashes; that receipt does
not claim neural numerical parity.

The separate GPU comparison consumes the frozen original SDK capsule or the
actual Coax vision outputs. Those are different integration inputs and are
recorded separately. The original F32-norm component trial passed 286,720
feature values at frozen `2e-3 + 2e-3*abs(reference)` bounds, using 140 tokens
and precomputed original camera/deepstack outputs. Its `0.06660679 s` GPU
command time excludes original weight loading/upload, compilation, capture
and comparison. Current mixed-profile/coupled measurements belong to their
own comparison records. Metadata checks across three prefix lengths do not
prove a uniform numerical error bound for every possible request.

```shell
psionic/tests/robotics/check-groot-text-metal-plan-native.sh NEW_OUTPUT_DIRECTORY
```

The model runs through required Metal effects with no CPU inference fallback.
Token/image preparation uses the existing Coax request and text primitives.
Full vision→text→action composition, action denormalization, simulated task
success and physical-robot control need their separate qualification records.
