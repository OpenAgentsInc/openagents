# The OpenAgents AI SDK

**Home moved.** The Effect-native OpenAgents AI SDK is developed and published
from [OpenAgentsInc/ai](https://github.com/OpenAgentsInc/ai) under Apache-2.0.
Install from npm under dist-tag `rc` (pre-stable never takes `latest`):

```sh
npm install @openagentsinc/ai@rc
# or pin: @openagentsinc/agent-harness-contract@0.1.3-rc.1
```

| Package | Role |
| --- | --- |
| `@openagentsinc/ai` | umbrella re-exports |
| `@openagentsinc/agent-runtime-schema` | L1 vocabulary |
| `@openagentsinc/agent-harness-contract` | L2–L5 (event log, harness, UI stream, ChatTransport, history_recall host-tool wire) |
| `@openagentsinc/ai-model` | L0 model-call bridge |
| `@openagentsinc/history-corpus` | L6 recall + host-tool resolve |
| `@openagentsinc/ai-sdk-sandbox-local` | L3 interop |
| `@openagentsinc/ai-sdk-sandbox-openagents` | L3 interop |

This monorepo consumes those packages from npm (OpenAgentsInc/ai#2). The
`khalaToolsToAiSdkTools` bridge stays monorepo-side in
`@openagentsinc/khala-tools` (`src/ai-sdk-tool-bridge.ts`).

Docs index: https://github.com/OpenAgentsInc/ai/blob/main/docs/README.md
