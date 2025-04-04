# Agent Instructions: Issue #852 - Refactor apps/coder/src/server/server.ts and Solidify Error Handling

**Objective:** Address issue #852 by refactoring the monolithic `apps/coder/src/server/server.ts` file into smaller, more focused modules and implementing the improved error handling strategy outlined in `docs/error-refactor.md`. The goal is to enhance maintainability, testability, robustness, and provide clearer error feedback to the user.

**Workflow:**

1.  **Branch:** Create a new branch for your work, named descriptively (e.g., `refactor/852-server-error-handling`). Check out the `errorrefactor` branch first and create your new branch from there.
2.  **Understand the Problem:**
    *   Thoroughly review the description and comments on issue #852.
    *   Carefully study the existing documentation on the `errorrefactor` branch:
        *   `docs/error-handling.md` (Current error display patterns)
        *   `docs/error-refactor.md` (Analysis of problems and proposed refactoring strategy)
    *   Deeply analyze the current structure and logic within `apps/coder/src/server/server.ts` (on the `errorrefactor` branch) to understand its different responsibilities (routing, provider logic, tool handling, error management, etc.).
3.  **Analysis and Planning:**
    *   Based on your review and the recommendations in `docs/error-refactor.md`, outline a detailed plan for refactoring `server.ts`. This should include:
        *   Proposed new file structure within `apps/coder/src/server/` (e.g., `routes/`, `providers/`, `streaming/`, `errors/`, `utils/`).
        *   How responsibilities will be divided among the new modules (e.g., provider client setup, streamText configuration, tool execution logic, error formatting).
        *   How the new centralized error handling system (potentially leveraging `packages/core/src/chat/errors/` as suggested) will be integrated.
        *   Specific strategies for handling different error types (API errors, tool errors, validation errors, etc.) consistently.
        *   How the error information will be propagated to the client-side stream, aligning with the patterns in `docs/error-handling.md`.
    *   **Create a new markdown file within `docs/issues/852/` directory** (e.g., `docs/issues/852/refactor_plan.md`) on your branch to store this detailed plan.
4.  **Request Feedback:** **Before proceeding with significant implementation**, add a comment to issue #852 summarizing your proposed refactoring plan (linking to the `refactor_plan.md` file you created). Ask for user feedback on the proposed approach.
5.  **Implementation:**
    *   Once the plan is approved or refined based on feedback, implement the refactoring on your branch.
    *   Break down `server.ts` into the planned modules.
    *   Implement the new error handling classes and logic.
    *   Integrate the new error system throughout the request lifecycle.
    *   Ensure the `/api/chat` endpoint functions correctly with the refactored structure.
    *   Focus on creating clean, well-documented, and testable modules.
    *   Apply robust and consistent error handling as per the plan.
6.  **Testing:**
    *   Thoroughly test the `/api/chat` endpoint with various scenarios:
        *   Different models and providers (OpenRouter, Anthropic, Google, Ollama - if configured).
        *   Conversations involving tool use (MCP tools, shell command).
        *   Scenarios designed to trigger errors (invalid API keys, non-existent models, context length exceeded, tool execution failures, network issues if possible).
        *   Verify that errors are correctly classified, propagated, and displayed in the chat UI according to `docs/error-handling.md`.
        *   Ensure normal chat functionality remains unaffected.
7.  **Documentation Updates:**
    *   Keep notes or a log of your work in a file within `docs/issues/852/` (e.g., `docs/issues/852/implementation_log.md`).
    *   Update `docs/error-handling.md` and `docs/error-refactor.md` if the final implementation differs significantly from the initial plan or reveals new insights.
    *   Add comments within the new code modules explaining their purpose and logic.
8.  **Pull Request:**
    *   When the implementation is complete, tested, and documented, open a pull request targeting the `errorrefactor` branch (or `main` if `errorrefactor` has been merged).
    *   Ensure the pull request description clearly explains the refactoring performed, the new structure, the error handling improvements, and references issue #852 (e.g., "Closes #852").
