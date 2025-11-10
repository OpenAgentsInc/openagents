# OpenAgents Engine Issues

This directory contains implementation issues for the OpenAgents engine components: embeddings, SearchKit, and IssueAgent.

## Organization

```
issues/
├── embeddings/     # Low-level embedding generation & storage (001-005)
├── searchkit/      # Hybrid search primitives (006-015)
└── issue-agent/    # High-level issue→PR automation (016-030)
```

## Issue Numbering

- **001-005**: Embeddings (MLX, storage, vector search)
- **006-015**: SearchKit (FTS5, hybrid search, chunking)
- **016-030**: IssueAgent (orchestration, retrieval, patching, PR)

## Status Labels

- `status:proposed` - Issue defined, not started
- `status:in-progress` - Actively being worked on
- `status:blocked` - Waiting on dependencies
- `status:review` - Implemented, needs review
- `status:done` - Completed and merged

## Priority Labels

- `priority:p0` - Critical, blocking other work
- `priority:p1` - High priority
- `priority:p2` - Medium priority
- `priority:p3` - Nice-to-have

## Component Labels

- `component:embeddings` - Embedding generation & storage
- `component:searchkit` - Search primitives
- `component:issue-agent` - Issue automation
- `component:acp` - ACP integration
- `component:bridge` - Bridge/RPC layer

## Dependencies

Many issues have dependencies on earlier issues. Check the "Depends On" section in each issue before starting work.

## Related Plans

- [Embeddings Implementation Plan](../../plans/embeddings-implementation-plan.md)
- [IssueAgent Architecture](../../plans/issue-agent-architecture.md)
- [SearchKit Spec v0.2.2](../spec-v0.2.2.md)
