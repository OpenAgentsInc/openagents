# Coverage and Test Additions

Current Strengths
- Extensive fixtures for initialize and `session/update` variants
- Bridge server/client integration tests
- Translator tests for Codex/Claude → ACP events
- Orchestration unit tests (FM tools, planning reducer, scheduler, etc.)

Gaps to Address
- Coordinator path lacks RPC‑level integration tests (no public JSON‑RPC yet).
- SessionUpdateHub metrics/persistence error path isn’t directly tested.
- Some legacy tests reference older `ACP.Client` shapes; align with current modeling.

Additions
- Coordinator RPC E2E: call `coordinator.run_once`, then assert plan/tool updates streamed; assert provider selection matches config preferences.
- Scheduler bind/run_now: verify next wake and that trigger runs through coordinator.
- SessionUpdateHub: simulate Tinyvex unavailable and assert metrics/logging.

