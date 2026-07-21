# Packages

The SDK publishes from
[OpenAgentsInc/ai](https://github.com/OpenAgentsInc/ai) as one `rc` train on
npm. This page lists each package, its key exports, and when to use it.
Every export named here exists in the published `rc` packages.

## @openagentsinc/ai

The umbrella package. It holds no logic. The root entry re-exports the L1
vocabulary, the L2-L5 harness contract, L6 recall, and the L0 model
substrate. Each layer also has a curated subpath, for example
`@openagentsinc/ai/harness` and `@openagentsinc/ai/recall`.

- Key exports: the union of the layer packages below, plus the subpaths
  `./model`, `./schema`, `./event-log`, `./sandbox`, `./harness`,
  `./ui-stream`, `./recall`, and `./rlm`.
- Use it as the default dependency. Reach for a layer package directly only
  when you want a smaller dependency surface.
- npm: [@openagentsinc/ai](https://www.npmjs.com/package/@openagentsinc/ai)

## @openagentsinc/agent-runtime-schema

The L1 vocabulary. One neutral event union from the model call to the
rendered message.

- Key exports: `KhalaRuntimeEvent`, `decodeKhalaRuntimeEvent`,
  `AgentRuntimeVisibility`, `AgentRuntimeRedactionClass`, and the turn,
  provider, artifact, and route schemas.
- Use it when a consumer needs the event union or its codecs without the
  harness machinery. Sequence is the durable cursor. Visibility and
  redaction class are schema fields on every event.
- npm: [@openagentsinc/agent-runtime-schema](https://www.npmjs.com/package/@openagentsinc/agent-runtime-schema)

## @openagentsinc/agent-harness-contract

The L2-L5 core. The durable event log, the sandbox contract, the harness
adapter contract, and the UI stream projection live here.

- L2 key exports: `makeHarnessEventLog`, `makeInMemoryEventLogStore`, replay,
  live attach, and rerun boundaries over a seq-cursor log.
- L3 key exports: the sandbox-provider contract and
  `makeLocalProcessSandboxProvider`.
- L4 key exports: the `AgentHarness` contract, `makeReferenceAdapter`,
  `projectHarnessReadiness`, capability-by-method-presence, session verbs,
  skills, host tools, and the ACP and opencode adapters.
- L5 key exports: `khalaEventToUiChunks`, `initialUiMessage`, `applyUiChunk`,
  `reduceUiMessageStream`, smooth streaming, partial object streams, and the
  chat transports for event-log, SSE, and IPC.
- Use it to build or consume a coding-agent harness behind one versioned
  contract.
- npm: [@openagentsinc/agent-harness-contract](https://www.npmjs.com/package/@openagentsinc/agent-harness-contract)

## @openagentsinc/ai-model

The L0 model-call substrate over `effect/unstable/ai`. Upstream is consumed,
never forked.

- Key exports: `khalaEffectAiLanguageModelLayer`,
  `makeKhalaModelFallbackPlan`, `buildKhalaAiSdkCoreStreamTextOptions`, and
  `runKhalaAiSdkCoreRuntime`.
- Use it to make the model call, map provider stream parts into
  `KhalaRuntimeEvent`, and run typed fallback plans that never launder an
  exhausted account.
- npm: [@openagentsinc/ai-model](https://www.npmjs.com/package/@openagentsinc/ai-model)

## @openagentsinc/history-corpus

L6 recall. The full history stays durable and a typed service traverses it.

- Key exports: `buildHistoryCorpus`, `corpusEntriesToJsonl`, `recallTierD`,
  the `HistoryRecall` service, and the `history_recall` host tool
  (`HistoryRecallTool`, `historyRecallToolkitLayer`,
  `dispatchHistoryRecallHostTool`).
- Use it for recall instead of compaction. Tier D recall is pure and
  deterministic and reports an honesty record with every answer.
- npm: [@openagentsinc/history-corpus](https://www.npmjs.com/package/@openagentsinc/history-corpus)

## @openagentsinc/rlm

The recursive recall engine. It runs typed recall programs over a corpus
source, deterministically first and recursively second.

- Key exports: the `Rlm` service, `makeRlm`, `rlmLayer`,
  `rlmDeterministicLayer`, `runRlm`, `streamRlm`, and the `RlmCorpusSource`
  service the host application implements.
- Use it when Tier D questions are not enough and you want a bounded engine
  that composes recall steps.
- npm: [@openagentsinc/rlm](https://www.npmjs.com/package/@openagentsinc/rlm)

## @openagentsinc/ai-sdk-sandbox-local

The L3 local sandbox provider implementation.

- Key exports: `createLocalAiSdkSandboxProvider` and
  `LocalAiSdkSandboxProvider`.
- Use it to run harness work in isolated local account homes on the
  developer machine.
- npm: [@openagentsinc/ai-sdk-sandbox-local](https://www.npmjs.com/package/@openagentsinc/ai-sdk-sandbox-local)

## @openagentsinc/ai-sdk-sandbox-openagents

The L3 managed sandbox provider implementation.

- Key exports: `createOpenAgentsAiSdkSandboxProvider` and
  `OpenAgentsAiSdkSandboxProvider`.
- Use it to attach harness work to a managed OpenAgents sandbox with the
  server as the authority.
- npm: [@openagentsinc/ai-sdk-sandbox-openagents](https://www.npmjs.com/package/@openagentsinc/ai-sdk-sandbox-openagents)

## Version discipline

The `rc` dist-tag tracks the current pre-stable train. Pre-stable never
takes the stable `latest` badge on purpose. Pin an exact version in a
production consumer and move the pin deliberately.
