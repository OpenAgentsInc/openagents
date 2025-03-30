This comment provides a detailed breakdown of the required changes for refactoring the `@openagents/core` `useChat` hook (in `packages/core/src/chat/useChat.ts`) to align with the official `useAgentChat` pattern, as outlined in the main issue description (#IssueNumber).

The goal is to simplify the hook by primarily using `useAgentChat` when an agent is active and `useChat` from `@ai-sdk/react` (aliased as `vercelChat` in the current code) only when an agent is *not* active, instead of running both side-by-side all the time.

**File to Edit:** `packages/core/src/chat/useChat.ts`

**Detailed Changes:**

1.  **Unconditional Hook Calls (Top Level):**
    *   Maintain the unconditional calls to `useAgent`, `useAgentChat`, and `useChat` (`vercelUseChat`) at the top level to comply with React Hook rules.
    *   Ensure `useAgentChat` receives necessary options (`agent`, `getInitialMessages: null`, etc., passed down via `chatOptions`).
    *   Ensure `vercelUseChat` receives necessary options (`api`, etc., passed down via `chatOptions`).

    ```typescript
    // ~ line 150 onwards
    export function useChat(options: UseChatWithCommandsOptions = {}): UseChatReturn {
        // ... options destructuring ...
        const shouldUseAgent = Boolean(agentId || ...);
        const [agentConnection, setAgentConnection] = useState(...);
        const initialMessagesFetchedRef = useRef(false);
        const agentConfigRef = useRef(...);

        // --- Ensure these hooks are called unconditionally ---
        const agent = useAgent({ /* ... options ... */ });

        const agentChat = useAgentChat({
            agent,
            initialMessages: chatOptions.initialMessages, // Pass initial messages
            getInitialMessages: null, // Keep workaround for CORS
            // Pass other relevant options from chatOptions (e.g., id, onFinish, onError?)
            // Check UseAgentChatOptions type for compatibility
            id: options.id, // Ensure ID is passed if available
            onFinish: options.onFinish,
            onError: options.onError, // Let useAgentChat handle its errors initially
            // Do NOT pass body, headers, api here as it uses WebSocket fetch override
        });

        const vercelChat = useChat({ // Rename from vercelUseChat if not already done
            // Pass options relevant for standard HTTP API chat
            api: options.api || "https://chat.openagents.com", // Default or from options
            id: options.id,
            initialInput: options.initialInput,
            initialMessages: chatOptions.initialMessages,
            headers: options.headers,
            body: options.body,
            streamProtocol: options.streamProtocol,
            sendExtraMessageFields: options.sendExtraMessageFields,
            onFinish: options.onFinish, // Can potentially be shared
            onError: options.onError,   // Can potentially be shared
            // Do NOT pass agent/agentOptions here
        });
        // --- End Unconditional Hook Calls ---

        // ... rest of the hook setup ...
    }
    ```

2.  **Introduce `isAgentActive` Flag:**
    *   Define a boolean constant near the top (after hook calls) to simplify conditional logic.

    ```typescript
    // ~ line after hook calls
    // Determine if the agent is the active mode *for this render*
    const isAgentActive = shouldUseAgent && agentConnection.isConnected;
    ```

3.  **Adapt Initial Message Fetch Effect:**
    *   This `useEffect` (around line 293 in current code) should remain mostly the same.
    *   **Crucially:** Ensure it continues to use `agentChat.setMessages` to populate the state managed by the `useAgentChat` hook.
    *   Add the `isAgentActive` check within the `if` condition for clarity.
    *   Keep the dependency array as determined previously (`[isAgentActive, agent, agentChat.setMessages, chatOptions.initialMessages]`).

    ```typescript
    // ~ line 293 onwards (Effect to Fetch Initial Agent Messages)
    useEffect(() => {
      // Ensure agent is the active mode before fetching
      if (isAgentActive && agent && agentChat.setMessages && !initialMessagesFetchedRef.current) {
        initialMessagesFetchedRef.current = true;
        console.log('📄 USECHAT: Agent active, attempting to fetch initial messages...');
        agent.call('getMessages')
          .then((fetchedMessages: unknown) => {
            const typedMessages = fetchedMessages as Message[]; // Using Message from 'ai'
            // ... rest of .then logic using agentChat.setMessages ...
          })
          .catch((error: Error) => {
             // ... .catch logic using agentChat.setMessages for fallback ...
          });
      }
      // Reset ref if agent becomes inactive
      if (!isAgentActive) {
          initialMessagesFetchedRef.current = false;
      }
      // Ensure dependencies are correct
    }, [isAgentActive, agent, agentChat.setMessages, chatOptions.initialMessages]);
    ```

4.  **Refactor Local Command Processing Effect:**
    *   This `useEffect` (around line 480 in current code) must **only** run and operate on `vercelChat` state when the agent is *not* active (`!isAgentActive`).
    *   It should read messages from `vercelChat.messages`.
    *   It should update messages using `vercelChat.setMessages` (preferred for replacing content) or potentially `vercelChat.append` (for adding new result messages). Using `setMessages` requires careful state management to avoid infinite loops. Appending might be safer but changes the chat flow. **Let's target using `vercelChat.setMessages` first.**
    *   The `processSingleMessage` internal function needs to be adapted accordingly.
    *   The `processedMessages` state variable and `updateMessage` callback might become redundant if we directly manipulate `vercelChat.messages` via `vercelChat.setMessages`. **Aim to remove `processedMessages` state.**

    ```typescript
    // ~ line 480 onwards (Effect for Local Command Processing)
    const processedMessageIds = useRef<Set<string>>(new Set()); // Keep refs for tracking
    const executedCommands = useRef<Set<string>>(new Set());

    useEffect(() => {
      // Run only if agent is NOT active AND local execution is enabled
      if (isAgentActive || !localCommandExecution) {
        processedMessageIds.current.clear(); // Clear tracking if switching mode
        executedCommands.current.clear();
        return;
      }

      // Use messages directly from vercelChat
      const currentMessages = vercelChat.messages as UIMessage[]; // Cast may be needed
      if (currentMessages.length === 0) return;

      console.log("⚙️ USECHAT: Processing local commands (agent not active)");

      const processSingleMessage = async (messageToProcess: UIMessage) => {
          // ... check if already processed, check role/content, parse commands ...
          if (processedMessageIds.current.has(messageToProcess.id) || messageToProcess.role !== 'assistant' || typeof messageToProcess.content !== 'string') return;
          const commands = parseCommandsFromMessage(messageToProcess.content);
          if (commands.length === 0) return;

          processedMessageIds.current.add(messageToProcess.id);
          console.log(`🚀 USECHAT: Processing ${commands.length} local commands for message ${messageToProcess.id}`);

          const commandResults: Array<{ command: string; result: string | { error: string } }> = [];
          for (const command of commands) {
              // ... execute command using safeExecuteCommand, track executedCommands ...
               const commandKey = `${messageToProcess.id}-${command}`;
               if (!executedCommands.current.has(commandKey)) {
                    executedCommands.current.add(commandKey);
                    try {
                        const result = await safeExecuteCommand(command, commandOptions);
                        // ... format result ...
                        commandResults.push({ command, result: formattedResult });
                    } catch (error) { /* ... handle error ... */ }
               }
          }

          if (commandResults.length === 0) return;

          // --- Update message content using vercelChat.setMessages ---
          const updateKey = `update-${messageToProcess.id}`;
          if (!executedCommands.current.has(updateKey)) {
              executedCommands.current.add(updateKey);
              const updatedContent = replaceCommandTagsWithResults(messageToProcess.content, commandResults);
              if (updatedContent !== messageToProcess.content) {
                  console.log(`🔄 USECHAT: Updating message ${messageToProcess.id} with local command results via setMessages.`);
                  // Create a new messages array with the updated content
                  const updatedMessagesArray = currentMessages.map(msg =>
                      msg.id === messageToProcess.id ? { ...msg, content: updatedContent } : msg
                  );
                  vercelChat.setMessages(updatedMessagesArray); // Use the setter from vercelChat
              }
          }
      };

      // Process relevant messages from vercelChat.messages
      const processNewMessages = async () => {
          const assistantMessages = currentMessages.filter(m => m.role === 'assistant');
          const unprocessedMessages = assistantMessages.filter(msg => !processedMessageIds.current.has(msg.id));

          if (unprocessedMessages.length > 0) {
              for (const message of unprocessedMessages) {
                  await processSingleMessage(message);
              }
          }
      };

      processNewMessages();

    }, [
        isAgentActive, // Primary condition
        localCommandExecution,
        vercelChat.messages, // Source messages
        vercelChat.setMessages, // Setter function
        commandOptions,
        onCommandStart, // Callbacks passed from options
        onCommandComplete,
    ]);
    ```

5.  **Refactor `append` Function:**
    *   Conditionally call `agentChat.append` or `vercelChat.append` based on `isAgentActive`.
    *   Remove local command parsing logic from here if it's fully handled in the effect.

    ```typescript
    // ~ line 377 onwards (append function)
    const append = useCallback(async (message: any /* TODO: Type */) => {
      if (isAgentActive && agentChat?.append) {
        console.log('📤 USECHAT: Appending via agentChat');
        return agentChat.append(message);
      } else if (!isAgentActive && vercelChat?.append) {
        console.log('📤 USECHAT: Appending via vercelChat');
        // Local command parsing from user input (if any) was removed here,
        // assuming commands are only parsed from *assistant* responses in the effect.
        // If user commands need immediate parsing, logic would go here.
        return vercelChat.append(message);
      } else {
        console.error("❌ USECHAT: Cannot append. No active chat implementation available.");
        return null;
      }
    }, [isAgentActive, agentChat, vercelChat]); // Dependencies check
    ```

6.  **Refactor `handleSubmit`:**
    *   The `handleSubmit` function returned by `useAgentChat` and `vercelChat` is tied to their internal state and `append` methods. We need a custom `handleSubmit` that uses our combined `append`.

    ```typescript
    // Define this after the combined `append` is defined
    const inputRef = useRef<string>(''); // Ref to track input state manually or derive from activeChat.input

    // Update inputRef when active input changes
    const activeInput = isAgentActive ? agentChat?.input : vercelChat?.input;
    useEffect(() => {
        inputRef.current = activeInput ?? '';
    }, [activeInput]);

    // Custom handleSubmit
    const handleSubmit = useCallback((e?: React.FormEvent<HTMLFormElement>) => {
        e?.preventDefault();
        const messageToSend = inputRef.current; // Get value from ref or activeChat.input
        if (!messageToSend) return;

        console.log(`📤 USECHAT: handleSubmit called. Active: ${isAgentActive}. Message: "${messageToSend}"`);
        // Call the combined append function
        append({ role: 'user', content: messageToSend });

        // Manually clear input using the *active* chat's setter
        if (isAgentActive && agentChat?.setInput) {
            agentChat.setInput('');
        } else if (!isAgentActive && vercelChat?.setInput) {
            vercelChat.setInput('');
        }
        // Reset ref just in case
        inputRef.current = '';

    }, [append, isAgentActive, agentChat, vercelChat]); // Dependencies check
    ```

7.  **Rebuild `returnValue`:**
    *   Conditionally select the core properties (`messages`, `isLoading`, `error`, `input`, `handleInputChange`, `setMessages`, `reload`, `stop`) from `agentChat` or `vercelChat` based on `isAgentActive`.
    *   Provide the custom `append` and `handleSubmit` created above.
    *   Add the custom agent-specific utilities (`agentConnection`, `fetchMessages`, `executeAgentCommand`, `testCommandExecution`) and the combined `executeCommand`.
    *   Remove `processedMessages` state from the return value if it was successfully eliminated.

    ```typescript
    // ~ line 758 onwards (Prepare return value)
    const activeChat = isAgentActive ? agentChat : vercelChat;

    const returnValue = {
      // Core properties from the *active* hook
      messages: activeChat.messages,
      isLoading: activeChat.isLoading,
      error: activeChat.error,
      input: activeChat.input,
      handleInputChange: activeChat.handleInputChange, // Use the one from the active hook
      setMessages: activeChat.setMessages,
      reload: activeChat.reload,
      stop: activeChat.stop,

      // Our custom/combined functions
      append: append, // Our combined append
      handleSubmit: handleSubmit, // Our combined handleSubmit

      // Custom agent-specific utilities (always included)
      agentConnection: agentConnection,
      fetchMessages: fetchMessages, // Agent RPC
      executeAgentCommand: executeAgentCommand, // Agent RPC (with fallback if needed)
      testCommandExecution: testCommandExecution, // Tests both

      // Combined command executor
      executeCommand: isAgentActive
        ? executeAgentCommand // Uses agent command with fallback
        : (command: string) => { // Non-agent path
            if (localCommandExecution) {
              return safeExecuteCommand(command, commandOptions);
            } else {
              console.error("❌ USECHAT: Local command execution disabled.");
              return Promise.reject("Local command execution disabled.");
            }
          },

       // Debug properties (always included)
       // ... defined via Object.defineProperties ...
    };

    Object.defineProperties(returnValue, { /* ... debug properties ... */ });

    return returnValue as UseChatReturn; // Keep final cast for now
    ```
