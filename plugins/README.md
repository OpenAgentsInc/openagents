# Agent plugins for the decision API

Two declarative packages teach an agent client how to authenticate to
and call the OpenAgents decision API. Nothing here installs itself —
copy a package into the client's plugin directory and edit the
environment values for your deployment.

| Package | Manifest | Install into |
| --- | --- | --- |
| `claude/` | `.claude-plugin/plugin.json` + `.mcp.json` + `skills/` | Claude Code plugin directory |
| `codex/` | `.codex-plugin/plugin.json` + `.mcp.json` + `skills/` | Codex plugin directory |
| `skills/openagents-decision-api/` | bare `SKILL.md` | Any client that reads the Agent Skills format |

## Manual installation

1. Build the caller binaries once:

   ```sh
   cargo build --release -p oak
   # target/release/oak, target/release/oak-mcp, target/release/oak-mcp-http
   ```

   Put `oak-mcp` on `PATH`, or edit the package's `.mcp.json` to name an
   absolute `command`.

2. Set the environment the `.mcp.json` forwards:

   ```sh
   export OPENAGENTS_API_KEY="oak_<id>.<secret>"   # issued by the operator
   export OPENAGENTS_BASE_URL="https://your-gateway.example.com"
   ```

3. Copy the package into the client's plugin directory — for example
   `plugins/claude/` to the Claude Code plugin location, or
   `plugins/codex/` to the Codex plugin location. Each package is
   self-contained: manifest, MCP server declaration, and the skill.

4. The skill alone is installable without a manifest: copy
   `skills/openagents-decision-api/` into any skills directory the
   client reads.

## Supported versions

- Claude-compatible clients that read `.claude-plugin/plugin.json` and
  Agent Skills (`SKILL.md` with `name` and `description` frontmatter).
- OpenAI Codex clients that read `.codex-plugin/plugin.json` — the
  compatibility manifest layout — and Agent Skills.
- The MCP declaration targets `oak-mcp` over stdio; the same tools are
  available over Streamable HTTP from `oak-mcp-http` — point
  `.mcp.json` at the running endpoint instead of `command` if your
  client supports remote servers.

## What the packages deliberately omit

No OAuth flow, no marketplace entry, no hooks, and no install-time
scripts. The skill documents implemented behavior only; the served
`api-catalog.json` at the origin names what does not exist.
