# Microcoder configuration sweep for Round 3

September 26, 2026. Tracking:
[#9683](https://github.com/OpenAgentsInc/openagents/issues/9683), the
[out-of-sample study](2026-09-26-out-of-sample-study.md).

This sweep asks which settings of Microcoder's existing flags give the most
passes per dollar, so that Round 3 can be declared with a configuration that
was chosen on development tasks only. It changed no code.

## Rules followed

- **Tasks.** Only the 14 excluded development tasks. The sweep ran 6 of
  them: `fin-saccr-rwa`, `gsea-proteomics`, `sound-change-cascade`,
  `hof-topology-interpenetration`, `batched-eval-parity`, and
  `risk-scorer-replay`. No held-out or Fable-fails task was run or read,
  and `~/study-oos` wasn't read.
- **Binary.** `~/.local/bin/microcoder-sweep` on coderos-4080, a copy of
  `microcoder-study-r2` (Microcoder `e799ac020d`, SHA-256
  `7368e5a4928f496c0428ed97680c631f24c7a231c7458e1f098c8800db06e10f`),
  the build Round 2 uses.
- **Base arguments.** Every run started from Round 2's arguments:
  `--kb candidates --max-steps 60 --max-minutes 60 --max-usd 1.00
  --provider openrouter --model openai/gpt-6-luna`, with
  `OPENAGENTS_KNOWLEDGE=~/.openagents/knowledge/empty-local`. An arm appends
  its flags, and a later flag overrides an earlier one.
- **Load.** At most 3 of the sweep's runs at a time, and a run started only
  while Docker had room for at least 6 more networks (see
  [Operational notes](#operational-notes)).
- **Records.** Each run directory was moved from
  `~/.openagents/microcoder/runs/` to
  `~/sweep-runs/<arm>__<task>__<rep>__<run>/` as soon as the run ended, so
  evidence scans and the study don't pick them up. Logs are in
  `~/sweep-logs/`, and the table below is `~/sweep-results.tsv`.

## Arms

| Arm | Flags added to the base | What it tests |
| --- | --- | --- |
| A | none | Round 2's configuration |
| N | `--no-acceptance` | no frozen acceptance tests |
| H | `--effort high` | Luna at high effort |
| R | `--route always --strong-model openai/gpt-6-sol` | Sol writes the acceptance tests |
| S | `--max-steps 120` (runs after the first three also `--max-minutes 90 --max-usd 2.00`) | a higher step cap |
| T | `--max-steps 200 --max-minutes 120 --max-usd 2.00` | a much higher step cap |
| SOL | `--model openai/gpt-6-sol --max-steps 200 --max-minutes 120 --max-usd 3.00` | Sol as the main model |
| HT | `--effort high` plus T's flags | high effort with 200 steps |
| CT | `--provider codex --model gpt-6-luna` plus T's flags | T through the Codex login |
| CGC | CT's flags plus `--gate-credible` | the model's own doubt as a gate |

The first pass ran A, N, H, R, and S on 6 tasks. Mid-sweep, the
coordinator moved the budget to step caps (Round 2's held-out runs mostly
end at the 60-step limit), then added Sol and Astra, then asked to stop
OpenRouter runs when its credit ran out (HTTP 402) and to finish on the
Codex login. `--kb on` versus `--kb candidates`, `--effort low`, and a
separate `--max-minutes` arm were dropped for time. No run ended by its time
limit, so the time cap never bound.

## Results by arm

Provider faults (18 runs that ended on OpenRouter's HTTP 402) are left out
of the pass rates and listed in the run table. Costs are billed OpenRouter
dollars, except CT and CGC, which are list-price estimates on the Codex
login; some of their calls reported no cost, so those totals are lower
bounds.

| Arm | Valid runs | Passes | Pass rate | Cost per run | Cost per pass | Fails with every frozen test green |
| --- | --- | --- | --- | --- | --- | --- |
| A (Round 2) | 9 | 1 | 11% | $0.140 | $1.26 | 4 |
| N (`--no-acceptance`) | 3 | 2 | 67% | $0.163 | $0.24 | — (no frozen tests) |
| H (`--effort high`) | 3 | 1 | 33% | $0.145 | $0.43 | 1 |
| R (Sol writes tests) | 3 | 2 | 67% | $0.154 | $0.23 | 1 |
| S (120 steps) | 5 | 1 | 20% | $0.143 | $0.72 | 4 |
| T (200 steps) | 5 | 0 | 0% | $0.303 | — | 3 |
| SOL (Sol, 200 steps) | 1 | 0 | 0% | $3.042 | — | 0 |
| HT (high, 200 steps) | 0 | — | — | — | — | — |
| CT (Codex, 200 steps) | 4 | 0 | 0% | ≥ $0.031 | — | 4 |
| CGC (Codex, 200 steps, `--gate-credible`) | 2 | 0 | 0% | ≥ $0.038 | — | 2 |

The arms didn't all run on the same tasks, so the pooled rates above mix
easy and hard tasks. The fair comparison is task by task:

| Task | A | N | H | R | S | T | Other |
| --- | --- | --- | --- | --- | --- | --- | --- |
| `fin-saccr-rwa` | 0/1 | 1/1 | 1/1 | 1/1 | 1/1 | fault | CT 0/2, CGC 0/1 |
| `gsea-proteomics` | 1/1 | 1/1 | 0/1 | 1/1 | 0/1 | fault | CT 0/2, CGC 0/1 |
| `sound-change-cascade` | 0/2 | 0/1 | 0/1 | 0/1 | 0/1 | 0/1 | SOL 0/1 ($3.04, spend limit) |
| `hof-topology-interpenetration` | 0/1 | — | — | — | 0/1 | 0/1 | |
| `batched-eval-parity` | 0/2 | — | — | — | 0/1 | 0/1 | |
| `risk-scorer-replay` | 0/2 | — | — | — | — | 0/2 | |

## What the runs show

1. **The step cap isn't what fails these development tasks.** A hits
   the 60-step limit on `hof-topology-interpenetration`,
   `batched-eval-parity`, and `risk-scorer-replay`. With 200 steps (T), the
   first two ended on their own at 50 and 59 steps with every frozen test
   green, and still failed the grader (24 of 38 and 3 of 5 grader tests
   failed). `risk-scorer-replay` used all 200 steps once ($0.79) and ended
   on its own at 19 steps once; both failed. No task that failed at 60 steps
   passed at 120 or 200.
2. **The main failure is a wrong solution that passes its own tests.** In
   20 of the 26 failed Luna runs that had frozen tests, the tests were all
   green when the run ended. On `sound-change-cascade`, a T run's frozen
   tests passed while the grader found 156 of 168 hidden pairs wrong. More
   steps, higher effort, and Sol-written tests (R) didn't change that on
   `sound-change-cascade`, the one task every arm ran.
3. **Cost is not the constraint.** A Luna run costs $0.03–$0.30, and the
   worst 200-step run cost $0.79. Fable 5.1 low's cheapest winning runs on
   held-out tasks mostly cost $2–$38, so a cap of 200 steps and $2.00 still
   leaves every Luna pass far below Fable's bar.
4. **Sol as the main model is too expensive for this loop.** Sol's steps
   cost about $0.04 each on `sound-change-cascade` (about 14,000 input
   tokens a step at $2 per million), so its one valid run hit the $3.00
   limit at step 71 and failed. The other Sol runs ended on HTTP 402. Astra
   wasn't run: at OpenRouter's $10 and $50 per million, the same prompts
   cost about $0.15–$0.20 a step, so a $3.00 cap ends a run in 15–20 steps,
   and a run can't stay under $2 unless it passes in about 12 steps. Both
   models' OpenRouter prices ($2/$10 and $10/$50 per million input and
   output tokens, cached input $0.20 and $1.00) match
   `crates/microluna/src/price.rs`.
5. **`--gate-credible` has no evidence here.** Its two runs were on the
   Codex route with the newer knowledge snapshot (see caveat 3) and both
   failed with green frozen tests.

## Recommendation for Round 3

Keep Round 2's loop settings, and raise the caps. On cost-headroom grounds,
not development evidence:

```sh
OPENAGENTS_KNOWLEDGE=~/.openagents/knowledge/empty-local \
microcoder <task> \
  --provider openrouter --model openai/gpt-6-luna --effort medium \
  --kb candidates \
  --max-steps 200 --max-minutes 120 --max-usd 2.00
```

- **`--max-steps 200 --max-usd 2.00`.** On the development tasks this
  didn't add a pass, but it didn't cost one either. On held-out tasks,
  where most Round 2 runs end at the step limit, it's the only change that
  can let a run that's still making progress finish, and even a runaway run
  stays under Fable's cheapest winning run on nearly every held-out task.
  The $2.00 cap bounds the rare run that uses all 200 steps.
- **`--max-minutes 120`.** No sweep run was stopped by time; 200 Luna steps
  took up to 40 minutes. Keep the study's rule of twice Fable's fastest
  winning time where that is longer, up to 120 minutes.
- **Keep medium effort.** On the three tasks both ran, high effort cost
  about twice as much per run ($0.145 against $0.077) and passed no more.
- **Keep acceptance tests on, and `--route never`.** N and R each passed 2
  of 3, against A's 1 of 3 on the same three tasks. That difference is one
  run on `fin-saccr-rwa`, and neither changed the frozen-green failure on
  `sound-change-cascade`. It isn't enough evidence to change the loop
  between rounds. R costs about twice A per run.
- **Keep Luna as the model.** Sol at $3.00 ran out of budget before it
  finished, and Astra can't fit under $2.00.
- **Keep `--kb candidates`.** `--kb on` wasn't tested.
- **Don't add `--gate-credible` yet.** It has 2 runs here, both confounded.

## Caveats

1. **The samples are tiny.** Most arm-task cells hold 1 run, and the
   largest arm has 9 valid runs. A difference of one pass is noise. None of
   the pass rates above supports a claim about held-out tasks.
2. **The development tasks aren't a fair sample.** Most Round 2 knowledge
   entries were written from these tasks, so knowledge-on runs here are
   in-sample, and Luna fails the four hardest ones under every arm, so they
   can't tell arms apart.
3. **The knowledge snapshot changed mid-sweep.** The relay grew from 104
   entries (Round 2's snapshot) to 154 during the sweep (the
   [Round 3 entries](2026-09-26-round3-knowledge.md)). All 29 valid
   OpenRouter runs saw 104 entries. Every run that saw 154 was either an HTTP
   402 fault or a Codex-route run. The 6 Codex runs on `fin-saccr-rwa` and
   `gsea-proteomics` all failed, while Luna through OpenRouter with 104
   entries passed 7 of 10 on those two tasks. The sweep can't separate the
   route from the new entries (the Codex runs showed new entries such as
   `numerics.decimal-currency-rounding` in full on `fin-saccr-rwa`), and 6
   runs could still be chance. **Before Round 3 declares the 154-entry
   snapshot, run a few 154-entry runs through OpenRouter on
   `fin-saccr-rwa` and `gsea-proteomics`.**
4. **Arms after the first pass didn't get 2 runs per task.** OpenRouter's
   credit ran out (HTTP 402) while T, HT, SOL, and the second A runs were
   queued, so HT has no valid run and SOL has one.

## Operational notes

- **Docker's address pools were full.** The first launch failed on every
  run with `all predefined address pools have been fully subnetted`: the
  default pools hold 30 custom networks, and 17 were Harbor networks from
  September 24–25. The sweep removed the 8 of those that had no container
  attached and no process referring to them (`docker network rm` refuses a
  network in use). It didn't touch the 9 with running containers. From then
  on, a sweep run started only while at most 27 networks existed. Every run
  now needs two networks at grading time (all 14 development tasks use a
  separate verifier), so Round 2 at 8 runs at a time can fill the pools on
  its own. Prune stale networks, or widen `default-address-pools` in the
  Docker daemon configuration, before Round 3 raises the load.
- **The first launch's runs** (60 that never started, one that ran about
  40 seconds) are in `~/sweep-runs/aborted-first-launch/` and aren't
  reported.
- **Three runs were killed from outside at about 15:20 CDT** (A on
  `hof-topology-interpenetration`, T on `sound-change-cascade`, and T on
  `hof-topology-interpenetration`, which exited with code 101). Their
  partial records are in `~/sweep-runs/killed-1520/`. They were queued
  again, and the reruns ended on HTTP 402.
- **One run leaked past the concurrency cap.** Restarting the first queue
  left the N run on `fin-saccr-rwa` running unsupervised, so for about 14
  minutes 4 sweep runs ran at once. Its record was moved by hand.

## Spend

| Item | Cost |
| --- | --- |
| OpenRouter, billed, reported runs | $8.02 |
| Codex login, list price, reported runs (lower bound) | $0.20 |
| Killed and aborted runs (estimated from their events) | about $0.20 |
| **Total** | **about $8.40** |

The budget was about $25, raised to about $60 for the Sol and Astra arms.
Almost all of it went unspent because OpenRouter's credit ran out.

## Every run

Run directories are under `~/sweep-runs/` on coderos-4080, prefixed with
`<arm>__<task>__<rep>__`. "Fault (402)" is a provider fault: OpenRouter
returned HTTP 402 and the run ended after three unusable replies.

| Arm | Task | Rep | Run directory | Reward | Ended by | Steps | Min | Cost (USD) | Frozen tests green |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| A | `batched-eval-parity` | 1 | `batched-eval-parity-1790452059996` | fail | `step_limit` | 60 | 10.7 | 0.178 | yes |
| A | `batched-eval-parity` | 2 | `batched-eval-parity-1790454042702` | fail | `finished` | 39 | 10.6 | 0.119 | yes |
| A | `fin-saccr-rwa` | 1 | `fin-saccr-rwa-1790446701655` | fail | `finished` | 10 | 4.2 | 0.040 | yes |
| A | `fin-saccr-rwa` | 2b | `fin-saccr-rwa-1790457214750` | fault (402) | `bad_replies` | 3 | 0 | 0.001 | — |
| A | `gsea-proteomics` | 1 | `gsea-proteomics-1790447901207` | pass | `finished` | 25 | 4.5 | 0.061 | yes |
| A | `gsea-proteomics` | 2b | `gsea-proteomics-1790457575134` | fault (402) | `bad_replies` | 3 | 0 | 0.000 | — |
| A | `hof-topology-interpenetration` | 1 | `hof-topology-interpenetration-1790451429463` | fail | `step_limit` | 60 | 17.9 | 0.243 | no |
| A | `hof-topology-interpenetration` | 2 | `hof-topology-interpenetration-1790457079600` | fault (402) | `bad_replies` | 3 | 0 | 0.000 | — |
| A | `risk-scorer-replay` | 1 | `risk-scorer-replay-1790452818342` | fail | `step_limit` | 60 | 10.6 | 0.177 | no |
| A | `risk-scorer-replay` | 2 | `risk-scorer-replay-1790454795085` | fail | `step_limit` | 60 | 10.5 | 0.177 | no |
| A | `sound-change-cascade` | 1 | `sound-change-cascade-1790448585407` | fail | `tests_held` | 29 | 9 | 0.131 | yes |
| A | `sound-change-cascade` | 2 | `sound-change-cascade-1790453480857` | fail | `tests_held` | 24 | 6 | 0.131 | yes |
| N | `fin-saccr-rwa` | 1 | `fin-saccr-rwa-1790446791726` | pass | `step_limit` | 60 | 13.6 | 0.224 | — |
| N | `gsea-proteomics` | 1 | `gsea-proteomics-1790447856176` | pass | `finished` | 12 | 2.4 | 0.027 | — |
| N | `sound-change-cascade` | 1 | `sound-change-cascade-1790448480234` | fail | `step_limit` | 60 | 19.9 | 0.237 | — |
| H | `fin-saccr-rwa` | 1 | `fin-saccr-rwa-1790446746670` | pass | `tests_held` | 26 | 18.6 | 0.119 | yes |
| H | `gsea-proteomics` | 1 | `gsea-proteomics-1790448032023` | fail | `finished` | 19 | 5.3 | 0.069 | yes |
| H | `sound-change-cascade` | 1 | `sound-change-cascade-1790450594994` | fail | `step_limit` | 60 | 22.5 | 0.246 | no |
| R | `fin-saccr-rwa` | 1 | `fin-saccr-rwa-1790446992516` | pass | `tests_held` | 27 | 13.9 | 0.174 | yes |
| R | `gsea-proteomics` | 1 | `gsea-proteomics-1790448201300` | pass | `finished` | 20 | 3.7 | 0.142 | yes |
| R | `sound-change-cascade` | 1 | `sound-change-cascade-1790450820338` | fail | `tests_held` | 24 | 6.4 | 0.147 | yes |
| S | `batched-eval-parity` | 1 | `batched-eval-parity-1790455554571` | fail | `finished` | 62 | 16.1 | 0.222 | yes |
| S | `fin-saccr-rwa` | 1 | `fin-saccr-rwa-1790447668469` | pass | `tests_held` | 39 | 10.4 | 0.164 | yes |
| S | `gsea-proteomics` | 1 | `gsea-proteomics-1790449111244` | fail | `finished` | 19 | 4.3 | 0.060 | yes |
| S | `hof-topology-interpenetration` | 1 | `hof-topology-interpenetration-1790455454317` | fail | `tests_held` | 60 | 22 | 0.241 | yes |
| S | `sound-change-cascade` | 1 | `sound-change-cascade-1790449706416` | fail | `finished` | 8 | 2.2 | 0.029 | yes |
| T | `batched-eval-parity` | 1 | `batched-eval-parity-1790452538070` | fail | `finished` | 59 | 13.7 | 0.190 | yes |
| T | `batched-eval-parity` | 2 | `batched-eval-parity-1790457124645` | fault (402) | `bad_replies` | 3 | 0 | 0.000 | — |
| T | `fin-saccr-rwa` | 1 | `fin-saccr-rwa-1790456877634` | fault (402) | `bad_replies` | 7 | 1.6 | 0.014 | no |
| T | `gsea-proteomics` | 1 | `gsea-proteomics-1790457440001` | fault (402) | `bad_replies` | 3 | 0 | 0.000 | — |
| T | `hof-topology-interpenetration` | 1 | `hof-topology-interpenetration-1790451354367` | fail | `tests_held` | 50 | 11.1 | 0.203 | yes |
| T | `hof-topology-interpenetration` | 2 | `hof-topology-interpenetration-1790457349897` | fault (402) | `bad_replies` | 3 | 0 | 0.000 | — |
| T | `risk-scorer-replay` | 1 | `risk-scorer-replay-1790452934552` | fail | `step_limit` | 200 | 39.8 | 0.790 | no |
| T | `risk-scorer-replay` | 2 | `risk-scorer-replay-1790455352506` | fail | `tests_held` | 19 | 2.9 | 0.050 | yes |
| T | `sound-change-cascade` | 1 | `sound-change-cascade-1790452105016` | fail | `unaccepted` | 49 | 13.3 | 0.282 | no |
| T | `sound-change-cascade` | 2 | `sound-change-cascade-1790457485054` | fault (402) | `bad_replies` | 3 | 0 | 0.000 | — |
| SOL | `batched-eval-parity` | 1 | `batched-eval-parity-1790457394956` | fault (402) | `bad_replies` | 3 | 0 | 0.000 | — |
| SOL | `hof-topology-interpenetration` | 1 | `hof-topology-interpenetration-1790457169700` | fault (402) | `bad_replies` | 3 | 0 | 0.000 | — |
| SOL | `risk-scorer-replay` | 1 | `risk-scorer-replay-1790457530090` | fault (402) | `bad_replies` | 3 | 0 | 0.000 | — |
| SOL | `sound-change-cascade` | 1 | `sound-change-cascade-1790454087742` | fail | `spend_limit` | 71 | 41.3 | 3.042 | no |
| SOL | `sound-change-cascade` | 2 | `sound-change-cascade-1790457620183` | fault (402) | `bad_replies` | 3 | 0 | 0.000 | — |
| HT | `batched-eval-parity` | 1 | `batched-eval-parity-1790456989500` | fault (402) | `bad_replies` | 3 | 0 | 0.000 | — |
| HT | `batched-eval-parity` | 2 | `batched-eval-parity-1790457304832` | fault (402) | `bad_replies` | 3 | 0 | 0.000 | — |
| HT | `hof-topology-interpenetration` | 1 | `hof-topology-interpenetration-1790456802512` | fault (402) | `bad_replies` | 11 | 2.9 | 0.033 | no |
| HT | `hof-topology-interpenetration` | 2 | `hof-topology-interpenetration-1790457259803` | fault (402) | `bad_replies` | 3 | 0 | 0.000 | — |
| HT | `sound-change-cascade` | 1 | `sound-change-cascade-1790456633838` | fault (402) | `bad_replies` | 15 | 5.6 | 0.049 | no |
| HT | `sound-change-cascade` | 2 | `sound-change-cascade-1790457034544` | fault (402) | `bad_replies` | 3 | 0 | 0.000 | — |
| CT | `fin-saccr-rwa` | 1 | `fin-saccr-rwa-1790457665247` | fail | `finished` | 27 | 6.4 | 0.064 | yes |
| CT | `fin-saccr-rwa` | 2 | `fin-saccr-rwa-1790458077728` | fail | `tests_held` | 10 | 3.4 | 0.014 (lower bound) | yes |
| CT | `gsea-proteomics` | 1 | `gsea-proteomics-1790457755351` | fail | `finished` | 22 | 5.4 | 0.033 (lower bound) | yes |
| CT | `gsea-proteomics` | 2 | `gsea-proteomics-1790458122750` | fail | `finished` | 12 | 5.8 | 0.012 (lower bound) | yes |
| CGC | `fin-saccr-rwa` | 1 | `fin-saccr-rwa-1790457710282` | fail | `tests_held` | 16 | 8.5 | 0.024 (lower bound) | yes |
| CGC | `gsea-proteomics` | 1 | `gsea-proteomics-1790458255770` | fail | `finished` | 42 | 9.1 | 0.052 (lower bound) | yes |
