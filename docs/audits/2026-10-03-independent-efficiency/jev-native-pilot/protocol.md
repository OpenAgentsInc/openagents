# Native coding pilot with Jev span selection

Frozen before scored coding sessions on October 3, 2026. The accompanying plan records exact input, program, source, seed, checker, CLI, configuration, and schedule hashes.

## Question

Does one Jev call to select source declarations improve a lean coding agent beyond deterministic selection? Compare three workflows with Claude Sonnet 5.5 at medium effort:

- **A, bare:** native default prompt and tools within the common isolated environment.
- **B, deterministic:** the retained lean system prompt, six tools (`Bash,Read,Edit,Write,Glob,Grep`), and a deterministic source pack.
- **C, Jev:** the same lean prompt, tools, metadata catalog, and pack renderer as B; one gateway call selects pointers for the task clauses.

C/B is the primary comparison. B/A and C/A compare workflow bundles. This pilot does not isolate the lean prompt from the smaller tool set. The six tools narrow the model-facing API; Bash retains the same operating-system authority. No semantic reviewer decides correctness or requests a repair.

## Tasks and order

Use the already-qualified beta task (#9425, trace integrity) and gamma task (#9424 A12, typed SDK validation). They have different behavior and the two shortest previously measured reference checks: 17.9 and 11.7 seconds. These are exposed development tasks, including in earlier preparation and review experiments. Fresh sessions do not make this a held-out test.

Run serially in this fixed order, without outcome-driven replacements:

1. Beta repetition 1: A, B, C.
2. Gamma repetition 1: C, A, B.
3. Beta repetition 2: B, C, A.
4. Gamma repetition 2: A, B, C.

Each arm gets four attempts. Freeze all 12 UUIDs and configurations before the first attempt. A separate synthetic Sonnet capability probe must succeed first. Its cost and machine setup remain reported separately.

## Common inputs and isolation

Supply the exact public task, full applicable historical instructions, and required public readings to every arm. Gamma's historical selected-answer and score contract governs the task; a selected choice need not be the argmax. Gamma uses `jev/blocking`, never `jev/live`. Scope, features, toolchain, source snapshot, seed, independent checker, and environment are identical across arms for each task.

The executor sees one source snapshot and a private writable home and target. It cannot see reference patches, hidden tests, other attempts, current repository history, credentials, or private conversations. It can reach inference only through the metered provider broker. The bare arm retains native tools and subagents, subject to the same network restrictions. Sonnet and the two documented Haiku 4.5 IDs are admitted; every served identity is retained and checked. No Opus substitution is admitted. All sessions use `--no-session-persistence`.

## Preparation policy

The span catalog admits at most 64 implementation declarations, 16 test declarations, and eight required contract documents. Thirty-two implementation positions are reserved for public entry points; remaining positions use lexical ranking across remaining implementation declarations. Entry points are prioritized over receiver-only getters and conventional constructors using signatures. All rules are generic; no task-specific target symbols enter selection.

The catalog carries names, signatures, paths, immutable source identities, roles, and line ranges. It does not carry reference fixes or hidden-checker hints. One Choice question per task clause includes a `none` option. The native pilot does not ask the optional per-pointer Scores. B selects from the same catalog by lexical clause matching. Both use the same 16 KiB materialization budget, fixed contract allocation, and explicit omissions/partial-span labels. Required common instructions and readings are outside this optional pack. There is no extra probe for C.

Jev runs through `https://ai-gateway.vercel.sh/typesafe/v1/systemone`, model alias `typesafe-ai/jev`. The alias is not a version pin. Retain provider metadata, internal fallback attempts, typed answers, exact request/response, and charged usage. One request per C attempt; no automatic application retry. Gateway preparation has a 90-second outer deadline; an ambiguous charge stops admission.

## Limits and endpoints

Native execution has a 600-second inner deadline, a 1,200-second outer deadline, and a requested $2 CLI budget. Final checks have 240 seconds inside a 260-second outer deadline. The serial driver gives the complete attempt 1,700 seconds and stops admission after an interruption. Each broker has an $8 admission target, 128,000 maximum output tokens per request, and bounded requests/concurrency. A request already admitted can exceed the CLI budget. Stop admitting attempts when known panel inference cost reaches $24; this is a stopping threshold, not a hard invoice cap. Include any overshoot. Also stop on unknown cost, unconfirmed execution closure, failed cleanup, changed identities, or failed capability. No paid retry or replacement is authorized by this plan.

Start each attempt's clock before configuration validation and recurring preparation. Report both elapsed time through durable checks and elapsed time through confirmed scratch cleanup. The latter is primary. Include source validation, catalog creation, Jev, materialization, native setup/execution, candidate capture, provider drain, final checks, evidence retention, and cleanup. Initial index construction, source archive provisioning, seed compilation, tool installation, and orchestration/engineering labor are separate setup costs. Warm caches and prebuilt indexes are an explicit assumption.

## Acceptance and analysis

Primary quality is a captured patch passing scope, formatting, ordinary tests, and the unchanged independent checks, with validated identities and closed execution. A budget-ended patch can pass; report native completion separately. Do not repair an output before checking it. Retain every failure and every charged request.

Report acceptance out of four first. Report per-task means, all matched repetition differences/ratios, and the ratio of the sums of task means for cost and time. Report cost per accepted patch with failed-attempt cost included. Reconcile CLI totals with the broker; count provider requests once, plus Jev, without adding duplicate CLI totals. Prices estimate inference usage, not the owner's subscription invoice or machine/engineering cost.

A directional pilot win requires C to accept all four patches, accept no fewer than either comparator, reduce both aggregate cost and elapsed time by at least 10% versus A and B, and have lower cost and time in at least three of four matched comparisons against each. Report all misses. Two exposed task clusters cannot establish general superiority or repository-wide reliability.

Rates were checked against [Anthropic pricing](https://platform.claude.com/docs/en/about-claude/pricing) and [model IDs](https://platform.claude.com/docs/en/models/overview) on October 3, 2026. USD per million tokens in input/output/five-minute-write/one-hour-write/read order: Sonnet 5.5 `2/10/2.5/4/0.2`; Haiku 4.5 `1/5/1.25/2/0.1`.
