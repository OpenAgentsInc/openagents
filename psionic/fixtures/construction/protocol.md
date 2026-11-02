# Chapter 254 construction protocol

2026-11-02. Written before baseline training, model calls and held-out
measurement. Working-tree exploratory study; no independent preregistration.
The author knows the exact algorithms and chapter 253's failures. Fresh
stateless proposer calls have no files, tools, evaluator source or held-out data.

Tasks: max(a,b), abs(a), c ? a : b for Boolean c, and four logits -d0..-d3
whose argmax selects the lowest neighboring shortest-path distance, lowest
action winning ties. Arithmetic inputs [a,b,c], a,b integers in [-64,64],
c=0/1. Control inputs are four distances 0..64, with 64 for blocked moves.

Direct architecture: 3→4 ReLU→1 (24 parameters), control 4→4 ReLU→4
(56), each with affine input/output skip. Row-major order: hidden weights,
hidden biases, output weights, skip weights, output biases. Model replies
must be a complete integer JSON array, coefficients [-1024,1024]. Continuous
trained baselines use the same arithmetic with binary64 weights.

Compiler condition: one postfix integer array per output. Tokens 0..inputs-1
push inputs; 10 add, 11 subtract, 12 ReLU, 13 negate, 14 multiply, 15 step
(1 iff integer>=1), 16 select consumes condition,a,b. Condition must be a
step-produced bit. At most 24 tokens per output, at most 4096 compiled
weights. Existing gate compiler owns lowering; its variable bounded gated
architecture differs from direct ReLU4. Count readout coefficients, including
zero-hidden-layer compiled artifacts, and report complete artifact identity.

Arithmetic supervised training: full integer [-4,4]^2 × {0,1}, 162 rows.
Development: [-6,6]^2 outside that square, × {0,1}: 176 rows.
Held-out: [-16,16]^2 outside [-6,6]^2 × {0,1}, plus all ordered pairs
from {-64,-63,-32,0,32,63,64} except (0,0), × {0,1}: 1936 rows.
Measure exact within 1e-9, within 0.01, MAE and worst absolute error.
Repeated irrelevant b/c values in abs are not independent task samples.

Control: chapter 110 maps A and S#G/.../... for development; maps B and C
for held-out transfer, every reachable nongoal start, horizon 128. Outcomes:
reached/starts, shortest walks, steps, collisions, actual policy forwards.
The policy is given true shortest-path neighbor distances, recomputed from
the current map. Native argmin and exact planning are strong references.
Frozen map-A Q-learning and Q-learning on each held-out map are separately
labelled conditions: state-only Q-learning has different observations and
online experience from the distance-conditioned zero-shot policies. No PPO
or general learned navigation claim. Record preprocessing/planning overhead.

Neural baseline: identical direct architecture, seeds 254..256, 1000 full-
batch AdamW steps, lr .01, beta .9/.99, epsilon 1e-8, no decay. Normalize
arithmetic a,b and regression outputs by4; control inputs/outputs by4;
fold those scales into exported weights. Control trains on 64 seeded
distance vectors in 0..6 (counter RNG seed25400), targets -distance.
Save weights before evaluation. No hyperparameter retries or early stopping.

Proposer amendment, before any task inference: the intended local GPT-OSS
server was killed by the host (exit137) during setup. Eight initial unready
transport calls are retained separately; the readiness-guarded second launch
started no trials. No task model answer or held-out prediction was observed.
Use the installed authenticated text-only Claude client, fixed model
claude-opus-5-5, effort low, tools/slash commands/MCP/hooks disabled, empty
working directory, no persistent session. At most $1 and 100 seconds per
call. Output-token ceiling is the client's default, not the earlier 512.
Record actual tokens, reported list cost and failures. Do not compare this
proposer's results with chapter 253 as if the budgets/models were equal. For each task/condition, one concise prompt
and one sparse-weight prompt, each stateless. The first candidate can use
up to two development-feedback refinements, stopping on exact development
success. No numerical tools. Hold all candidates before evaluating held-out
cases; choose the first chain by development exact score then MAE only.
Compiler intermediate values are range-preflighted by the gate evaluator
before actual weight execution; preflight refusals are counted, never scored
as predictions, and exclude a candidate from selection.
Record invalid proposals, model usage, every request/response, feedback and
wall time. An invalid candidate has no prediction; fallback is not credited.
Prompt variants are deterministic sensitivity probes, not independent seeds.

Baselines: native arithmetic/controller, hand-constructed same architecture,
canonical graph compiled by the existing compiler, all-zero network, three
trained models. Deliberately broken control and conditional artifacts must
fail. Record admitted arithmetic-core timing and native reference timing on
the same workload; do not confuse validation or interpreter time with inference.
All comparisons are exploratory, with no automatic promotion or deployment.
