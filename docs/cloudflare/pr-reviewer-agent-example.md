![Mall-Cop Workflow Overview](docs/cloudflare/mallcop.png)

# Mall-Cop™ Workflow Overview

**Mall-Cop™** is an AI Agent that reviews PRs made against your repos, analyzes them, and suggests fixes when there's room for improvement.

---

## Workflow Diagram and Process

The following describes the operational flow of the Mall-Cop™ system as depicted in the diagram:

### 1. Trigger Event (GitHub Repository)

*   **Component:** `user/big-project` (Represents a GitHub repository)
*   **Action:** The process initiates when a `pull_request.opened` or `pull_request.edited` event occurs within the specified GitHub repository.

### 2. Agent Activation and Coordination

*   **Component:** `agent-{user}/{repo}` (Represents multiple Agent instances, one per repository)
*   **Role:** Each Agent serves as a task coordinator for a specific repository.
*   **Responsibilities:**
    *   Agents track all Pull Requests (PRs) for their assigned repository.
    *   They manage potential race conditions.
    *   They ensure state is maintained consistently on a per-repository basis.
*   **Action:** Upon receiving the trigger event from GitHub, the relevant Agent initiates the code review process by invoking a workflow:
    ```javascript
    await this.codeReviewWorkflow()
    ```
*   **Scheduling:** The Agent is responsible for managing the lifecycle of review Workflows. It can:
    *   Schedule N-many Workflows.
    *   Poll the status of active Workflows (e.g., using `this.schedule(X, "checkWorkflowStatus", {})`).
    *   Cancel Workflows as needed.

### 3. Workflow Execution

*   **Component:** `workflow-{pr_number}` (Represents multiple Workflow instances, potentially many per Agent/PR)
*   **Initiation:** Scheduled and managed by an Agent.
*   **Responsibilities (Each Workflow):**
    1.  **Parse PR:** Analyzes the content and changes within the specific Pull Request (`{pr_number}`).
    2.  **Construct Prompt:** Creates a detailed prompt suitable for an AI model based on the PR data.
    3.  **Call Model:** Sends the prompt to an AI model for code review and analysis.
    4.  **Parse Response:** Receives the (structured) response from the AI model.
    5.  **Comment on PR:** Posts the review findings, suggestions, and analysis back as comments on the original Pull Request in the GitHub repository. This action is conceptually represented by:
        ```javascript
        this.writeReviewComment
        ```

### 4. Feedback Loop and State Update

*   **Action:** Workflow status is reported back to the originating Agent.
*   **Agent Response:** The Agent receives the status update and performs the following:
    *   Reviews the status.
    *   Resolves any conflicts (e.g., if the PR was updated concurrently).
    *   Updates its own local state regarding the PR's review status. This is represented by:
        ```javascript
        this.setState(updatedPRState)
        ```

---

## Summary of Components and Roles

*   **GitHub Repository:** The source of PRs and the target for review comments. Triggers the process via webhooks (`pull_request` events).
*   **Agent (`agent-{user}/{repo}`):** A per-repository coordinator. Manages state, handles events, and schedules/monitors Workflows.
*   **Workflow (`workflow-{pr_number}`):** A task-specific process. Handles the detailed work of parsing a PR, interacting with the AI model, and reporting results back to the PR and the Agent.
