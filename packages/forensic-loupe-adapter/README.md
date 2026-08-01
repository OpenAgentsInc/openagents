# OpenAgents Loupe forensic adapter

`@openagentsinc/forensic-loupe-adapter` compiles structured forensic prompt
artifacts into bounded Loupe-style discovery plans and mediates a pluggable
Linux runtime. It does not provision a worker, mutate a checkout, report a
finding, contact a maintainer, or raise an admitted authority.

The adapter owns four boundaries:

- prompt artifacts are content-digested, recursively immutable, and retain
  parent lineage;
- execution plans inherit source coverage, budgets, network policy, worker
  digests, and tool availability from admitted contracts rather than prompt
  prose;
- only `submit_forensic_finding` and `submit_forensic_hypothesis` payloads can
  create typed output envelopes; diagnostic prose creates neither; and
- execution observes the immutable checkout digest before and after the
  backend call, emits bounded driver events, and keeps reporting
  `manual_no_reporting` with private findings.

Incomplete coverage is allowed for explicitly degraded research runs, but it
remains bound into every output. Missing tools and dependency paths appear in
the compiled prompt and plan instead of being advertised as available.

Run the package checks with:

```sh
pnpm --filter @openagentsinc/forensic-loupe-adapter typecheck
pnpm --filter @openagentsinc/forensic-loupe-adapter test
```
