# Autopilot documentation

- **Main spec:** [spec.md](./spec.md) — product behavior and constraints
- **Legacy note:** Some runbooks in this folder capture historical `apps/web` + Convex workflows. Treat those as reference-only unless explicitly mapped to current `apps/openagents.com` and `apps/openagents-runtime` flows.

## Structure

| Folder | Contents |
|--------|----------|
| [runbooks/](./runbooks/) | DSE playbook plus legacy self-improvement runbooks |
| [dse/](./dse/) | DSE full spec, RLM roadmap, trace mining |
| [testing/](./testing/) | Prod E2E, stream testing, trace retrieval, fixtures |
| [synergies/](./synergies/) | RLM/Horizons/Monty/Microcode/Typed synergies and learnings (legacy Crest notes archived under `docs/plans/archived/autopilot/`) |
| [reference/](./reference/) | Context failures, telemetry, optimization plan, known issues, design decisions |
| [admin/](./admin/) | Admin trigger and test-user flows |

## Key entry points (also referenced from AGENTS.md)

- [spec.md](./spec.md) — autopilot behavior spec
- [testing/PROD_E2E_TESTING.md](./testing/PROD_E2E_TESTING.md) — prod E2E + request correlation
- [testing/STREAM_TESTING.md](./testing/STREAM_TESTING.md) — stream fixture and contract testing
- [runbooks/DSE_PLAYBOOK.md](./runbooks/DSE_PLAYBOOK.md) — DSE operation and tuning
- [testing/TRACE_RETRIEVAL.md](./testing/TRACE_RETRIEVAL.md) — trace retrieval/debug workflow
- [admin/AUTOPILOT_ADMIN_TEST_USER_TRIGGER.md](./admin/AUTOPILOT_ADMIN_TEST_USER_TRIGGER.md) — admin trigger flow
- [reference/THREAD_STUCK_STREAMING_FIX.md](./reference/THREAD_STUCK_STREAMING_FIX.md) — known issue + mitigation
