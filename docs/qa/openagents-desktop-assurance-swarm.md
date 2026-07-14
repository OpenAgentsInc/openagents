# OpenAgents Desktop Assurance Swarm

The current QA Swarm Assurance target is OpenAgents Desktop at
`apps/openagents-desktop`. The old Khala clients are migration sources and are
not valid targets for this execution path.

`apps/qa-runner/src/assurance-swarm.ts` is the evidence-only coordinator. A
caller supplies an exact compiled Assurance Manifest and digest, a complete
one-time partition of its units, and adapters backed by the existing QA Runner
browser, terminal, native, performance, monkey, and model substrates. The
coordinator does not perform network, provider, or native work on its own.

The six required lanes are:

1. scripted browser;
2. seeded monkey;
3. LLM explorer;
4. performance;
5. terminal;
6. macOS native.

Every lane has independent action, duration, and model-token caps. Real work,
provider spend, and native control are separately armed. An unsupported,
missing, unarmed, failed, over-budget, or usage-ambiguous adapter produces an
uppercase `INCONCLUSIVE` observation with blockers and no fabricated native
report, artifact commitment, or normalized Assurance Receipt. A configured lane
adapter must match every assigned Manifest unit's locked adapter exactly.
Non-model lanes report exact zero model
usage; an executed LLM lane must report exact observed input plus output tokens
within its cap.

Each Manifest unit with observed adapter output produces the existing normalized Assurance Receipt, bound to
the exact ProductSpec, AssuranceSpec, admission, Manifest, environment, locked
adapter, execution unit, command, source, and native report digests. Its lane
wrapper additionally records the exact artifact commitment, budget, arming,
provider usage, blockers, and receipt digest. These artifacts remain evidence
only and confer no admission, acceptance, release, merge, deploy, or public
promise authority.

The deterministic contract tests use injected adapters and spend no tokens or
native authority:

```sh
bun test apps/qa-runner/src/assurance-swarm.test.ts
```
