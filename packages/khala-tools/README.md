# @openagentsinc/khala-tools

Shared native Khala tool runtime contracts.

This package is the provider-neutral layer for Khala Code desktop, Khala CLI,
and later Pylon native-tool fallback execution. It owns typed tool definitions,
registries, invocation/result/event contracts, permission requests, output lanes,
and model-provider adapter helpers.

The local tool runtime does not choose model authority. Hosted OpenAgents cloud
is the default backend, OpenRouter BYOK is opt-in through `OPENROUTER_API_KEY`
or the existing Khala provider-key store, and tests use the mock backend with no
network or spend.

Tool results keep four lanes separate:

- bounded model-visible output
- structured UI payloads
- private local artifact refs
- public-safe summaries

Blueprint policy and Pylon/Probe runtime pieces can materialize scoped tool
menus into this package, but local inspect/coding presets do not require a
Blueprint registry to exist.
