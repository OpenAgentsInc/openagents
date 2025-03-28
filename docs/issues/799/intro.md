# Local Command Execution Support for useChat

## Issue Understanding

Issue #799 is about adding local command execution support to the `useChat` wrapper to enable autonomous coding agents. The primary requirement is to allow the AI agent to execute bash commands on the local machine, with appropriate security measures in place.

## Key Requirements

1. **Extended useChat Hook**: Modify the `useChat` hook in `packages/core/src/chat/useChat.ts` to support a `localCommandExecution` flag.

2. **Command Parser**: Implement functionality to parse messages for command execution XML tags (`<execute-command>...</execute-command>`).

3. **Command Executor**: Create a secure utility to execute commands on the host machine (using Node.js in Electron context).

4. **Security Measures**: Implement safety checks to prevent dangerous commands.

5. **Integration with UI**: Update the UI components to display command execution results.

## Technical Approach

Our approach will be based on the design outlined in the `bash-tool-implementation.md` document, but instead of creating a separate MCP server, we'll integrate the command execution directly into the `useChat` hook to simplify the implementation for the MVP. This approach aligns with "Option 2: Direct Bash Integration in Coder App" from the design document.

### Implementation Steps

1. Extend the `useChat` hook to support a new `localCommandExecution` option.

2. Create a utility function to execute commands securely using Node.js `child_process`.

3. Implement a message parser that looks for special command execution tags.

4. Create a middleware that intercepts message handling to detect and execute commands.

5. Add UI components to display the command execution status and results.

### Security Considerations

- Command validation will block dangerous operations
- Optionally prompt the user for confirmation before executing commands
- Apply execution timeouts to prevent long-running commands
- Sanitize command output before displaying it

### Technical Challenges

1. **Electron Integration**: Ensuring the command execution only works in the Electron context (where Node.js APIs are available) and gracefully fails in web contexts.

2. **Security**: Implementing robust security checks to prevent malicious commands.

3. **Cross-Platform Compatibility**: Ensuring commands work properly across different operating systems.

4. **Error Handling**: Properly capturing and displaying command errors and execution issues.

This implementation will provide a foundation for the autonomous coding agent to execute local commands in a controlled and secure manner, enabling it to perform tasks like running tests, installing dependencies, and interacting with the local development environment.