Following the successful implementation of the initial Cloudflare Agent connection in **Issue #804**, this issue proposes a significant refactoring of the `packages/core/src/chat/useChat.ts` hook.

The primary goal is to align our custom `useChat` hook more closely with the internal design and patterns of the official `useAgentChat` hook provided by the `agents` SDK (`agents/ai-react`), thereby simplifying the implementation, improving efficiency, and ensuring better long-term maintainability and compatibility with the SDK.

**Current Implementation (`useChat.ts` as of closing #804):**

*   Conditionally uses the official `useAgent` and `useAgentChat` hooks when an `agentId` is provided.
*   *Simultaneously* calls `useChat` from `@ai-sdk/react` (aliased as `vercelUseChat`).
*   Switches between the outputs (`messages`, `append`, `isLoading`, `error`, etc.) of `agentChat` and `vercelUseChat` based on whether an agent is being used (`shouldUseAgent && agentConnection.isConnected`).
*   Manually implements initial message fetching for agents using `agent.call('getMessages')` in a `useEffect` because the default `getInitialMessages` caused CORS issues.
*   Layers custom functionality (local command execution, agent RPC calls like `executeAgentCommand`, `fetchMessages`, connection state) on top of this dual-hook structure.

**Shortcomings of the Current Approach:**

1.  **Complexity:** Managing two separate chat state instances (`agentChat` and `vercelUseChat`) and conditionally selecting outputs adds significant complexity and potential for subtle state inconsistencies.
2.  **Missed Optimization:** It does not leverage the official `useAgentChat` hook's internal `fetch` override (`aiFetch`). This override cleverly tunnels the AI requests (that `@ai-sdk/react`'s `useChat` would normally make via HTTP) over the *existing agent WebSocket connection*. Our current approach likely makes separate HTTP requests when using `vercelUseChat` and potentially uses a different mechanism within `agentChat.append`.
3.  **SDK Divergence:** Our manual layering approach is less aligned with the SDK's intended integration pattern, potentially making it harder to adopt future SDK features or improvements.
4.  **Initial Message Loading:** While our `useEffect`-based workaround for initial messages functions, the official hook uses React 19's `use()` for a more integrated asynchronous loading pattern (though we currently cannot use this directly due to needing `getInitialMessages: null`).

**Target Implementation Pattern (Based on `agents/ai-react` source):**

*   The official `useAgentChat` *internally* calls `@ai-sdk/react`'s `useChat`.
*   It provides a custom `fetch` function (`aiFetch`) to this internal `useChat` call, tunneling requests over the agent's WebSocket.
*   It handles initial message loading (ideally using `use()`, but we'll need our workaround).
*   It sets up listeners on the agent connection to synchronize state changes initiated by the agent (`cf_agent_chat_clear`, `cf_agent_chat_messages`).
*   It overrides `setMessages` and `clearHistory` to send synchronization messages back to the agent.

**Proposed Refactoring Plan:**

1.  **Unify Hook Usage:**
    *   Modify `useChat` to primarily rely on `useAgentChat` when `shouldUseAgent` is true.
    *   Conditionally call `useChat` from `@ai-sdk/react` (`vercelUseChat`) *only* when `shouldUseAgent` is false to handle the non-agent case. Ensure hook calls remain unconditional at the top level.
2.  **Adopt `useAgentChat` Output:** When the agent is active (`shouldUseAgent && agentConnection.isConnected`), use the `messages`, `append`, `setMessages`, `reload`, `stop`, `isLoading`, `error` etc., returned directly from the `useAgentChat` instance as the primary source for the hook's return value.
3.  **Integrate Initial Message Fetch:** Retain the `useEffect`-based workaround for fetching initial messages (`agent.call('getMessages')`). Ensure it correctly uses the `setMessages` function returned by the *primary `useAgentChat` instance*.
4.  **Integrate Custom Features (Agent Active):** Layer our existing custom features onto the `useAgentChat` results:
    *   `agentConnection` state.
    *   `executeAgentCommand`, `fetchMessages`, `testCommandExecution`.
    *   Debug properties (`isAgentConnected`, etc.).
5.  **Handle Non-Agent Case:** When `shouldUseAgent` is false:
    *   Return the results from the conditionally invoked `vercelUseChat`.
    *   Ensure local command execution logic (parsing, execution via `safeExecuteCommand`, message updates using `processedMessages` state and `updateMessage` callback) functions correctly based on the `vercelUseChat` state. The `useEffect` for local command processing needs to be adapted to *only* operate on the `vercelUseChat` results and only run when the agent is *not* active.
6.  **Maintain Hook Rules:** Ensure all hooks (`useAgent`, `useAgentChat`, `vercelUseChat`, `useEffect`, etc.) are called unconditionally at the top level of `useChat`. Use conditional logic *inside* the hook to determine which results to use and which effects to run.
7.  **Type Safety:** Continue efforts to use proper types (`Message` from `ai`) and minimize `any`, adding `// TODO:` comments where SDK limitations force workarounds.
8.  **Testing:** Thoroughly test both agent and non-agent modes, including initial message load, sending/receiving messages, local command execution (non-agent mode), agent command execution, connection stability, and state consistency on reload.

**Expected Benefits:**

*   Reduced code complexity and improved readability.
*   Leveraging the `agents` SDK's efficient WebSocket tunneling for AI requests (when agent is active).
*   Closer alignment with the official SDK, improving future compatibility.
*   Potentially improved state synchronization and reliability.

**Potential Challenges:**

*   Cleanly separating the logic for the agent vs. non-agent paths while respecting hook rules.
*   Ensuring the local command execution logic integrates smoothly with the non-agent (`vercelUseChat`) path without interfering with the agent path.
*   Maintaining full compatibility with the existing `UseChatReturn` type and custom functionalities.
*   Continuing to manage type compatibility issues between different libraries (`ai`, `@ai-sdk/react`, `agents`).

**Related Issues:**

*   Closes #804 (Initial Agent Connection) - This refactor builds upon the work done in #804.
