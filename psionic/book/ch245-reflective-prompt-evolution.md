# 245. Reflective prompt evolution

> **Status: unimplemented — design draft.** Nothing here runs yet.

GEPA improves a prompt by asking a model to reflect on failures and propose
better text. This chapter implements GEPA in Coax with the safeguards that the
pre-reset attempts lacked, and measures it against [Chapter 243](ch243-fit-settings-on-recorded-answers.md)'s settings
fitting.

## What this chapter covers

- Candidates whose fields are fixed by the seed candidate. A proposal cannot
  add or remove fields.
- Per-instance Pareto sampling: candidates are sampled by how many validation
  items they are tied-best on.
- A reflector served by Psionic, a separately pinned model such as gpt-oss
  with constrained output. The reflector is never the grader.
- Refusal of a malformed reflection. GEPA as written installs an unmatched
  reply verbatim; this chapter refuses it.
- A drift check that the answer space did not change.
- A canary that proves the candidate text reached the model.
- The NIP-OPT discipline: the student, reflector and grader are separate; the
  proposer sees development data only; candidates may not read answers, change
  the runner, widen tool grants or lower thresholds.

The prior is weak. Ten pre-reset prompt-optimization attempts produced no
measured win. The one run that executed gained +0.025 against a 0.056 noise
floor, which is unverifiable.

## Dependencies

- [Chapter 239](ch239-a-suite-is-three-digests.md), [Chapter 240](ch240-how-far-a-metric-moves-by-itself.md) and [Chapter 241](ch241-gates-that-can-say-unverifiable.md): suites, noise floors and verdicts.
- [Chapter 243](ch243-fit-settings-on-recorded-answers.md): settings fitting, the baseline to beat.
- [Chapter 53](ch053-choosing-the-next-token.md) and [Chapter 64](ch064-execute-gpt-oss-20b.md): constrained output from a served gpt-oss.
- [Chapter 246](ch246-judges-are-components.md): judges as components.

## Done means

- A malformed reflection, a drifted answer space and a missing canary are each
  refused by a fixture.
- On a held-out partition read once, the chapter reports GEPA's gain and
  settings fitting's gain with a verdict from [Chapter 241](ch241-gates-that-can-say-unverifiable.md).
- A result within the noise floor is reported as unverifiable.

## Sources

- [Chapter plan](PLAN.md), Part XXIV.
- [Measure, then improve](../docs/self-improvement.md).
- [NIP-OPT](../../nostr/nips/openagents/NIP-OPT.md).
