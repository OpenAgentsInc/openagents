 Plan: Internalize Assistant-UI Dependencies for ACP Runtime

     Current State Analysis

     What's Working:
     - useAcpRuntime() successfully connects to tinyvex WebSocket and streams ACP messages
     - Custom assistant-ui-runtime package vendored with minimal external-store runtime extraction
     - Vendor shim in tauri/src/vendor/assistant-ui/ routes to local packages
     - Full @assistant-ui/react package pulled into packages/react/

     Current External Dependencies:
     - ❌ @assistant-ui/react (npm) - used in 12+ files in tauri/src
     - ❌ @assistant-ui/react-markdown (npm) - used for markdown rendering
     - ❌ Vercel AI SDK (@ai-sdk/react, ai) - pulled in via react-ai-sdk/react-data-stream but NOT needed for ACP runtime
     - ✅ assistant-stream - already local, zero external deps
     - ✅ @assistant-ui/tap - already local, zero deps
     - ✅ Radix UI - reasonable boundary (UI primitives)
     - ✅ zustand, zod - reasonable boundaries (state/validation)

     Goal

     Remove ALL external assistant-ui npm dependencies and replace with local packages. Keep reasonable boundaries (Radix UI, zustand, zod) but internalize all assistant-ui
     glue code.

     What We Need (Core Functionality)

     1. Runtime System ✅ (already have)
       - useExternalStoreRuntime - in packages/assistant-ui-runtime/src/minimal/
       - ExternalStoreAdapter, MessageRepository types
     2. React Context & Providers ✅ (in packages/assistant-ui-runtime/)
       - AssistantRuntimeProvider
       - Context hooks (useThreadState, useMessageState, etc.)
     3. Primitives (UI Components) ✅ (in packages/assistant-ui-runtime/)
       - Thread, Message, Composer primitives
       - ThreadList, BranchPicker primitives
       - All in packages/assistant-ui-runtime/src/primitives/
     4. Tool System ✅ (in packages/assistant-ui-runtime/)
       - makeAssistantTool
       - Tool registry and execution
     5. Markdown Rendering ✅ (already pulled in)
       - packages/react-markdown/ - self-contained

     What We DON'T Need (Remove/Don't Use)

     1. react-ai-sdk ❌ - Vercel AI SDK integration (not using for ACP)
     2. react-data-stream ❌ - Alternative to AI SDK (not using for ACP)
     3. assistant-cloud ⚠️ - Optional cloud features (check if used)
     4. Any upstream @assistant-ui/* npm packages

     Execution Plan

     Phase 1: Audit Current Imports (Quick)

     1. Find all imports of @assistant-ui/react in tauri/src
     2. Map each import to its location in packages/assistant-ui-runtime/
     3. Identify any missing exports from local package

     Phase 2: Update Import Paths

     1. Replace all @assistant-ui/react imports with @openagentsinc/assistant-ui-runtime
     2. Update packages/assistant-ui-runtime/src/index.ts to export all needed symbols
     3. Verify path aliases in tsconfig.json are correct

     Phase 3: Handle Markdown Package

     1. Create proper package scope for react-markdown
     2. Update imports from @assistant-ui/react-markdown to local path
     3. Ensure styles are imported correctly

     Phase 4: Clean Up Unnecessary Packages

     1. Remove react-ai-sdk and react-data-stream from packages/ (if not needed)
     2. Remove Vercel AI SDK dependencies from tauri/package.json if not used elsewhere
     3. Document what was removed and why

     Phase 5: Verify No External Assistant-UI Dependencies

     1. Run bun run build to ensure everything compiles
     2. Grep for any remaining @assistant-ui imports from npm
     3. Update package.json to remove external assistant-ui packages

     Phase 6: Test Runtime Integration

     1. Verify ACP runtime still works with local packages
     2. Test message streaming, reasoning display, tool calls
     3. Ensure ThreadList and sidebar functionality works

     Expected Outcome

     - ✅ All assistant-ui code lives in packages/
     - ✅ Zero external assistant-ui npm dependencies
     - ✅ Vercel AI SDK removed (not needed for ACP)
     - ✅ Clean import paths via @openagentsinc/*
     - ✅ Reasonable boundaries maintained (Radix, zustand, zod)
     - ✅ ACP runtime works end-to-end with vendored packages

     Post-Plan Actions

     After execution, I'll post a summary comment to Issue #1476 documenting:
     - What was internalized
     - What was removed
     - Current dependency tree
     - Next steps for ACP runtime completion
