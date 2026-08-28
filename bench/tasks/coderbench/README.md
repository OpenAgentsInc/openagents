# CoderBench tasks

Two directories, two statuses:

- `drafts/` — distiller output (`gym dataset distill`). Candidates. Never
  auto-promoted. An agent does not mark these gradeable.
- `smoke/` — Harbor-registry pins the first CoderBench suite can actually
  run. They are Terminal-Bench 2.0 tasks, used only so `gym run` has a
  gradeable environment before agent-building images exist. They are not
  promoted distiller drafts and they are not a claim that Terminal-Bench is
  the agent-building domain.

`bench/suites/coderbench-agent-building-v1.suite.json` pins the smoke
directory, tier `smoke`. Tracker-closed-issue drafts stay out of that
manifest until a container can grade them; `gym run` supports
harbor-registry tasks only.
