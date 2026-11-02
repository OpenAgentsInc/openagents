# Original-model editing protocol

2026-11-02, working-tree exploratory protocol, frozen before live development
generation. Not independent preregistration. GPT-OSS-20B original revision
6cee5e81ee83917806bbde320786a8fb61efebee, existing pinned tokenizer/low-effort
Harmony recipe and gpt-oss-metal-f32-ff-v1. No Qwen3.5 reproduction is claimed.
Read-only authenticated originals, private GPU copy, capacity512, batch128,
64 generated tokens. Test commands serial/low priority, at most840seconds.
The initial batch8 note was corrected to the existing engine's automatic
batch128 before observing development results; no new batch-size policy is
implemented. Actual prompts are shorter than128 tokens.

Frozen cases are OS_CASES in original-study.coax: six development arithmetic,
six development retained, twelve held-out arithmetic and twelve retained.
Strict requested-answer accuracy removes ASCII punctuation/whitespace and
case from a completed assistant final and compares the complete remaining
alphanumeric/minus string. No analysis/incomplete output earns accuracy.
Record completion, token limits, syntax/call failures, generated count, tokens
per completion, three-repeat spans3..16, prompt IDs, output IDs and wall time.
No claim that all these failures are loops; include nonlooping errors/limits.

Original parity: two recorded original CPU prefixes, all logits under the
existing .0005 scaled-error rule with exact argmax, before intervention.
Two BF16 readout rows, resolved by encoding literal analysis/final, each
2880 columns, form separate named windows. All other weights remain original.
Original-window-v1 binds model/shard pins, tensor/shape/offset/dtype and bytes.
Replacement rounds F64 to F32 then BF16 ties-to-even. Save every window and
candidate identity before opening held-out generation. A failed upload ends
the process before inference; restore both original rows before each edit.

Direction: mean normalized final-RMS activations on the six development
arithmetic prefixes followed by the channel marker. Normalize its L2 length
to 1/16 (a fixed scale, not tuned). Rank-one repair adds alpha*d to final row, subtracts alpha*d from
analysis row. This attempts earlier final answers under a small budget and
may remove useful reasoning. Never infer general reasoning improvement.

Matrix IDs:0 base greedy;1 sampling-only(.6,top-k20,top-p.95,seed255+case);
2 force final in prompt without weight edits;3 no-op;4 zero analysis row;
5 zero final row (negative control);6/7/8 direction alpha1/2/4;9/10/11
count/norm-matched random controls(seeds255..257) for8;12 rank-one channel
fine-tuning;13/14/15 bounded LLM proposals from selected weights only,
numerical row analysis, or activation/development observations. A bad or
declined proposal is retained, never credited as an edit. Random controls
shuffle replacement BF16 codepoints among columns with identical original
codepoints, preserving exact support count and squared-change multiset.
This constrained permutation is not a uniform random direction.

Fine-tuning: one scalar alpha, frozen d, two-class final-vs-analysis loss on
the six development channel activations, 200 AdamW steps, lr.05,beta.9/.99,
eps1e-8,no decay; clamp alpha[-4,4]. It is a channel-only rank-one fit, not
full-model LoRA or next-token language-model fine-tuning. No retuning.
LLM uses installed authenticated text-only Claude Opus5.5, low effort, tools,
MCP/hooks/browser disabled, empty cwd, no persistent session; one call/arm,
max$1 and100sec each. It may propose [analysis_scale,final_scale,alpha],
scales integer0..2 and alpha integer-4..4. Weights/numerics arms see no
activation or generation result; evaluation arm sees development only.

Select edited winner by development target exact, retained exact, then fewer
generated tokens, earlier ID on tie. Eligible only if encoded identity differs
from base. Predeclared target: at least2 additional correct held-out arithmetic
answers and no loss on held-out retained versus base. Also record teacher-
forced log loss and confident wrong first-channel choices. Log loss alone
cannot pass. Independent quality qualification remains unverifiable without
additional model/task/seed blocks, even if this finite worksheet passes.
No promotion, live replacement or original-file writes. No-op and exact
byte restoration are explicit controls, not repeated baseline confirmations.
No numerical/evaluation tool access is granted to the proposer itself.

## Screen amendment, before any edit or held-out generation

The initial six arithmetic questions all pass in20..37 tokens. They provide
no improvement headroom; retain them as nominal controls. Freeze18 new cases
(IDs36..53) now: six harder arithmetic development, twelve held-out. Primary
target is this new block; original cases remain retained controls. Final
development has6 primary plus12 retained; held-out has12 primary plus24
retained. Do not change this block after seeing its answers. Direction and
channel fit now use the six primary development cases, not nominal examples.
The original H2O answer was valid H₂O. Correct answer normalization to handle
all Unicode subscript digits0..9 before candidate evaluation; retain the
original v1 boolean and raw output. New scoring is original-answer-v2. This
is a scorer failure, not an original-model reasoning failure. There is no
loop evidence from the screen. Targets/floors remain +2 primary held-out
answers and zero retained loss, now over the expanded retained block.
Weights-only and numerical-only proposals must use alpha0; activation-derived
directions are available only to the activation/evaluation proposal arm.

Before any held-out generation, add two supplementary sampling-only blocks:
IDs17/18 use seed511+case and767+case, otherwise identical to1. They do not
select edits. This measures sampler sensitivity, not independent greedy
baseline/model blocks. Exact duplicate greedy artifacts may carry their
source traces with an explicit same-digest receipt; the no-op3 and
rollback16 controls still execute. Carried observations are not new trials.
Also freeze ID19: unchanged greedy weights/prompt with128 output tokens,
a separate more-generous-budget baseline. Its additional compute is counted;
it cannot select edits or earn the64-token finite goal. This budget arm is
fixed after observing development limits and before any held-out generation.
