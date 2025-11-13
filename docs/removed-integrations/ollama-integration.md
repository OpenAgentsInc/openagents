# Ollama Integration (Removed)

**Removed**: 2025-11-12
**Reference Commit**: `b7713e63106ca7d947c47bb7a96a195b79938621`
**GitHub Issue**: [#1483](https://github.com/OpenAgentsInc/openagents/issues/1483)

## Overview

The Ollama integration provided support for local LLM inference using the [Ollama](https://ollama.ai/) server. This allowed users to interact with locally-hosted language models (specifically `glm-4.6:cloud` - Zhipu AI's GLM-4 model) without requiring cloud-based API services.

The integration was removed to simplify the codebase and focus exclusively on ACP (Agent Communication Protocol) based agents: Claude Code and Codex.

## Why It Was Removed

1. **Architectural Focus**: Streamline to support only ACP-based agents
2. **Code Simplification**: Remove dual-runtime complexity
3. **Maintenance Burden**: Reduce testing surface and dependency maintenance
4. **User Experience**: Single, consistent protocol across all agents

## What It Provided

- **Local LLM Inference**: Run language models locally via Ollama server
- **Privacy**: All inference happened on-device, no cloud API calls
- **Model Flexibility**: Support for any Ollama-compatible model
- **Performance Optimization**: Used OLLAMA_FLASH_ATTENTION and q8_0 quantization
- **Streaming Responses**: Real-time token streaming for responsive UX
- **Message Persistence**: Integration with tinyvex database for conversation history

## Architecture

### Core Components

The Ollama integration consisted of several key components that worked together:

1. **Custom Runtime Hook** (`useOllamaRuntime.tsx`)
   - Managed Ollama-specific runtime lifecycle
   - Integrated with tinyvex WebSocket for persistence
   - Handled streaming responses from Ollama API
   - Created threads with `source="ollama"` for filtering

2. **Chat Adapter** (`adapters/ollama-adapter.ts`)
   - Factory function to create ChatModelAdapter instances
   - Wrapped `ollama-ai-provider-v2` for Vercel AI SDK compatibility
   - Configured Ollama base URL and model selection

3. **Configuration** (`config/ollama.ts`)
   - Centralized Ollama settings
   - Environment variable support for customization
   - Default: `http://127.0.0.1:11434/api` and `glm-4.6:cloud`

4. **Tauri Commands** (`src-tauri/src/lib.rs`)
   - `create_ollama_thread`: Create new conversation thread
   - `save_ollama_message`: Persist messages to tinyvex database

### Data Flow

```
User Input
    ↓
useOllamaRuntime
    ↓
Ollama Adapter (ChatModelAdapter)
    ↓
ollama-ai-provider-v2
    ↓
Ollama Server (127.0.0.1:11434)
    ↓
Model Inference (glm-4.6:cloud)
    ↓
Streaming Response
    ↓
tinyvex Persistence (via WebSocket)
    ↓
UI Update
```

### Integration Points

The Ollama runtime integrated with the broader application through:

- **Runtime Switcher** (`MyRuntimeProvider.tsx`): Selected between Ollama and ACP runtimes based on user choice
- **Model Store** (`model-store.ts`): Managed model selection state with "ollama" option
- **UI Components**: Model toolbar and header dropdowns for selecting Ollama
- **Thread UI**: Special handling for Ollama threads (Refresh button, Edit capabilities)

## Files Removed

### Core Ollama Files (4 files)

1. **`tauri/src/runtime/useOllamaRuntime.tsx`** (390 lines)
   - Custom runtime hook for Ollama integration
   - Message streaming and thread management
   - tinyvex persistence integration
   - **Key patterns**: ExternalStoreAdapter usage, streaming accumulation, WebSocket sync

2. **`tauri/src/runtime/adapters/ollama-adapter.ts`** (26 lines)
   - ChatModelAdapter factory for Ollama
   - **Key patterns**: Vercel AI SDK integration, adapter pattern

3. **`tauri/src/config/ollama.ts`** (4 lines)
   - Configuration constants
   - Environment variable reading

4. **`tauri/src/__mocks__/ollama-adapter.ts`** (16 lines)
   - Mock adapter for testing/Storybook
   - **Key patterns**: Mock implementation strategy

### Modified Files (7 files)

1. **`tauri/src/runtime/MyRuntimeProvider.tsx`**
   - Removed: Ollama runtime branch in conditional logic
   - Kept: ACP runtime selection

2. **`tauri/src/lib/model-store.ts`**
   - Removed: `"ollama"` from `ModelKind` type
   - Kept: `"codex" | "claude-code"` types

3. **`tauri/src/components/assistant-ui/model-toolbar.tsx`**
   - Removed: `<SelectItem value="ollama">GLM-4.6</SelectItem>`
   - Kept: Codex and Claude Code options

4. **`tauri/src/components/assistant-ui/app-header.tsx`**
   - Removed: `<option value="ollama">Ollama (glm-4.6:cloud)</option>`
   - Kept: ACP-based options

5. **`tauri/src/components/assistant-ui/thread.tsx`**
   - Removed: Ollama-specific conditionals (Refresh button, Edit button logic)
   - Kept: General thread rendering logic

6. **`tauri/src/runtime/index.ts`**
   - Removed: `createOllamaAdapter` export
   - Kept: Other runtime exports

7. **`tauri/src-tauri/src/lib.rs`**
   - Removed: `create_ollama_thread` and `save_ollama_message` commands
   - Kept: ACP-based Tauri commands

### Dependencies Removed

- **`ollama-ai-provider-v2`** (v1.5.3): Vercel AI SDK provider for Ollama

## How to Re-add in the Future

If you need to re-add Ollama support or study the implementation, follow these steps:

### 1. Study the Reference Commit

```bash
git checkout b7713e63106ca7d947c47bb7a96a195b79938621
```

### 2. Key Files to Study

Focus on these files to understand the implementation:

1. **`tauri/src/runtime/useOllamaRuntime.tsx`**
   - Study how it implements the ExternalStoreAdapter pattern
   - Note the streaming accumulation strategy (concat chunks vs. replace)
   - Understand tinyvex WebSocket integration for persistence
   - See how threads are created with `source="ollama"`

2. **`tauri/src/runtime/adapters/ollama-adapter.ts`**
   - Understand the ChatModelAdapter factory pattern
   - See how to wrap external providers for assistant-ui compatibility

3. **`tauri/src-tauri/src/lib.rs`**
   - Lines 137-186: `create_ollama_thread` command
   - Lines 188-254: `save_ollama_message` command
   - Study JSON handling and tinyvex database operations

### 3. Key Patterns to Understand

**Pattern 1: Custom Runtime Hook**
```typescript
// useOllamaRuntime.tsx pattern
const useOllamaRuntime = (workingDirectory?: string) => {
  const adapter = useMemo(() => createOllamaAdapter(), []);
  return useExternalStoreRuntime({
    adapters: { chatModel: adapter },
    // ... other config
  });
};
```

**Pattern 2: ChatModelAdapter Implementation**
```typescript
// ollama-adapter.ts pattern
export const createOllamaAdapter = (): ChatModelAdapter => {
  return async ({ abortSignal, messages }) => {
    const stream = ollama(model).doStream({
      messages,
      abortSignal,
    });
    return stream;
  };
};
```

**Pattern 3: Thread Persistence**
```rust
// lib.rs pattern
#[tauri::command]
async fn create_ollama_thread(...) -> Result<String, String> {
  // Create thread in tinyvex
  // Return thread_id
}

#[tauri::command]
async fn save_ollama_message(...) -> Result<(), String> {
  // Save message to tinyvex
}
```

### 4. Dependencies to Add

```bash
cd tauri
bun add ollama-ai-provider-v2
```

### 5. Configuration Checklist

- [ ] Add Ollama configuration to `src/config/ollama.ts`
- [ ] Add environment variables: `VITE_OLLAMA_BASE_URL`, `VITE_OLLAMA_MODEL`
- [ ] Update `model-store.ts` to include `"ollama"` in `ModelKind`
- [ ] Add Ollama runtime to `MyRuntimeProvider.tsx`
- [ ] Add Ollama option to UI dropdowns
- [ ] Implement Ollama-specific Tauri commands
- [ ] Update CLAUDE.md with Ollama setup instructions
- [ ] Add Ollama server to prerequisites

### 6. Testing Checklist

- [ ] Verify Ollama server connection
- [ ] Test message streaming
- [ ] Test thread creation and persistence
- [ ] Test model switching between Ollama and ACP
- [ ] Verify working directory handling
- [ ] Test error handling (server offline, model not found)

## Lessons Learned

### What Worked Well

1. **Clean Separation**: Ollama runtime was well-isolated from ACP runtime
2. **Adapter Pattern**: ChatModelAdapter made it easy to swap implementations
3. **Persistence Integration**: tinyvex WebSocket worked for both runtimes
4. **Streaming**: Chunk accumulation strategy provided smooth UX

### Challenges

1. **Dual Runtime Complexity**: Supporting two different runtime patterns increased code complexity
2. **UI Conditionals**: Thread UI had to handle Ollama-specific features (Refresh, Edit)
3. **State Management**: Model switching required careful runtime lifecycle management
4. **Documentation**: Ollama setup required additional user documentation

### Architecture Insights

1. **Runtime Abstraction**: assistant-ui's runtime abstraction made multiple backends possible
2. **ExternalStoreAdapter**: Powerful pattern for custom runtime implementations
3. **Tauri Commands**: Clean separation between frontend and backend persistence
4. **WebSocket Sync**: Real-time updates worked well across runtimes

## Alternative Approaches

If re-adding local LLM support in the future, consider these alternatives:

1. **ACP Wrapper for Ollama**: Implement Ollama as an ACP-compatible agent instead of a custom runtime
2. **Plugin Architecture**: Allow Ollama as a plugin rather than core integration
3. **Unified Runtime**: Build a single runtime that supports both ACP and direct LLM APIs
4. **Server-Side ACP Bridge**: Create a bridge service that translates ACP to Ollama protocol

## References

- **Ollama**: https://ollama.ai/
- **ollama-ai-provider-v2**: https://www.npmjs.com/package/ollama-ai-provider-v2
- **assistant-ui**: https://www.assistant-ui.com/
- **Vercel AI SDK**: https://sdk.vercel.ai/
- **GLM-4 Model**: https://open.bigmodel.cn/dev/howuse/glm-4

## Questions?

For questions about this removal or the original implementation, see:
- GitHub Issue: #1483
- Commit: b7713e63106ca7d947c47bb7a96a195b79938621
- Contact: OpenAgents team
