# Moltbot codebase tour (overview)

This is a high-level map of the Moltbot repo at `~/code/moltbot`. It is meant as
an orientation doc for integration work (Autopilot or other systems) so you can
find the right surface quickly.

## Top-level layout

- `src/` - Core runtime (Gateway, agent loop, tools, routing, config, CLI). This
  is the heart of Moltbot.
- `extensions/` - Plugin packages (channels, tools, auth providers, optional
  features). Most integrations live here.
- `docs/` - Mintlify docs for the product. Good for protocol + feature behavior.
- `apps/` - Client apps (mobile/macOS). Separate from the Gateway runtime.
- `packages/` - Shared packages and build plumbing.
- `skills/` - Bundled skills shipped with the product.
- `ui/` - Web UI sources (Control UI and related assets).
- `test/` - Test fixtures and non-colocated test utilities.

## Runtime core (src/)

### Gateway (control plane)
- `src/gateway/` - WebSocket + HTTP server, protocol, auth, device pairing,
  node support, tool invocation, and request handling.
- Key docs: `docs/concepts/architecture.md`, `docs/gateway/protocol.md`,
  `docs/gateway/tools-invoke-http-api.md`.

### Agent loop and tool runtime
- `src/agents/` - Embedded agent runtime, tool wiring, sessions, skills,
  sandboxing, and provider/model selection.
  - Core tool registry is built in `src/agents/moltbot-tools.ts` and merges
    plugin tools via `resolvePluginTools`.
  - CLI fallback backends live in `src/agents/cli-backends.ts` and
    `src/agents/cli-runner.ts`.
- `docs/concepts/agent-loop.md` describes the main runtime lifecycle.

### Tools
- `src/agents/tools/` - Built-in tool implementations (browser, message,
  nodes, sessions, etc.).
- `src/plugins/` - Plugin tool registry, optional tools, and hook contracts.

### Routing, sessions, and channels
- `src/routing/` - Session key rules, agent routing, group handling.
- `src/sessions/` - Session storage and policy (send policy, overrides, etc.).
- `src/channels/` - Built-in channels and adapters.
- Extensions add additional channels under `extensions/*`.

### Configuration and policy
- `src/config/` - Types, schema validation, defaults, config loading.
- `src/infra/` - Support utilities: TLS, gateway lock, auth helpers,
  presence, exec approvals.

### CLI
- `src/commands/` - CLI commands (`moltbot gateway`, `moltbot agent`,
  `moltbot doctor`, etc.).
- `src/cli/` - CLI framework utilities, prompts, progress output.

## Plugin system (extensions/)

- Plugins are in-process TypeScript modules loaded by the Gateway.
- Capabilities: tools, gateway methods, http routes, services, providers,
  channels, hooks, and skills.
- Examples:
  - `extensions/llm-task/` (optional JSON-only LLM tool)
  - `extensions/nostr/` (channel plugin)
  - `extensions/voice-call/` (telephony)
- Plugin docs: `docs/plugin.md`, `docs/plugins/agent-tools.md`.

## Channels

Built-in channel implementations live in `src/` (WhatsApp, Telegram, Slack,
Discord, Signal, iMessage, WebChat). Optional channel plugins live in
`extensions/` (Matrix, Zalo, MS Teams, etc.).

Docs: `docs/channels/`.

## Nodes (device control)

Nodes connect to the Gateway as role `node` and expose commands like
`camera.snap`, `canvas.navigate`, `screen.record`, `location.get`.

- Protocol: `docs/gateway/protocol.md`
- Node tools: `src/agents/tools/nodes-tool.ts`
- Node command policies: `src/gateway/node-command-policy.ts`

## Skills and hooks

- Skills: `skills/` (bundled) + user installs in `~/.clawdbot/skills` +
  workspace skills in `<workspace>/skills`.
- Hooks: event-driven scripts discovered from workspace, managed, or bundled
  directories. See `docs/hooks.md`.

## Docs and user behavior

Moltbot docs are in `docs/` and are served via Mintlify. These docs are
extensive and include:
- Gateway behavior and protocol
- Channel setup
- Tool surface and policy
- Onboarding and configuration

## Integration entry points (summary)

If you are integrating another system (Autopilot, custom agent, external
service), the main entry points are:

- Gateway WS API (operator or node role)
- Gateway HTTP endpoints (`/tools/invoke`, `/v1/responses`)
- Plugin system (optional tool or gateway method)
- Hooks and skills for prompting and automation

## Reference files (quick links)

- Gateway protocol: `docs/gateway/protocol.md`
- Gateway architecture: `docs/concepts/architecture.md`
- Tools invoke API: `docs/gateway/tools-invoke-http-api.md`
- Agent loop: `docs/concepts/agent-loop.md`
- Plugin system: `docs/plugin.md`
- Plugin tools: `docs/plugins/agent-tools.md`
