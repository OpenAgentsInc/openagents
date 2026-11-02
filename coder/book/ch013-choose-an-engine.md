# 13. Choose an engine

> **Status: unimplemented — design draft.** The chapter text and its checks aren't written yet. The
> desktop Lab's router in `apps/openagents-desktop/router.coax` already asks
> Mammoth this question and shows the answer; nothing runs on it yet.

Coder picks an engine, a model and an effort for each task. The choice is a
typed decision from System One, not a keyword rule.

## What this chapter covers

- The request: the task and the machine's environment (OS, hardware, which
  agent CLIs are installed and signed in).
- The decision: engine (`micro`, `psionic`, `claude`, `codex`), difficulty,
  effort and model, each with a probability.
- A labelled set split into tuning and held-out halves; labels split engines by
  the job's shape, not by whether it mentions code.
- What happens below a confidence threshold: ask, or use the person's default.
- `--engine` always overrides the decision.

## Dependencies

- [Chapters 9–12](PLAN.md#ii-engines): the engines.
- [Chapter 16](PLAN.md#iii-many-tasks): System One decisions as a tool.

## Done means

- The held-out half routes at a recorded accuracy on a named decision model.
- `openagents coder run "TASK"` with no `--engine` prints the decision and
  runs it.

## Sources

- `apps/openagents-desktop/router.coax` and its labelled set.
