# @openagentsinc/ai-sdk-sandbox-local

Owner-local `HarnessV1SandboxProvider` for AI SDK harness fixtures.

This package is intentionally not a production sandbox. It creates a temporary
workspace, scopes file APIs to that workspace, launches child processes with
explicit `HOME`, `CODEX_HOME`, and `CLAUDE_CONFIG_DIR`, and exposes localhost
port URLs for bridge experiments. It does not provide kernel, network, or
multi-tenant containment.

Use it to prove Khala Code can run AI SDK harnesses without Vercel before the
same adapter contract is backed by the OpenAgents sandbox/workroom runtime.

Set `inheritClaudeConfig: true` for an owner-local Claude Code session that
must reuse the host CLI's current login instead of an isolated config path.
