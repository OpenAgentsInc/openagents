# 254. Write a network from a specification

> **Status: implemented — construct-relu4-f64-v1, bounded CPU construction study.**

An LLM can write working weights for these small calculations. Direct weights
and compiler-assisted programs both produce exact maximum, absolute value and
Boolean conditional results on the frozen cases. Both also produce an optimal
controller given neighboring cells' true distances to the goal. Three initial
proposals fail admission; development feedback repairs the first chains.
Ordinary Coax functions and hand constructions already solve every task and
cost less to execute. This experiment supplies no advantage over them.

The motivating [weight-space programming article](https://rikiyat.github.io/blog/weight-space-programming/)
reports direct construction of game policies and interventions in language
models. We test a smaller claim in Psionic's own arithmetic, with saved
proposals and independently checked weights. This is neither a reproduction
of those game experiments nor an original-model edit.
[Chapter 253](ch253-read-and-repair-a-models-weights.md) supplies immutable
named weights and identity; [229](ch229-compile-a-gate-graph-into-a-block.md)
supplies the compiler; [110](ch110-a-grid-world-that-learns-plans-and-recovers.md)
supplies the headless world and Q-learning baseline.

## A complete, bounded artifact

The direct architecture is `h = ReLU(Wx + b); y = Vh + Dx + e`, with four
hidden units and an affine skip. Arithmetic uses three inputs `[a,b,c]` and
one output: 24 binary64 parameters. Control uses four inputs and four outputs:
56. Flatten `W,b,V,D,e`, row major, including unused zero entries.

<!-- source: psionic/lib/construct.coax types -->
```jai
CnShape :: struct {inputs:int;outputs:int;}
CnStatus :: enum u8 {ok;shape;number;json;}
CnWeights :: struct {status:CnStatus;count:int;values:[56]f64;digest:[32]u8;}
CnOutput :: struct {status:CnStatus;count:int;values:[4]f64;}
```

[The reusable interface](../lib/construct.coax) admits 1–4 inputs and
outputs. `cn_forward` checks exact lengths, finite weights within ±2^20 and
finite inputs within ±64 before returning owned output. `cn_parse` admits a
complete JSON array of **integer** coefficients within ±1024; fractions,
exponents, prose, booleans and nested arrays refuse. Refusals publish zero
count and payload. `cn_tensors` names all five tensors for chapter 253's
`we_identity` and expected-old/replacement editing interface. Identity binds
the profile, shapes, names and every weight bit; it does not establish quality.

`cn_compute` is the arithmetic core for already admitted inputs; arbitrary
callers use `cn_forward`. Proposal coefficient range and runtime range are
distinct. No GPU path, original-model overlay or promotion permission is added.

The compiler condition accepts a postfix program per output through
[construct-program.coax](../lib/construct-program.coax). Tokens `0..inputs-1`
push inputs; `10,11,12,13,14,15,16` mean add, subtract, ReLU, negate, multiply,
integer step, select. Step is one iff its integer input is at least one.
Select consumes `condition,a,b` and requires a step-produced bit in the
condition position. Each output has 1–24 tokens. The front end checks shape,
types and stack arity before publishing a graph. Chapter 229 lowers it to
actual gated residual weights; the study executes those weights rather than
crediting the interpreter's answer.

This condition synthesizes programs for an existing compiler. Its variable
gated architecture differs from fixed ReLU4. An integer graph preflight refuses
intermediates beyond ±2^26 before floating-point weight execution. Refusals
count as missing predictions and exclude a candidate from selection. The
compiler's capacity here is 4096 weights.

## Work the calculation first

Maximum is `b + ReLU(a-b)`. For `a=-7,b=3`, the hidden value is zero and
the skip supplies three. Absolute value is `ReLU(a)+ReLU(-a)`. For Boolean
`c`, the conditional can use

`b + ReLU(a-b+128(c-1)) - ReLU(b-a+128(c-1))`.

When `c=0`, both ReLU arguments are nonpositive because `|a-b|≤128`;
when `c=1`, their difference is `a-b`. This proves the formula throughout
the declared bounded real `a,b` domain, including extremes and ties.

<!-- source: psionic/lib/construction-study.coax hand -->
```jai
cs_hand :: (task:int)->[56]f64 {
    p:[56]f64;
    if task==0 {p[0]=1;p[1]=-1;p[16]=1;p[21]=1;}
    if task==1 {p[0]=1;p[3]=-1;p[16]=1;p[17]=1;}
    if task==2 {p[0]=1;p[1]=-1;p[2]=128;p[3]=-1;p[4]=1;p[5]=128;p[12]=-128;p[13]=-128;p[16]=1;p[17]=-1;p[21]=1;}
    if task==3 {for a:0..<4 {p[36+a*4+a]=-1;}}
    return p;
}
```

The maximum program is `[[0,1,11,12,1,10]]`. Canonical conditional
`[[2,15,0,1,16]]` uses step and select. The model's repaired conditional
uses `b+(a-b)*c`, which compiles to fewer weights; Boolean `c` is part of
its input contract. Both are measured after lowering, with the full readout.
Pure affine outputs can live entirely in the readout: the controller has
zero hidden gated weights **and four nonzero readout coefficients**. It
still computes four negations; zero hidden weights is not zero computation.

## Freeze the questions before the answers

The [protocol](../fixtures/construction/protocol.md) was written in this
working tree before training and task inference. It is exploratory, not an
independent preregistration. The author knows the algorithms. The proposer
has no tools, files, evaluator source or held-out examples. Calls have fresh
empty working directories and no persistent sessions; MCP, hooks, slash
commands and browser access are disabled.

Arithmetic uses integer `a,b` in `[-64,64]`, Boolean `c`, and these partitions:

| Partition | Rows per task | Purpose |
| --- | ---: | --- |
| `[-4,4]² × {0,1}` | 162 | Supervised baseline training |
| `[-6,6]²` outside training square, both bits | 176 | Selection and feedback |
| `[-16,16]²` outside development square, both bits | 1840 | Held-out extrapolation |
| Ordered pairs from `{-64,-63,-32,0,32,63,64}`, excluding `(0,0)`, both bits | 96 | Held-out boundaries |

Exact means absolute error at most `1e-9`; also report error at most `.01`,
mean absolute error (MAE), worst error and refusals. Repeated irrelevant
`b,c` values for absolute value are not independent samples. A later
independent audit exhausts all 338 rows in `[-6,6]² × {0,1}`; all admitted
arithmetic proposals pass. Extra central rows did not select or refine any
proposal. Finite testing does not prove an arbitrary submitted network's
correctness outside the tested domain.

There are two stateless initial prompts per task/route: concise and sparse.
Only the concise chain is selected. It may receive two development-feedback
calls, stopping on exact development success without refusals. Select by
development exact count, then absolute error, retaining the earlier candidate
on a tie. The sparse variant never substitutes for a failed first chain.
All proposals for a task are saved before held-out evaluation. An invalid
artifact has no prediction and earns no fallback accuracy.

The intended local GPT-OSS server produced no task answers: eight calls
preceded readiness, and a guarded retry started no trials. The server later
exited 137; the cause is unknown. The infrastructure failures remain
[separately recorded](../fixtures/construction/startup-failure.csv). Before
task inference, the protocol switched to the installed text-only client's
**claude-opus-5-5**, low effort, at most $1 list cost and 100 seconds per
call. Its output ceiling is the client's default, not chapter 253's 512.
This is a different model and budget from chapter 253.

## Arithmetic results, including failures

The proposal ledgers for [maximum](../fixtures/construction/llm-0.csv),
[absolute value](../fixtures/construction/llm-1.csv),
[conditional](../fixtures/construction/llm-2.csv) and
[control](../fixtures/construction/llm-3.csv) retain all **19 calls**: ten
direct and nine compiler-assisted. Sixteen artifacts pass admission. All
twelve admitted arithmetic artifacts score **1936/1936** held-out, zero
error and no range refusals. Selected first-chain results:

| Task | Direct initial / selected | Compiler initial / selected | Selected compiler hidden weights + readout terms |
| --- | --- | --- | ---: |
| Maximum | invalid / 1936 of 1936 | 1936 / 1936 | 24 + 2 |
| Absolute value | 1936 / 1936 | 1936 / 1936 | 45 + 2 |
| Conditional | 1936 / 1936 | invalid / 1936 of 1936 | 48 + 2 |

The failed direct maximum (case 0) emits nested arrays, prose and a correction
rather than one JSON document: `CnStatus.json`, no artifact. The failed
compiler conditional (20), `[[1,0,2,15,16]]`, puts input `b` in the condition
slot, while step is in the last slot: `CpStatus.type`. The direct controller
(24) prefixes its array with prose and also refuses JSON admission. One
feedback call repairs each first chain. The independent sparse variants all
pass initially. There are no hidden retries.

The [supervised baselines](../fixtures/construction/baselines.csv) use the
same architecture, seeds 254–256, 1000 full-batch AdamW steps, learning rate
`.01`, betas `.9/.99`, epsilon `1e-8`, no decay. Arithmetic inputs/targets
are normalized by four (the bit remains unscaled); exported weights absorb
the scales. Weights freeze before evaluation, without hyperparameter retuning.
The gradient is checked by central differences and export equivalence.

| Baseline, seeds in order 254 / 255 / 256 | Maximum | Absolute value | Conditional |
| --- | --- | --- | --- |
| Trained exact /1936 | 0 / 0 / 0 | 0 / 0 / 0 | 0 / 0 / 0 |
| Trained within `.01` /1936 | 0 / 1222 / 230 | 1566 / 1455 / 1399 | 2 / 491 / 97 |
| Trained MAE | 8.72810 / .56400 / .96468 | .00680 / .00787 / .00878 | 4.87441 / 4.61028 / 4.87841 |
| Hand construction | 1936 exact, MAE 0 | 1936 exact, MAE 0 | 1936 exact, MAE 0 |
| Canonical compiled reference | 1936 exact, MAE 0 | 1936 exact, MAE 0 | 1936 exact, MAE 0 |
| All-zero network | 52 exact, MAE 11.00826 | 52 exact, MAE 11.00826 | 52 exact, MAE 11.00826 |

The exact threshold is demanding for fitted continuous weights; close counts
and errors expose useful approximation behind zero exact scores. These are
three fixed training runs, not a tuned optimizer frontier. Construction
matches known formulas; it does not demonstrate an advantage over native
arithmetic or optimal training.

For a deliberate failure, replace conditional `128` by `64`. This network
passes **176/176 development** but only **1920/1936 held-out**, MAE
`.3904958678`, worst error `64`. At `[64,-64,0]` it returns zero instead of
`-64`. A development winner can still violate the declared domain.

## A controller with an explicit source of information

The observation is four neighboring cells' **true shortest-path distances**,
64 for a blocked action. The policy outputs `-distance`; argmax selects the
lowest action index on a tie. Distances are recomputed from the current map
by exact bounded relaxation. The network implements argmin over already
planned distances. It does not infer maps, discover routes, remember
observations or solve partial observability.

Development maps are A and `S#G/.../...`, 23 reachable non-goal states.
Held-out maps B and C provide 15 and 10 starts, every reachable non-goal
cell, with a 128-step cap. Starts are paired across methods. The neural
baseline uses the same ReLU4 architecture, 64 fixed synthetic distance
vectors in `0..6`, regression targets `-distance`, the same seeds and 1000
AdamW steps. It has seen no maps; blocked value 64 tests extrapolation.

| Policy | B reached / shortest | C reached / shortest | Total steps | Collisions |
| --- | ---: | ---: | ---: | ---: |
| Selected direct, compiled, hand and native argmin | 15 / 15 | 10 / 10 | 122 | 0 |
| Exact planner | 15 / 15 | 10 / 10 | 122 | 0 |
| Trained 254 | 15 / 14 | 3 / 3 | 971 | 0 |
| Trained 255 | 9 / 8 | 4 / 4 | 1573 | 0 |
| Trained 256 | 15 / 13 | 10 / 10 | 128 | 0 |
| Zero logits | 0 / 0 | 0 / 0 | 3200 | 3188 |
| Frozen map-A Q table, each seed | 10 / 10 | 0 / 0 | 1950 | 1898 |
| Q-learning on each held-out map, each seed | 15 / 15 | 10 / 10 | 122 | 0 |

Both admitted direct and both compiled policies reach all 25 starts in
shortest paths. The direct concise attempt first needed format feedback.
Each step executes the saved network: successful constructed policies
require 122 forwards per complete held-out sweep. The
[grid ledger](../fixtures/construction/grid.csv) preserves each map and seed.

Q-learning uses state-only tables, 512 episodes, learning rate 1, discount
`.9`, exploration `4/16`, 128 steps per episode. Frozen A uses
2876/2851/2923 training transitions, reused across maps. Online B uses
3297/3262/3339; online C uses 3972/3965/4023. Their information and
experience differ from distance-informed zero-shot construction; this
comparison does not establish superiority over general reinforcement
learning. Chapter 171's scalar-LQR PPO is not a game-policy baseline here.
Recovery after map change and numerical-tool-assisted proposals remain
unrun in this profile.

## Account for construction and execution

The 19 task calls consume **38 uncached input, 16,545 cache-creation input,
zero cache-read and 3271 output tokens**, reported list cost **$0.197932**,
and 129.549 seconds serial client wall time. Actual payment is not inferred
from list cost. A separate readiness smoke consumes 2 input, 528
cache-creation and 4 output tokens, list cost $0.004312. Failed local setup
exposes no token usage; proposer accelerator time is not exposed. All task
calls finish without tools or spawned subjects; raw responses retain usage
and stop reasons.

Baseline fitting performs **12,000 updates and 1,650,000 sample forward/backward
evaluations**. Twelve saved models prevent retraining during checks.
Preparation wall time was not captured; no speedup over training is claimed.
The proposal batch performs **27,026 network forwards**: 25,924 for the
development/held-out ledger plus 1102 repeated development forwards for
selection. Structural admission, digesting, graph preflights, independent
audits and references are additional work; each full offline replay repeats
those 27,026 forwards without a model call. These are logical counts, not
measured proposer compute. Environment transitions are separate above.

Direct parameter storage is 192 bytes for arithmetic, 448 for control,
before metadata. Selected compiled maximum/absolute/conditional use
192/360/384 hidden-weight bytes plus two readout terms each. Compiled
control has four terms and no hidden-weight bytes. A term carries its slot
and coefficient; excluding readouts miscounts the model. Artifact JSON
also records layer shape and the SHA-256 semantic digest. Its `layers` field
describes gated plans only; the direct architecture always has a ReLU layer.

The [timing ledger](../fixtures/construction/bench.csv) measures 200,000
varying-input calls, three serial trials on the shared M5 Max, milliseconds:

| Path | Maximum median | Absolute median | Conditional median | Argmin median |
| --- | ---: | ---: | ---: | ---: |
| Ordinary Coax function | 0 | 1 | 1 | 1 |
| Admitted ReLU4 core | 5 | 4 | 5 | 8 |
| Canonical compiled weights, checked `gg_run` | 34 | 39 | 56 | 26 |

The clock has one-millisecond resolution: zero is unresolved, not free.
Checksums agree across paths. Timings use canonical references, not the
model-selected conditional's different graph. `gg_run` includes public
checks, while ReLU4 excludes admission. These are different contracts and
shared-host observations, not portable kernel benchmarks. One thousand
distance-preprocessing calls take 19/8/18/13 ms on A/small/B/C. Rollouts
recompute distances per policy sweep; fixed-map distances could be cached.
Planning belongs in the controller's cost even when its network is tiny.

## Run it without another model call

The [public example](../examples/construct-network.coax) prints a worked
maximum, compiler comparison, held-out conditional, planted failure and
complete distance-informed rollout. Its [exact transcript](../fixtures/construction/example.out)
is checked in debug and release.

```shell
coax/bin/coax run psionic/examples/construct-network.coax
nice -n 10 coax/web/tool.sh psionic/tools/check-construction.coax checks
nice -n 10 coax/web/tool.sh psionic/tools/check-construction.coax mutate
nice -n 10 coax/web/tool.sh psionic/tools/check-construction.coax native
nice -n 10 coax/web/tool.sh psionic/tools/check-construction.coax book
```

Run phases separately below fifteen minutes. Mutation uses one worker:
the new admission modules kill all **160 + 137** entry-source mutants.
Native Coax assertions exercise nonfinite inputs/weights in plain and
ASan/UBSan debug/release builds. A structurally valid `a^8` program is
deliberately range-refused before weight execution on large inputs.

[The independent oracle](../tests/construction_oracle.py) derives network
equations, postfix semantics/type checks, identities, BFS distances and
seeded Q-learning separately. It checks all 25 baselines, 24 grid rows,
19 proposal records, failed admission, selection and compiled-weight outputs.
Four offline Coax replays reproduce every CSV byte and verify request
identity, with no inference or training. The
[verification record](../../docs/verification/psionic-ch254-2026-11-02.md)
preserves commands, evidence and limits.

The justified library connections are immutable weight edits, the exact
gate compiler, headless grid and existing AdamW. Arbitrary learned circuit
transplants remain outside chapter 237's exact linker contract. Original
language-model edits, larger weight windows and retained-capability
qualification remain in [255](ch255-edit-an-original-language-model.md).
