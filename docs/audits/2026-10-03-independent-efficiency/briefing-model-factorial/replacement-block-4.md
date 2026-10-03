# Prospective replacement of block 4

The original coordinator rejected run 16 because its initialization reported
Claude CLI 2.1.288. The registration requires 2.1.287; every initialization in
runs 1–15 reports that version. The local launcher symlink now points to
2.1.288. The last run's checks passed after repair, but its runtime identity
does not meet the protocol. The [original results](results-original/README.md),
[source review](quality-review-original.md), patches, and costs remain retained.
The original 16 scored runs cost $5.6952538 in CLI estimates; their two shared
warmups add $0.3462816. These are unverified list-price estimates.

The [original protocol](plan.md#four-balanced-blocks) permits a prospectively
registered whole-block replacement after a confirmed infrastructure defect.
This amendment uses that rule. It changes no task, source, briefing, model,
effort, tool, check, repair allowance, cost threshold, or comparison formula.
The separate static concern in original run 16 does not trigger this
replacement and adds no new checker case.

## Fixed replacement

Retain original blocks 1–3 exactly. Replace **all four** positions in block 4,
in their original order:

1. D: Sonnet with the frozen brief.
2. A: Opus control.
3. C: Sonnet control.
4. B: Opus with the same frozen brief.

Run each position once on a fresh export. Finish all four regardless of model
or candidate outcomes. Each keeps the 600-second agent-time bound, $10 CLI
estimate cap, and at most one repair. The additional scored cap is four runs
and $40 in estimates. No extra warmups run. The original shared warmups remain
retained and charged once. Another infrastructure defect requires another
explicit prospective registration; it never licenses silently repeating a
failed candidate.

The helper copies the first 12 runs, original plan and schedule, two warmups,
and two frozen harness source files into a separate study. A full file
manifest binds the copied bytes. Generated Python bytecode is excluded from
that input manifest. Original per-arm registration hashes remain unchanged;
the separate replacement registration and completion receipt establish which
four records are new. Original block 4 remains visible outside the comparison
and in cumulative research expenditure. Reused runs are never charged twice.

## Executable identity

A private regular executable named `claude`, copied from the retained
2.1.287 installation, has mode `0500`, size 227,827,120 bytes, and SHA-256:

```text
6eab8333fe2121553100d8f40bfada384a3e989b94f947e18ba6677a6fcb41ea
```

It reports `2.1.287 (Claude Code)`. The helper places its directory first on
PATH and verifies its type, permissions, digest, and version before and after
each replacement arm. It sets `DISABLE_AUTOUPDATER=1` only in the replacement
process environment, as documented in the
[Claude Code environment-variable reference](https://code.claude.com/docs/en/env-vars).
The owner's global launcher and settings remain untouched. Existing session
initialization and served-model checks still apply.

The earlier runs bind the observed CLI version, not executable bytes. Hashing
the retained binary now does not retroactively prove the bytes executed in
those runs. The additional binding prevents the observed launcher drift in
the replacement block.

## Reporting

The registered endpoint formula remains unchanged. New wrapper setup and
pre/post executable-check time is recorded separately, outside that timer.
It must not be reported as zero or included silently in a different formula.
The source export, agent phase, external checks, and executor shutdown remain
in the existing endpoint; final artifact capture and deletion remain outside.

The effective comparison uses original blocks 1–3 plus the complete new
block 4. It requires the replacement completion receipt and independent
lineage verification in addition to the original model, input, cost, and
acceptance checks. Publish all original and replacement costs. Apply the
same five planned comparisons and thresholds once, without pooling outcomes
from the discarded block or adding samples until a desired result appears.

The replacement registration and helper must be committed and pushed before
any replacement task call. The registration binds this amendment's digest.
