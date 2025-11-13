# Repository Guidelines

## Codebase Summary

**OpenAgents** is a desktop chat application for interacting with AI assistants, built with Tauri, React, TypeScript, and assistant-ui.

- **Purpose**: Desktop command center for AI assistants using ACP (Agent Communication Protocol). Supports Claude Code and Codex agents with tools and rich UI interactions.
- **Architecture**: Tauri (Rust + WebView) with React/TypeScript frontend, assistant-ui components, and ACP-based agent runtime
- **Platforms**: Windows, macOS, Linux (cross-platform desktop)
- **Previous versions**: v0.3 (Swift iOS/macOS) is deprecated and no longer maintained. v0.2 (Expo/React Native + Rust) also deprecated.

### Key Components

- **Tauri Desktop App** (`tauri/`)
  - Cross-platform desktop application (Windows, macOS, Linux)
  - React + TypeScript frontend with assistant-ui components
  - ACP runtime for Claude Code and Codex agents
  - Tool system with makeAssistantTool for client-side tools
  - Dark mode UI with Berkeley Mono font
  - Sidebar with ThreadList for conversation history

- **Frontend** (`tauri/src/`)
  - React components using assistant-ui library
  - AssistantRuntimeProvider with useAcpRuntime adapter
  - ACP protocol for agent communication
  - Tool definitions in `src/tools/`
  - shadcn/ui components for UI primitives

- **Backend** (`tauri/src-tauri/`)
  - Rust-based Tauri application
  - Native OS integration and window management
  - ACP session management and message handling

### Repository Layout

```
tauri/                            # Main Tauri desktop app (TypeScript/React)
├── src/                          # React application source
│   ├── App.tsx                   # Main app component with runtime setup
│   ├── App.css                   # Global styles and CSS variables
│   ├── components/               # React components
│   │   ├── assistant-ui/         # assistant-ui components
│   │   │   ├── assistant-sidebar.tsx    # Left sidebar with ThreadList
│   │   │   ├── thread.tsx               # Main chat interface
│   │   │   └── thread-list.tsx          # Conversation history list
│   │   └── ui/                   # shadcn/ui components
│   └── tools/                    # Tool definitions
│       └── calculator.tsx        # Example calculator tool
├── src-tauri/                    # Tauri Rust backend
│   ├── src/                      # Rust source code
│   └── Cargo.toml                # Rust dependencies
├── package.json                  # JavaScript dependencies (use bun!)
└── tsconfig.json                 # TypeScript configuration

docs/                             # Documentation
├── adr/                          # Architecture Decision Records
└── logs/                         # Development logs

ios/                              # DEPRECATED Swift iOS/macOS app (v0.3)
packages/tricoder/                # DEPRECATED npm package (v0.2)
```

### Package Management

**CRITICAL**: **ALWAYS use `bun` for installing packages**. Never use `npm` or `yarn`.

```bash
cd tauri
bun install              # Install dependencies
bun run dev              # Start dev server
bun run build            # Build for production
bun add <package>        # Add new package
```

## Architecture

### Tauri + React Architecture

**Stack**: Tauri for native desktop, React + TypeScript for UI, assistant-ui for chat components, ACP for agent communication.

- **Tauri**: Rust-based desktop application framework with native OS integration
- **React + TypeScript**: Modern frontend with full type safety
- **assistant-ui**: Specialized React library for AI chat interfaces
- **ACP**: Agent Communication Protocol for Claude Code and Codex agents
- **Tools**: Client-side tool execution using makeAssistantTool
- **Styling**: Tailwind CSS with dark mode, Berkeley Mono font, zero border radius

### Tool System

Tools extend the assistant's capabilities with custom functions:

- **Definition**: Use `makeAssistantTool` to create client-side tools
- **Parameters**: Zod schemas for type-safe parameter validation
- **Execution**: Async functions that run in the browser
- **Registration**: Place tool components inside AssistantRuntimeProvider
- **Example**: See `src/tools/calculator.tsx` for reference implementation

### ACP Integration

Agent communication powered by ACP (Agent Communication Protocol):

- **Agents**: Supports Claude Code and Codex agents
- **Runtime**: useAcpRuntime hook manages ACP sessions
- **Protocol**: Bidirectional communication for prompts and responses
- **Persistence**: Messages stored in tinyvex database via WebSocket

## Development

### Prerequisites

- **Bun** (JavaScript package manager - required)
- **Node.js 18+** (for Tauri and Vite)
- **Rust** (for Tauri build - will be installed automatically if missing)
- **Ollama** (for local LLM inference)

### Setting Up

```bash
# Install bun if needed
curl -fsSL https://bun.sh/install | bash

# Clone and setup
cd tauri
bun install              # Install all dependencies
```

### Running Ollama

```bash
# Start Ollama server (required for chat)
OLLAMA_FLASH_ATTENTION="1" OLLAMA_KV_CACHE_TYPE="q8_0" ollama serve

# In another terminal, pull the model if needed
ollama pull glm-4.6:cloud
```

### Building and Running

#### Development Mode

```bash
cd tauri
bun run dev              # Start Vite dev server + Tauri app
```

This starts the Vite development server and launches the Tauri application. Changes to React code hot-reload automatically.

#### Production Build

```bash
cd tauri
bun run build            # Build optimized bundle
bun run tauri build      # Build native app for your platform
```

Built applications will be in `src-tauri/target/release/`.

### Type Checking

```bash
cd tauri
bun run build            # Runs tsc + vite build
```

Always run type checking before committing to catch TypeScript errors early.

### Testing

Currently no automated tests configured. Test manually by:
1. Starting the dev server (`bun run dev`)
2. Testing chat functionality with Ollama
3. Testing tool execution (e.g., calculator tool)
4. Verifying ThreadList and conversation history

## Coding Style & Conventions

### TypeScript/React Style

- **Language**: TypeScript (strict mode enabled)
- **Formatting**: Prettier with 2-space indentation
- **Indentation**: 2 spaces for JavaScript/TypeScript/JSX
- **Fonts**:
  - All monospace text MUST use Berkeley Mono
  - Set via `font-family: 'Berkeley Mono', monospace` in CSS
  - Global styles in `App.css` define font-face declarations
- **Naming**:
  - Components: `PascalCase` (e.g., `AssistantSidebar`, `CalculatorTool`)
  - Functions/variables: `camelCase` (e.g., `useLocalRuntime`, `streamText`)
  - Constants: `UPPER_SNAKE_CASE` for true constants, `camelCase` for config
  - Files: Match component name (e.g., `assistant-sidebar.tsx`)

### React Conventions

- **Hooks**: Use hooks appropriately (`useState`, `useEffect`, `useRef`, etc.)
- **Components**: Prefer function components over class components
- **Props**: Define explicit TypeScript interfaces for component props
- **Composition**: Extract components when they exceed ~150 lines
- **assistant-ui**: Use provided hooks and components (`useLocalRuntime`, `makeAssistantTool`, etc.)

### Architecture Patterns

- **Component-based**: React components for all UI
- **Runtime Pattern**: AssistantRuntimeProvider wraps app, tools registered as children
- **Adapters**: ChatModelAdapter for custom LLM integrations
- **Async/Await**: Use for all asynchronous operations
- **Streaming**: Accumulate chunks properly for smooth text rendering

### Type Safety

- **Strict TypeScript**: Always enable strict mode
- **No `any`**: Avoid `any` type unless absolutely necessary, use `unknown` if type is truly unknown
- **Zod schemas**: Use Zod for runtime validation (tool parameters, API responses)
- **Type imports**: Use `import type` for type-only imports when possible

## Build Discipline (Mandatory)

### Before Committing

1. **Build succeeds**: Run `bun run build` to ensure TypeScript compiles
2. **No type errors**: Fix all TypeScript errors before committing
3. **Test manually**: Verify changes work in development mode
4. **Ollama running**: Ensure Ollama is running if testing chat functionality

### Build Breakage Policy

- If you break the build, **fix forward immediately** or revert the breaking change
- Never leave the main branch in a broken state
- Run `bun run build` before pushing to shared branches
- For large changes, test on a clean `bun install` to catch missing dependencies

### Pre‑release Policy

- Avoid feature gates/flags and backwards compatibility changes — app is still unreleased.
- Never add feature flags unless the user explicitly requests it. If a flag exists, prefer removing it and consolidating behavior.

## Git Workflow

### Branching Policy

- **Main branch**: `main` (production-ready code)
- **Default branch for work**: Commit directly to `main` unless instructed otherwise
- **Feature branches**: Only create when explicitly requested by user
- **No destructive operations**: Never use `git reset --hard`, `git clean -fdx`, `git stash`, or force pushes unless explicitly requested

### Commit Guidelines

- **Imperative mood**: "Add feature" not "Added feature" or "Adds feature"
- **Concise subject**: ≤50 characters
- **Body when needed**: Explain why, not what (the diff shows what)
- **Commit often**: Small, focused commits with immediate pushes
- **No stashing**: Always commit work in progress, never stash

### Staging Discipline

- **Only stage files you changed**: Never use `git add .` or `git add -A` unless you changed all those files
- **Use explicit paths**: `git add path/to/file.swift` not `git add .`
- **Review before committing**: `git status` and `git diff --staged` to verify staged changes
- **Leave unrelated changes untouched**: If you see unstaged changes you didn't make, leave them alone

### Multi-Agent Safety

- **Assume concurrent work**: Other agents may be working on the same branch
- **Never delete untracked files**: They may be in-progress work by another agent
- **No history rewriting**: No rebases, amended commits, or force pushes without explicit permission
- **Respect local changes**: Don't revert or restore files you didn't modify

## Architecture Decision Records (ADRs)

ADRs are available in `docs/adr/` but most are for the deprecated Swift v0.3 implementation. They may be useful for historical context but do not apply to the current Tauri implementation.

## Key Technologies

### Tauri

Rust-based framework for building native desktop apps with web technologies.

- **Version**: v2
- **Benefits**: Small bundle size, native performance, security
- **Backend**: Rust for system integration
- **Frontend**: Standard web stack (React, TypeScript, Vite)

### assistant-ui

React library specialized for AI chat interfaces.

- **Docs**: https://www.assistant-ui.com/
- **Components**: Thread, ThreadList, AssistantSidebar
- **Hooks**: useLocalRuntime, useAssistantTool
- **Tools**: makeAssistantTool for client-side tool definitions
- **Adapters**: Custom ChatModelAdapter for Ollama integration

### Ollama

Local LLM inference server.

- **Model**: glm-4.6:cloud (other models can be used)
- **API**: REST API compatible with OpenAI format
- **Integration**: ollama-ai-provider-v2 for Vercel AI SDK compatibility
- **Performance**: Optimized with OLLAMA_FLASH_ATTENTION and q8_0 quantization

## Security & Privacy

- **No secrets in code**: Use environment variables for API keys and sensitive data
- **Local-first**: All LLM inference runs locally via Ollama
- **No telemetry**: No analytics or tracking by default
- **Tool safety**: Tools run in browser context with standard web security model

## Common Tasks

### Adding a New Tool

1. Create tool file in `src/tools/` (e.g., `my-tool.tsx`)
2. Use `makeAssistantTool` with:
   - `toolName`: Unique identifier
   - `description`: Clear description for LLM
   - `parameters`: Zod schema for parameters
   - `execute`: Async function implementing the tool
3. Import and render tool component in `App.tsx` inside `AssistantRuntimeProvider`
4. Test by asking the assistant to use the tool

Example:
```typescript
import { makeAssistantTool } from "@assistant-ui/react";
import { z } from "zod";

export const MyTool = makeAssistantTool({
  toolName: "myTool",
  description: "What this tool does",
  parameters: z.object({
    param: z.string().describe("Parameter description"),
  }),
  execute: async ({ param }) => {
    // Tool logic here
    return { result: "value" };
  },
});
```

### Adding a New React Component

1. Create component in appropriate directory under `src/components/`
2. Use TypeScript with proper typing for props
3. Follow React hooks conventions
4. Import and use in parent components
5. Test in development mode

### Modifying Ollama Configuration

1. Update model in `App.tsx`: `ollama("model-name")`
2. Ensure model is pulled: `ollama pull model-name`
3. Adjust Ollama server flags in run command if needed
4. Test chat functionality after changes

### Styling Components

1. Use Tailwind CSS classes for styling
2. Respect CSS variables in `App.css` (e.g., `--radius`)
3. Use Berkeley Mono font for all text
4. Maintain dark mode theme (zinc-900, zinc-800, etc.)
5. Keep border radius at 0 for sharp corners

## Troubleshooting

### "No such module 'OpenAgentsCore'"

- Make sure you opened `OpenAgents.xcworkspace` not `OpenAgents.xcodeproj`
- Clean build folder: Product > Clean Build Folder (⌘⇧K)
- Close Xcode, delete `~/Library/Developer/Xcode/DerivedData/OpenAgents-*`, reopen

### Bridge Connection Issues

- Check macOS app is running and Bonjour service is advertised
- Check iOS app can browse `_openagents._tcp` services
- Use simulator: ensure both simulator and Mac are on same network
- Check logs: Server logs in Console.app, client logs in Xcode debug console

### Foundation Models Not Available

- Requires iOS 26+ or macOS 15+ with Apple Intelligence enabled
- Check device support: Settings > Apple Intelligence
- Models may be downloading: wait and retry
- Fallback logic should handle gracefully (see ADR-0006)

### Xcode Build Errors

- **"Command SwiftCompile failed"**: Check for syntax errors in Swift files
- **"Cycle in dependencies"**: Check framework/target dependencies in Xcode project settings
- **"Failed to build module"**: Clean build folder (⌘⇧K) and rebuild

## Testing Guidelines

### Unit Tests

- **Location**: `ios/OpenAgentsCore/Tests/OpenAgentsCoreTests/`
- **Naming**: `*Tests.swift` (e.g., `BridgeMessageTests.swift`)
- **Target**: Test shared core logic (ACP parsing, bridge messages, protocols)
- **Run**: Press ⌘U in Xcode

### Integration Tests

- Test WebSocket bridge end-to-end (server + client)
- Test ACP message flow through the system
- Test UI rendering with sample data
- Examples: `BridgeServerClientTests.swift`, `ToolCallViewRenderingIntegrationTests.swift`

### UI Tests

- **Framework**: XCTest UI Testing
- **Target**: `OpenAgentsUITests` (if configured)
- **Scope**: Critical user flows (session creation, agent prompting, message rendering)

### Test Coverage

- Aim for 70%+ coverage on new code
- Critical paths (ACP parsing, bridge protocol) should have 90%+ coverage
- Use Xcode's coverage reports: Product > Test (⌘U), then coverage tab

## Deprecation Notes

### v0.2 and Earlier (Deprecated)

The following are **NO LONGER SUPPORTED** as of v0.3:

- ❌ Expo/React Native mobile app (`expo/` - deleted)
- ❌ Rust WebSocket bridge (`crates/oa-bridge/` - deleted)
- ❌ Tauri desktop app (`tauri/` - deleted)
- ❌ TypeScript packages (`packages/openagents-core`, `packages/openagents-theme`, `packages/tinyvex` - deleted)
- ❌ npm package `tricoder` (v0.3.0 published as deprecated, last working version v0.2.5)
- ❌ Bun/npm build system (replaced with Xcode/SwiftPM)
- ❌ Maestro E2E tests (`.maestro/` - deleted)

### Migration from v0.2

If you were familiar with v0.2:

- **No more Expo**: All UI is now SwiftUI
- **No more Rust bridge**: Bridge is now Swift WebSocket server/client with JSON-RPC
- **No more TypeScript**: All application code is Swift
- **No more Tauri**: macOS app is native Swift
- **No more bun/npm**: Use Xcode and SwiftPM
- **No data migration**: v0.3 is a fresh start

The repository was cleaned up in PR #1414. See that PR and issue #1413 for the full deletion list and rationale.

### v0.3.0 Dashboard (Deprecated in v0.3.1)

The original macOS dashboard view (`SimplifiedMacOSView`) has been replaced by the chat‑first layout:

- Dashboard cards (Bridge Status, Working Directory, Agent Config, Dev Tools) moved into Settings (⌘,) and Developer (⌥⌘D) views.
- The main window now opens into `ChatMacOSView` with a session sidebar and chat area.
- See ADR‑0007 for the rationale and architecture details.

## Additional Documentation

- **ADRs**: `docs/adr/` - All architectural decisions
  - ADR‑0007: macOS Chat Interface Architecture
- **Liquid Glass**: `docs/liquid-glass/` - Visual design, APIs, examples
- **iOS Bridge**: `docs/ios-bridge/` - WebSocket protocol specification
- **Logs**: `docs/logs/` - Historical development logs and decisions

## Getting Help

- **GitHub Issues**: https://github.com/OpenAgentsInc/openagents/issues
- **ADRs**: Check `docs/adr/` for architectural context
- **TestFlight**: https://testflight.apple.com/join/dvQdns5B

## Summary for AI Agents

**This is a native Swift iOS/macOS project. Use Xcode, not Expo/npm/Rust tooling.**

- Build with Xcode (⌘B)
- Test with Xcode (⌘U)
- All code is Swift (no TypeScript, no Rust)
- Read ADRs in `docs/adr/` before making architectural changes
- Read `docs/adr/AGENTS.md` before creating/modifying ADRs
- Always build and test before committing
- Commit frequently with explicit file staging
- Never use destructive git operations unless explicitly requested
- LLM‑First Policy: NEVER add deterministic heuristics unless the user explicitly requests it; use Foundation Models wherever interpretation is needed.
