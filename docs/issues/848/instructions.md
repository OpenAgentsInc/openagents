# Agent Instructions: Issue #848 - Audit and Refactor Application Initialization Flow

**Objective:** Address issue #848 by auditing the current application initialization flow, identifying root causes for reported problems (database issues, MCP client timing, window activation, code structure), and refactoring the code for improved stability, robustness, maintainability, and adherence to Electron best practices.

**Workflow:**

1.  **Branch:** Create a new branch for your work, named descriptively (e.g., `fix/848-init-refactor`).
2.  **Understand the Problem:**
    *   Thoroughly review the description and comments on issue #848.
    *   Carefully study the existing documentation:
        *   `docs/initialization.md` (Primary flow description)
        *   `docs/mcp-client-initialization.md` (MCP singleton context)
        *   `docs/rxdb-setup-learnings.md` (Database context)
        *   `docs/error-handling.md` (Error handling patterns)
    *   Examine the code files identified in the comments of #848 (e.g., `main.ts`, `entry.tsx`, `db.ts` helpers, MCP client files, `preload.ts`, `App.tsx`, etc.) to trace the *actual* current initialization sequence, including asynchronous operations.
3.  **Analysis and Planning:**
    *   Based on your review, document your findings regarding the root causes of the reported issues (DB locks/wipes, MCP timing, window activation, code smells).
    *   Outline a detailed plan for refactoring. This should include:
        *   Proposed changes to the initialization sequence.
        *   How database initialization will be made more robust (e.g., centralized logic, improved lock handling).
        *   How MCP client initialization will be synchronized correctly.
        *   How the macOS window activation issue will be addressed.
        *   Specific files to be refactored and how (e.g., breaking down large files, improving error propagation).
        *   How Electron best practices (context isolation, preload usage, etc.) will be applied.
    *   **Create new markdown file(s) within the `docs/issues/848/` directory** (e.g., `docs/issues/848/analysis_and_plan.md`) to store this analysis and plan.
4.  **Request Feedback:** **Before proceeding with significant implementation**, add a comment to issue #848 summarizing your findings and proposed plan (linking to the file(s) you created in the previous step). Ask for user feedback on the proposed approach.
5.  **Implementation:**
    *   Once the plan is approved or refined based on feedback, implement the necessary code changes on your branch.
    *   Focus on addressing all points mentioned in the scope of issue #848.
    *   Ensure clear, maintainable code with appropriate comments.
    *   Apply robust error handling for all initialization steps.
6.  **Testing:**
    *   Thoroughly test the application startup sequence on different platforms (especially macOS for the window issue).
    *   Specifically test scenarios related to the reported problems:
        *   Simulate database lock conditions if possible.
        *   Test startup with potential network delays affecting MCP clients.
        *   Verify data persistence across restarts.
        *   Test cold and warm starts.
7.  **Documentation Updates:**
    *   Keep notes or a log of your work in a file within `docs/issues/848/` (e.g., `docs/issues/848/implementation_log.md`).
    *   If the initialization flow changes significantly, update `docs/initialization.md` to accurately reflect the new sequence and patterns.
8.  **Pull Request:**
    *   When the implementation is complete, tested, and documented, open a pull request targeting the `main` branch.
    *   Ensure the pull request description clearly explains the changes made and references issue #848 (e.g., "Closes #848").
