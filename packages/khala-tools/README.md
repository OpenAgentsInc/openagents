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

## Built-In Tools

### `read`

`read` reads text files only. It accepts a workspace-relative or explicitly
approved absolute/external path plus optional 1-indexed `offset` and `limit`
arguments. It returns numbered, bounded model output and structured UI line
metadata. Image-like files are not read as text; the tool returns a `view_image`
hint instead. Credential-shaped paths, device files, sockets, pipes, directories,
and denied workspace escapes are blocked before bytes are returned.
