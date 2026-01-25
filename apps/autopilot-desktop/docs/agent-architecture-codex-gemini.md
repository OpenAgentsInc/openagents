# Agent Implementation Architecture: Codex & Gemini

This document outlines the architecture for integrating Codex and Gemini agents into the Autopilot codebase, ensuring complete feature parity and adherence to the Agent Client Protocol (ACP).

## Architectural Vision

Autopilot uses a **capability-driven architecture**. Instead of treating agents as unique entities with different code paths, we treat them as interchangeable components that advertise specific capabilities through the Agent Client Protocol.

### Core Principles
1. **Protocol Uniformity**: All agents communicate via standard ACP JSON-RPC messages.
2. **Interchangeability**: The UI and orchestration layers should not care whether the underlying agent is Codex or Gemini.
3. **Feature Parity**: Both agents must support the same baseline capabilities (Session management, File system, Terminal, Tooling).

---

## Agent Integration Strategy

### 1. Unified Agent Interface
We define a single `Agent` interface that abstracts the underlying provider.

```typescript
interface Agent {
  readonly id: string;
  readonly name: string;
  readonly version: string;
  
  /**
   * Resolves the executable command and environment variables 
   * required to spawn the agent process.
   */
  getCommand(
    rootDir?: string,
    extraEnv?: Record<string, string>
  ): Promise<AgentCommand>;

  /**
   * Returns the capabilities supported by this specific implementation.
   */
  getCapabilities(): AgentCapabilities;
}
```

### 2. Capability-Driven Design
Agents differentiate themselves by the flags they set in their `AgentCapabilities` response. Autopilot's UI dynamically enables or disables features based on these flags.

| Capability | Purpose | Supported by Codex | Supported by Gemini |
| :--- | :--- | :---: | :---: |
| `session_new` | Start fresh conversations | ✅ | ✅ |
| `session_load` | Resume previous work | ✅ | ✅ |
| `fs_write` | Create/Edit source code | ✅ | ✅ |
| `fs_read` | Context awareness | ✅ | ✅ |
| `terminal` | Run tests/builds | ✅ | ✅ |

### 3. Agent Registry and Factory
A centralized registry manages the available agent types, allowing for easy selection and configuration.

```typescript
const AGENT_REGISTRY = {
  codex: {
    name: "Codex",
    implementation: CodexAgent,
    description: "High-performance coding agent optimized for local workflows."
  },
  gemini: {
    name: "Gemini CLI",
    implementation: GeminiAgent,
    description: "Google's Gemini model integrated via the standard CLI tool."
  }
};
```

---

## Implementation Details

### Codex Agent
- **Source**: Local binary or integrated SDK.
- **Initialization**: Typically requires no extra flags to enable ACP.
- **Auth**: Uses system-level OAuth or API Keys stored in secure storage.

### Gemini Agent
- **Source**: `@google/gemini-cli` npm package.
- **Initialization**: Requires the `--experimental-acp` flag to enable ACP mode.
- **Resolution**: Implements a "find-or-install" logic similar to Zed:
  1. Check for global `gemini` binary.
  2. Fall back to local `node_modules`.
  3. Offer to install via `npm install -g @google/gemini-cli` if missing.

---

## Feature Parity Roadmap

To achieve true parity, both agents are integrated with the same Autopilot subsystems:

1. **Unified Stream**: Both agents feed into the `UnifiedConversationItem` renderer (Message, Reasoning, Tool, Diff).
2. **Context Servers**: Both agents can connect to MCP (Model Context Protocol) servers provided by the host.
3. **Diff Viewing**: Both agents emit standard diff chunks that are rendered using our Effuse `Diff` component.
4. **Auth Workflow**: Both agents share the same UI modal for authentication, with provider-specific logic handled at the agent instance level.

---

## User Experience

Users can switch between Codex and Gemini from the **Agent Panel**. 
- **Configuration** is per-agent (API keys, default models).
- **History** is unified; sessions are tagged with the agent that created them.
- **Tooling** remains consistent; whether using Codex or Gemini, the user has access to the same terminal and file operations.
