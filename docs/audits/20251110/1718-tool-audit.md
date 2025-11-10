
  🔍 THOROUGH AUDIT OF FOUNDATION MODELS TOOL IMPLEMENTATION

  Critical Finding #1: CODE WILL NOT COMPILE ❌

  Location: OpenAgentsLocalProvider.swift:122

  Problem: Registration passes 4 arguments to a struct with no init:
  // Line 122: Tries to pass 4 arguments
  tools.append(FMTool_DelegateRun(sessionId: sessionId, updateHub: updateHub,
                                   workspaceRoot: workspaceRoot, server: server))

  // Lines 159-175: Struct has NO stored properties or init parameters
  struct FMTool_DelegateRun: Tool {
      let name = "delegate.run"
      // ... NO stored properties!
  }

  Impact: This is a compile error. The tool never gets registered, explaining why Foundation Models responds with "null" - the tool doesn't exist at
  runtime.

  ---
  Critical Finding #2: Pattern Mismatch with FMTools.swift

  Analysis of working tools in FMTools.swift:

  Pattern A (no dependencies):
  struct FMTool_SessionList: Tool {
      // NO stored properties
      func call(arguments a: Arguments) async throws -> Output {
          let tool = SessionListTool() // Instantiate separate implementation
          return tool.list(...)
      }
  }
  // Registration: tools.append(FMTool_SessionList()) ✅

  Pattern B (with dependencies):
  struct FMTool_ReadSpan: Tool {
      private let workspaceRoot: String  // ✅ Stored property
      init(workspaceRoot: String) { self.workspaceRoot = workspaceRoot }  // ✅ Custom init

      func call(arguments a: Arguments) async throws -> Output {
          let tool = ContentSpanTool(workspaceRoot: workspaceRoot)  // Use stored property
          return tool.readSpan(...)
      }
  }
  // Registration: tools.append(FMTool_ReadSpan(workspaceRoot: workspaceRoot)) ✅

  Our code: Tries to use Pattern B registration with Pattern A definition ❌

  ---
  Critical Finding #3: Missing Arguments Expected by UI

  Location: ToolCallView.swift:119 expects description argument

  // UI code (ToolCallView.swift):
  if let desc = obj["description"], case .string(let descStr) = desc {
      let shortDesc = descStr.components(separatedBy: " ").prefix(5).joined(separator: " ")
      return "Delegate to \(provider): \(shortDesc)"
  }

  // Current tool Arguments (OpenAgentsLocalProvider.swift):
  @Generable
  struct Arguments {
      var user_prompt: String
      var provider: String?
      // ❌ Missing: description
  }

  Impact: UI won't display delegate.run calls correctly.

  ---
  Critical Finding #4: Tests Reference Different Implementation

  Location: OpenAgentsLocalProviderFMToolTests.swift:47-62

  // Test expects FMTool_CodexRun with these arguments:
  let tool = OpenAgentsLocalProvider.FMTool_CodexRun(  // ❌ Doesn't exist
      sessionId: sid, updateHub: hub, workspaceRoot: ..., server: server
  )
  let args = OpenAgentsLocalProvider.FMTool_CodexRun.Arguments(
      task: "delegate",                    // ❌ Not in current Arguments
      description: "...",                   // ❌ Not in current Arguments
      user_prompt: "list files",            // ✅ Exists
      workspace_root: "...",                // ❌ Not in current Arguments
      files_include_glob: ["**/*"],         // ❌ Not in current Arguments
      summarize: true,                      // ❌ Not in current Arguments
      max_files: 100                        // ❌ Not in current Arguments
  )

  Impact: Tests fail immediately.

  ---
  Critical Finding #5: Missing Static Method

  Location: OpenAgentsLocalProviderFMToolTests.swift:80

  let text = OpenAgentsLocalProvider.FMTool_DelegateRun.composeDelegationPrompt(...)
  // ❌ This static method doesn't exist

  ---
  Apple Documentation Compliance ✅/❌

  | Requirement                     | Status | Notes                                      |
  |---------------------------------|--------|--------------------------------------------|
  | Conforms to Tool protocol       | ✅      | Correct                                    |
  | Has name property               | ✅      | "delegate.run"                             |
  | Has description property        | ✅      | Present                                    |
  | Has @Generable Arguments        | ✅      | Correct structure                          |
  | Has call(arguments:) method     | ✅      | Present                                    |
  | Output is PromptRepresentable   | ✅      | String works                               |
  | Tool is Sendable                | ✅      | Struct is implicitly Sendable              |
  | Registration matches definition | ❌      | CRITICAL MISMATCH                          |
  | Tool name convention            | ⚠️     | Uses "delegate.run" (ok per FMTools.swift) |
  | Argument names                  | ⚠️     | Uses snake_case (ok per FMTools.swift)     |

  ---
  Root Cause Analysis

  The code was simplified to remove stored properties (likely attempting Pattern A), but:

  1. ❌ Registration code at line 122 was not updated
  2. ❌ call() implementation needs those dependencies to do actual delegation
  3. ❌ Arguments struct was over-simplified, removing fields UI/tests expect
  4. ❌ Code never compiled, so Foundation Models never saw the tool
  5. ❌ "null" responses were because tool doesn't exist at runtime

  ---

───────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────
 Ready to code?

 Here is Claude's plan:
╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌
 Fix Foundation Models Tool Calling for delegate.run

 Problem

 Tool registration and definition are mismatched, causing compile errors. Foundation Models can't invoke a tool that doesn't compile.

 Changes Required

 1. Restore Pattern B structure to FMTool_DelegateRun:
   - Add stored properties: sessionId, updateHub, workspaceRoot, server
   - Add custom init to accept dependencies
   - Update call() to use stored properties for actual delegation logic
 2. Expand Arguments struct to match UI expectations:
   - Keep: user_prompt, provider
   - Add: description (required by ToolCallView.swift)
   - Consider: task, workspace_root, files_include_glob, summarize, max_files (required by tests)
 3. Fix registration at line 122 (already correct for Pattern B, just needs struct to match)
 4. Add composeDelegationPrompt static method (referenced by tests)
 5. Build and verify compilation succeeds
 6. Test tool invocation with simple prompt to confirm Foundation Models actually calls the tool

 Files to Modify

 - ios/OpenAgentsCore/Sources/OpenAgentsCore/Agents/OpenAgentsLocalProvider.swift

 Expected Outcome

 - Code compiles successfully
 - Foundation Models sees and can invoke delegate.run
 - Transcript shows toolCall entries when delegation is requested
 - UI renders tool calls correctly with provider and description
