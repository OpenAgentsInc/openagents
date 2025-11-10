# Plan Deltas — Proposed Edits to Align Plans

## Embeddings Plan

- Switch RPC naming to dot form to match repo conventions
  - Change `embedding/generate` → `embedding.generate`
    - docs/plans/embeddings-implementation-plan.md:752
  - Same for `embedding/store`, `embedding/search`, `embedding/store_batch`

- Note Tinyvex integration explicitly (call migration in `migrate()`)
  - Reference Tinyvex DbLayer: ios/OpenAgentsCore/Sources/OpenAgentsCore/Tinyvex/DbLayer.swift:1

## SearchKit Compute Doc

- Replace direct “Core ML embeddings” component with dependency on EmbeddingService (provider-agnostic)
  - docs/compute/issues/phase-4-searchkit/024-searchkit-mvp.md:47,83
  - Add note: semantic leg uses `EmbeddingService.semanticSearch` and shared `embeddings` table

## IssueAgent Plan

- Clarify ACP tool exposure vs orchestration RPCs
  - Keep tools as per §6 (tool names), stream via `session/update`
    - docs/plans/issue-agent-architecture.md:776,791
  - Note that scheduled/automation runs should use existing `orchestrate/coordinator.run_once`
    - ios/OpenAgentsCore/Sources/OpenAgentsCore/AgentClientProtocol/rpc.swift:33

