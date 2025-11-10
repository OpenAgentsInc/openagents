eady to code?

 Here is Claude's plan:
╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌
 Plan: Swap MLX GPTOSS for llama.cpp + GGUF

 Overview

 Replace the slow MLX-based GPTOSS implementation with llama.cpp CLI for significantly faster inference. Use the existing CLIAgentProvider pattern (same as
 Codex/Claude Code).

 Phase 1: Model File Management

 1.1 Create Models Directory & Move GGUF

 - Create ~/.openagents/models/ directory for local model storage
 - Move ~/Downloads/gpt-oss-20b-MXFP4.gguf to ~/.openagents/models/gpt-oss-20b-MXFP4.gguf
 - This keeps models outside the repo and in a standard user directory

 1.2 Update .gitignore

 - Add *.gguf pattern to ensure GGUF files never get committed
 - Add ~/.openagents/models/ to documentation (not to .gitignore since it's outside repo)

 Phase 2: Implement LlamaAgentProvider

 2.1 Create New Provider

 File: ios/OpenAgentsCore/Sources/OpenAgentsCore/Agents/LlamaAgentProvider.swift

 - Extend CLIAgentProvider (same pattern as CodexAgentProvider/ClaudeCodeAgentProvider)
 - Implement:
   - buildStartArguments(): Construct llama-cli args (model path, temp, top-p, max tokens)
   - buildResumeArguments(): Rebuild context from history
   - processStdoutLine(): Stream plain text as ACP chunks
 - Model path discovery:
   a. Check env var OPENAGENTS_LLAMA_MODEL
   b. Default to ~/.openagents/models/gpt-oss-20b-MXFP4.gguf
 - GPU acceleration: Use -ngl 35 for Metal
 - Binary discovery: Uses "llama-cli" (already at /opt/homebrew/bin/llama-cli)

 2.2 Add Session Mode ID

 File: ios/OpenAgentsCore/Sources/OpenAgentsCore/Agents/ACPSessionModeId.swift

 - Add .llama_cpp case (replace or alongside .gptoss_20b)

 2.3 Register Provider

 File: ios/OpenAgentsCore/Sources/OpenAgentsCore/DesktopBridge/DesktopWebSocketServer.swift

 - Register LlamaAgentProvider in agent registry
 - Remove or comment out GPTOSSAgentProvider registration

 Phase 3: Remove MLX GPTOSS Implementation

 3.1 Delete GPTOSS Module

 - Delete ios/OpenAgentsCore/Sources/OpenAgentsCore/GPTOSS/ directory:
   - GPTOSSModelManager.swift
   - GPTOSSAgentProvider.swift
   - GPTOSSTypes.swift
   - (All other GPTOSS files)

 3.2 Remove MLX Dependencies from Package.swift

 File: ios/OpenAgentsCore/Package.swift

 - Keep MLX dependencies if embeddings still use them:
   - MLXEmbedders - Used by embeddings system
 - Remove GPTOSS-specific dependencies:
   - MLXLLM - Only used by GPTOSS
   - MLXLMCommon - Only used by GPTOSS
 - Keep Tokenizers from swift-transformers (may be used elsewhere)

 3.3 Remove ACPSessionModeId.gptoss_20b References

 - Remove .gptoss_20b enum case (or mark deprecated)
 - Search for any UI references to GPTOSS and update to Llama

 Phase 4: Update Documentation

 4.1 Update GPTOSS Docs

 Files in: docs/gptoss/

 Create new document: docs/gptoss/llama-cpp-migration.md
 - Document why we switched (performance: MLX too slow)
 - Benchmarks showing llama.cpp is 2-3x faster
 - Installation instructions for llama-cli
 - Model file management (where to put GGUF files)

 Update: docs/gptoss/README.md
 - Strike through or remove MLX references
 - Update to reflect llama.cpp approach
 - Update system requirements (no longer needs 16GB minimum - llama.cpp is more efficient)
 - Update performance expectations with llama.cpp benchmarks

 Update: docs/gptoss/gptoss-integration-spec.md
 - Add migration note at top
 - Mark as "Superseded by llama.cpp approach"

 4.2 Update Issues

 Files in: docs/gptoss/issues/

 - Mark Issues #1-10 as "Superseded by llama.cpp"
 - Create new issue: 011-llama-cpp-integration.md with simpler implementation plan

 4.3 Update CLAUDE.md

 Add section about model file management:
 - Where to put GGUF models (~/.openagents/models/)
 - How to configure model path (env var)
 - Binary requirements (llama-cli via Homebrew)

 Phase 5: Foundation Models Delegation

 5.1 Update gptoss.generate Tool

 File: ios/OpenAgentsCore/Sources/OpenAgentsCore/Agents/OpenAgentsLocalProvider.swift

 Option A: Rename to llama.generate
 Option B: Keep gptoss.generate name but route to llama.cpp

 Update tool to:
 - Set mode to .llama_cpp instead of .gptoss_20b
 - Pass temperature, max_tokens to llama-cli
 - Keep same delegation pattern

 Phase 6: Testing & Validation

 6.1 Manual Tests

 - Verify llama-cli works: llama-cli -m ~/.openagents/models/gpt-oss-20b-MXFP4.gguf -p "Hello" -n 50
 - Test streaming in macOS app
 - Test cancellation
 - Verify memory usage (should be lower than MLX)

 6.2 Performance Comparison

 - Benchmark first token latency (should be <1s)
 - Benchmark throughput (should be >50 t/s on M-series)
 - Compare to MLX numbers (should see 2-3x improvement)

 Phase 7: Cleanup & Polish

 7.1 Remove Dead Code

 - Delete any unused GPTOSS imports
 - Clean up any lingering references

 7.2 Update Settings UI (if exists)

 - Update any GPTOSS-specific UI to reference Llama
 - Model path configuration field
 - No more download progress UI (model is local)

 Key Files Changed

 Created:
 - ios/OpenAgentsCore/Sources/OpenAgentsCore/Agents/LlamaAgentProvider.swift
 - docs/gptoss/llama-cpp-migration.md
 - docs/gptoss/issues/011-llama-cpp-integration.md

 Modified:
 - .gitignore (add *.gguf)
 - ios/OpenAgentsCore/Package.swift (remove MLXLLM, MLXLMCommon)
 - ios/OpenAgentsCore/Sources/OpenAgentsCore/Agents/ACPSessionModeId.swift (add .llama_cpp)
 - ios/OpenAgentsCore/Sources/OpenAgentsCore/DesktopBridge/DesktopWebSocketServer.swift (register Llama provider)
 - ios/OpenAgentsCore/Sources/OpenAgentsCore/Agents/OpenAgentsLocalProvider.swift (update delegation tool)
 - docs/gptoss/README.md
 - docs/gptoss/gptoss-integration-spec.md
 - CLAUDE.md

 Deleted:
 - ios/OpenAgentsCore/Sources/OpenAgentsCore/GPTOSS/ (entire directory)

 Benefits

 1. Performance: 2-3x faster inference vs MLX (based on benchmarks)
 2. Simpler: No Hub downloads, no snapshot management, no memory pressure monitoring
 3. Proven Pattern: Uses existing CLIAgentProvider (same as Codex/Claude)
 4. User Control: Users can swap GGUF models easily
 5. Smaller Binary: Remove MLX LLM dependencies
 6. Lower Memory: llama.cpp with GGUF is more memory-efficient

 Risks & Mitigations

 Risk: Users need to install llama-cli
 Mitigation: Document in setup guide, provide Homebrew one-liner

 Risk: No session persistence
 Mitigation: Store conversation history in app, rebuild context on resume

 Risk: Plain text output (no structured tool calling)
 Mitigation: Can add later with JSON grammar/schema

 Implementation Order

 1. Move GGUF file & update .gitignore
 2. Create LlamaAgentProvider (small, focused implementation)
 3. Register provider & test manually
 4. Delete GPTOSS module
 5. Update Package.swift dependencies
 6. Update documentation
 7. Test end-to-end

 Estimated Time: 1-2 days (much simpler than MLX approach)
