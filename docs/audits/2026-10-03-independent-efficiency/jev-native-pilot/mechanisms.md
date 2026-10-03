# Native pilot: observed mechanisms

Descriptive analysis of the completed original 12-run panel. No pending clause-review outputs, independent checker code, or reference patches were read. Exact recomputed values and source digests are in `mechanisms.json`. A=bare, B=lean deterministic pack, C=lean Jev-selected pack.

## Main finding

C/A is a bundle comparison: lean system prompt, six model-facing tools, and selected source replace the default native prompt/tools. C/B isolates selection within the same lean workflow more closely. All arms passed the primary gate on both beta attempts and failed the independent gate on both gamma attempts; all 12 passed scope, format, and ordinary tests. All sessions completed. Lower spend therefore did not repair the shared gamma quality failure. Passing the primary gate is not exhaustive correctness.

C/A cost fell **35.1522%** and endpoint time **24.3679%**. C/B fell only **8.7847%** and **9.2799%**; these missed the registered 10% thresholds. C was lower in both cost/time in 4/4 matched pairs versus A and 3/4 versus B. Its 2/4 acceptance also missed the all-four quality gate. These are two exposed development tasks with two repetitions each, not a causal or general superiority estimate.

## Time and accounting

| Across four runs | A | B | C |
|---|---:|---:|---:|
| Total cost, USD | 2.082077 | 1.480213 | 1.350181 |
| Mean endpoint, s | 243.356699 | 202.883057 | 184.055801 |
| Mean executor phase, s | 176.446114 | 144.180146 | 124.392342 |
| Mean preparation, s | 0.333882 | 0.549202 | 1.663291 |
| Mean native wrapper, s | 186.589412 | 150.572544 | 130.884123 |
| Mean acceptance, s | 54.778005 | 50.926994 | 50.719084 |

C saved 19.688421 native seconds per attempt versus B, while adding 1.114089 preparation seconds. Acceptance differed by only 0.207910 seconds per attempt. Its end-to-end reduction was 18.827256 seconds. The native phase includes setup, execution and capture; the separately retained executor phase fell 19.787804 seconds. None of these intervals is pure model inference time.

Four Jev calls cost **$0.004973052** in total. Using the frozen Sonnet token prices, the native C/B cost difference decomposes into $0.040148 fewer one-hour cache writes, $0.0520988 fewer cache reads, $0.04273 less output-token cost, and $0.000028 fewer uncached input tokens, offset by that Jev charge. This exactly accounts for the $0.130031748 net reduction. Prices are usage estimates, not subscription invoices; setup, machine and engineering cost remain excluded.

## Context and visible work

| Four-run total | A | B | C |
|---|---:|---:|---:|
| Inference requests | 50 | 45 | 38 |
| Additional count-token requests | 1 | 0 | 1 |
| Unique assistant message IDs | 50 | 45 | 38 |
| Cache-created input tokens | 231,607 | 185,940 | 175,903 |
| Cache-read input tokens | 2,981,745 | 1,654,414 | 1,393,920 |
| Output tokens | 55,910 | 40,539 | 36,266 |
| Mean first-request input context | 36,483.00 | 24,759.25 | 24,581.50 |
| Mean final-request input context | 78,114.75 | 50,077.25 | 48,764.75 |

The report’s provider-call totals 51/45/39 include one non-inference `count_tokens` request in A and one in C. Actual message-generation requests are 50/45/38. Every served generation model was `claude-sonnet-5-5`; no delegation tool or visible child activity appears. Cache-created totals are also labeled one-hour writes: do not add both fields. Input context here means uncached + cache-created + cache-read tokens for one provider request; cumulative cache reads repeatedly count earlier context and are not unique material read.

A starts with roughly 36.5k input-context tokens, versus 24.8k B and 24.6k C, even though its supplied user prompt is shorter (beta 22,266 bytes versus 38,472 B / 38,311 C; gamma 30,177 versus 46,283 B / 46,405 C). That is consistent with the registered lean system/tool bundle lowering native overhead; this panel does not isolate which component caused it. B/C initial contexts are close, so their difference is chiefly later trajectory work, not a materially smaller starting context.

Visible root tools total 52/42/41: A 45 Bash + 7 Read; B 42 Bash; C 32 Bash + 7 Read + 2 Grep. The conservative parser counts shell read actions 65/76/49 and searches 36/41/23, with 17/13/11 Bash calls unclassified. It sees cargo-test actions 9/9/4, but these are incomplete lower observations, not test executions or test-pass counts. For example C beta1 has two unclassified multiline Bash calls containing four literal Cargo mentions, yet the conservative summary shows no Cargo action. One Bash can combine discovery, editing, and several conditional tests. Zero exact repeated Bash payloads were observed; this cannot establish absence of repeated work.

## Beta versus gamma

| Task | C/B cost change | C/B endpoint change | A/B/C inference requests | A/B/C mean executor seconds |
|---|---:|---:|---|---|
| beta | -23.7403% | -7.9619% | 22/21/13 | 155.841/128.406/112.458 |
| gamma | +3.6389% | -10.4631% | 28/24/25 | 197.051/159.954/136.327 |

Beta C used 13 generation calls across its two runs, versus 21 B; output tokens fell 19,520→14,665 and cache reads 697,842→361,030. Gamma C used 25 versus 24 B, with output 21,019→21,601 and cache reads 956,572→1,032,890. Thus gamma erased part of beta’s incremental cost gain. Gamma C1 used 8 calls and 119.996281 executor seconds; C2 used 17 calls and 152.657064 seconds despite identical delivered brief bytes. C2 also had more visible discovery/verification actions. That within-arm spread and service/tool timing prevent attribution of the aggregate time difference to selection alone.

## Delivered span evidence

B and C used the same task-specific catalogs and renderer. Their pack bytes were identical across repetitions within each task/arm. These are complete declaration line ranges with adjacent attributes/docs, not exact AST byte slices. Each pack also included explicitly labeled document excerpts; full required contracts and instructions were separately supplied in the common prompt.

| Pack | Payload bytes | Implementation source bytes | Test source bytes | Document source bytes |
|---|---:|---:|---:|---:|
| beta deterministic | 16204 | 3690 | 5418 | 2709 |
| beta jev | 16043 | 5350 | 4423 | 2709 |
| gamma deterministic | 16104 | 483 | 9796 | 2390 |
| gamma jev | 16226 | 3291 | 7430 | 2390 |

- **Beta:** C added complete `atif::log::read` (`crates/atif/src/log.rs:225–306`) and `Task::judge_record` (`crates/coderbench/src/lib.rs:1267–1290`) to the delivered material. B omitted both for byte budget. Both include `Log::append` (`log.rs:144–155`). Neither delivers `Log::finish`: it is present in the catalog but excluded by the 24-source-pointer materialization limit. C therefore improves relevant implementation coverage without becoming a complete lifecycle brief. Native C beta1 still explicitly reads `log.rs` twice and `coderbench/src/lib.rs` five times.

- **Gamma:** C delivers `SystemOneResponse::decode` (`answers.rs:169–198`) and `decode_answer` (`answers.rs:303–347`); B’s only implementation declaration is unrelated `Models::list_raw`. However C still spends 7,430 of 13,111 source bytes on tests and includes no async/blocking client caller bodies. `Client::system_one`, `BlockingClient::system_one`, and `Client::system_one_raw` are in the catalog but excluded by the pointer limit. Both arms also carry an existing live-test body as source evidence; selection of that body is not evidence it was executed. The request-validation boundary thus remains absent from the delivered C pack even though decoder coverage improves.

Both observations support a narrower diagnosis: selection can improve the seed context, but it does not enforce caller/requirement closure. This is a plausible explanation to test, not proof that an omitted span caused any specific failed patch. The analysis did not inspect hidden checker details or pending advisory reviews.

## Evidence map and limits

The `runs/` paths below are inside [the retained native archive](native-evidence.tgz).

- `report.json`: registered costs, endpoints, phases, gate outcomes and matched comparisons.
- `traces.json`: deduplicated visible assistant/tool counts and conservative shell categories.
- `runs/<run-id>/native/provider-calls.jsonl`: counted finished request endpoints, exact reported usage and per-request input context.
- `runs/<run-id>/preparation/{pack,catalog,preparation}.json` and `briefing.md`: delivered ranges, omission reasons, repeated pack equality and preparation cost.
- `mechanisms.json`: 12 identity-bound rows and recomputed aggregates for every table above.

Summed provider-request wall times are neither GPU time nor total tool time; no causal latency allocation was inferred by subtracting them. The serial schedule is not a fresh-cache experiment. First-call cache hits and the unusually slow first native setup are retained, not normalized away. Every raw outcome stays in the denominator. Neither fewer tools nor lower spend establishes less wasted work, and the observed 50% acceptance leaves the registered success claim false.
