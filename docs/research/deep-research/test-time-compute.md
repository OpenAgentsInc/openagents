Test-time compute (TTC) is everything you do *at inference* beyond “one forward pass, one answer”: generating longer reasoning traces, sampling many candidates, running search/MCTS, calling verifiers/tests/tools, doing revision loops, or even iterating extra *internal* compute (latent “thinking”)—all to buy accuracy with latency/cost instead of bigger pretraining. ([ar5iv][1])

## Core papers that most directly study TTC scaling (and why small+many can beat big+one)

### 1) **Compute-optimal TTC can beat parameter scaling**

* **Snell et al. (2024), “Scaling LLM Test-Time Compute Optimally…”**: introduces *prompt-adaptive* “compute-optimal” policies (choose strategy/hyperparams by predicted difficulty) and shows you can nearly match best-of-N with **~4× less TTC** in both PRM-guided search and revision settings; in a FLOPs-matched comparison they report cases where TTC on a smaller model can outperform a **14× larger** model. ([ar5iv][1])
  *Key idea:* TTC isn’t one knob—allocating it *differently per prompt* (easy vs hard) matters more than blindly increasing N or token budget. ([ar5iv][1])

### 2) **Inference scaling laws + new search can make a 7B beat a 34B**

* **Wu et al. (2024), “Inference Scaling Laws…”**: empirically studies “compute-optimal inference” and reports that advanced inference algorithms plus smaller models can give Pareto-optimal tradeoffs; they propose **REBASE** tree search and state that it can be more compute-optimal than sampling/MCTS, and that **Llemma-7B + their tree search can outperform Llemma-34B** across strategies on MATH. ([ar5iv][2])
  *Why this matters for swarms:* with a verifier/search loop, the “effective capability” comes from exploration + selection, not just raw parameters. ([ar5iv][2])

### 3) **Brute-force sampling + verification can scale shockingly far**

* **Brown et al. (2024), “Large Language Monkeys…”**: shows repeated sampling gives large gains and observes log-linear / power-law-like trends over large sample budgets; for **SWE-bench Lite**, they report **15.9% → 56.0%** by sampling **250** solutions (with verification), beating a then-strong single-sample baseline they cite. ([arXiv][3])
  *Takeaway:* if “a correct solution exists in the model’s distribution,” TTC can turn low pass@1 into high pass@k—*especially when you can verify*. ([ar5iv][4])

### 4) **Big 2025 meta-study: no universal TTC strategy, and “reasoning horizon” matters**

* **Agarwal et al. (2025), “The Art of Scaling Test-Time Compute…”**: large-scale comparison (billions of generated tokens) finds (i) **no single TTS strategy dominates**, (ii) reasoning models split into **short-horizon vs long-horizon** trace-quality patterns, and (iii) within a model type, optimal performance scales monotonically with compute budget; they also categorize TTC as parallel/sequential/hybrid/internal and give a “recipe” based on difficulty + model type. ([arXiv][5])

## “More thinking” is not monotonic: overthinking + inverse-scaling results

A big 2025 thread is: **naively increasing reasoning length can hurt**, so you need TTC *control* (early stop, pick shorter traces, abstain, etc.).

* **Zeng et al. (2025), “Revisiting the Test-Time Scaling of o1-like Models…”**: finds longer CoTs in several “o1-like” models don’t consistently improve accuracy; **correct solutions are often shorter than incorrect ones** on the same questions; they argue this relates to self-revision failure and propose **Shortest Majority Vote** (prioritize shorter clusters) and report parallel scaling gives better coverage/scalability than sequential scaling. ([arXiv][6])
* **Hassid et al. (2025), “Don’t Overthink it…”**: reports shorter chains within a question are more likely to be correct (they cite “up to 34.5%” better than the longest chain) and propose **short-m@k** (parallel decode, stop once the first *m* chains finish, vote among those).
* **Gema et al. (2025), “Inverse Scaling in Test-Time Compute”**: constructs tasks where extending reasoning length *degrades* accuracy (explicit “inverse scaling”), spanning distractor counting, spurious regression, constraint tracking, and “advanced AI risks” categories.

So TTC needs *policy*: **how much compute**, **what kind**, **when to stop**, **which trace to trust**.

## Breadth vs depth: search that chooses “go wider” or “go deeper”

* **Misaki et al. (2025), “AB‑MCTS”**: proposes **Adaptive Branching MCTS** that dynamically decides to expand new candidates (“go wider”) or refine existing ones (“go deeper”) *using external feedback* (e.g., tests in coding). They report AB‑MCTS consistently outperforms repeated sampling and standard MCTS on coding/engineering benchmarks under the same compute budget.

This is a crisp formalization of *test-time decomposition*: keep branching until feedback says “this subtree is promising,” then spend depth compute refining it.

## TTC for *agents* (multi-step tool use) is trickier than TTC for one-shot QA

When you decompose a task into many steps, “just do best-of-N at every step” can blow up compute and compound errors.

* **Zhou et al. (2025), “Scaling Test-time Compute for LLM Agents”**: systematic study of TTS inside agent frameworks; explores **parallel sampling**, **sequential revision**, **verifiers/merging**, and **diversifying rollouts**; finds agent performance improves with TTC, but **“knowing when to reflect” matters**, and **list-wise** verify/merge works best among their tested approaches; on GAIA, they show a baseline vs BoN improvement (e.g., average **55.76 → 63.03** for GPT‑4.1 in their setup).

Practical implication: for agent swarms, spend TTC on **trajectory-level** selection/verification (and selective reflection), not uniform stepwise branching everywhere.

## Internal TTC: “thinking” without extra tokens (latent compute)

Most TTC work scales compute by emitting more tokens or more samples. There’s also a line on *internal* compute scaling:

* **Geiping et al. (2025), “Scaling up Test-Time Compute with Latent Reasoning: A Recurrent Depth Approach”**: proposes iterating a recurrent block to “unroll to arbitrary depth at test time,” reasoning in latent space; they scale to **3.5B params / 800B tokens** and report gains “up to a computation load equivalent to **50B parameters**” without bespoke CoT data.

This is conceptually important: TTC doesn’t have to mean “longer CoT,” it can be “more internal steps before producing the next token.”

## Evaluating TTC properly: add confidence + abstention + utility

Classic TTC papers often assume “always answer.” That can mislead, because more compute can increase *response rate* (trying harder on more questions) while changing accuracy among answered questions.

* **Jurayj et al. (2025), “Is That Your Final Answer?”**: evaluates TTC under **selective QA** by extracting confidence during reasoning and thresholding; finds increasing compute can increase both correctness and confidence in correct answers, and proposes reporting performance under non-zero “risk” utility settings (e.g., “Jeopardy odds”).

If you’re building TTC systems, “accuracy vs compute” is incomplete; you want “utility vs compute vs abstain threshold.”

## So… can “many small models + decomposition” exceed one larger model?

Across these works, the answer is “yes, often”—*when TTC is spent in ways that increase the chance of hitting and selecting correct trajectories.*

Patterns that make small+many work well:

* **There’s a verifier or external feedback** (unit tests, constraints, answer checkers): enables repeated sampling / search / MCTS to reliably select better candidates.
* **Compute is allocated adaptively by difficulty** (don’t overspend on easy prompts; use deeper/searchy methods on hard ones).
* **You control overthinking** (prefer shorter traces when they correlate with correctness; stop early; don’t assume longer CoT helps).
* **For agents**, you do TTC at the *trajectory* level and use selective reflection + strong merging, rather than branching at every step.

If you want, I can also condense this into a “design playbook” (budgeting rules: width vs depth vs verification vs early-stop) keyed to task type (math, coding, web agent, planning) using the recipes in Snell+Wu+Agarwal+Zeng.

[1]: https://ar5iv.org/html/2408.03314v1 "[2408.03314] Scaling LLM Test-Time Compute Optimally can be More Effective than Scaling Model Parameters"
[2]: https://ar5iv.org/html/2408.00724v3 "[2408.00724] An Empirical Analysis of Compute-Optimal Inference for Problem-Solving with Language Models"
[3]: https://arxiv.org/abs/2407.21787 "[2407.21787] Large Language Monkeys: Scaling Inference Compute with Repeated Sampling"
[4]: https://ar5iv.org/html/2407.21787v3 "[2407.21787] Large Language MonkeysTitle inspired by https://en.m.wikipedia.org/wiki/Infinite_monkey_theorem.: Scaling Inference Compute with Repeated Sampling"
[5]: https://arxiv.org/html/2512.02008v1 "The Art of Scaling Test-Time Compute for Large Language Models"
[6]: https://arxiv.org/html/2502.12215v1 "Revisiting the Test-Time Scaling of o1-like Models: Do they Truly Possess Test-Time Scaling Capabilities?"
