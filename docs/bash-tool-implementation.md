# Bash Tool Implementation for Autonomous Coding Agents

## Overview

This document outlines the implementation plan for a Bash execution tool that will be used by our autonomous coding agents to run local commands. This tool is critical for the "Coding Overnight" MVP as it enables the agent to execute commands like linting, testing, and other local development workflows.

## Requirements

The Bash tool should:

1. Execute bash commands securely on the local machine
2. Integrate with our existing MCP architecture
3. Return command output and exit codes
4. Support environment variables and working directories
5. Handle timeouts appropriately
6. Provide appropriate error handling
7. Be usable by the Cloudflare Agent SDK

## Implementation Options

We have two main implementation options to consider:

### Option 1: Local MCP Server for Bash

Create a dedicated MCP server that runs locally and exposes a Bash execution tool:

```
┌───────────────┐     ┌──────────────────┐     ┌──────────────────┐
│               │     │                  │     │                  │
│  Coder App    │────▶│  MCP Client      │────▶│  Local Bash      │
│  (Electron)   │     │  Manager         │     │  MCP Server      │
│               │     │                  │     │                  │
└───────────────┘     └──────────────────┘     └──────────────────┘
```

**Pros:**
- Consistent with our MCP-based architecture
- Clean separation of concerns
- Can be extended to other local tools in the future
- Leverages existing MCP client/server code

**Cons:**
- Additional complexity of running a local server
- Requires handling process lifecycle management
- May involve cross-origin concerns in the local environment

### Option 2: Direct Bash Integration in Coder App

Implement a bash execution capability directly within the Electron app:

```
┌───────────────┐     ┌──────────────────┐
│               │     │                  │
│  Coder App    │────▶│  Node child_     │
│  (Electron)   │     │  process API     │
│               │     │                  │
└───────────────┘     └──────────────────┘
```

**Pros:**
- Simpler implementation (no need for MCP protocol)
- Direct access to Node.js child_process
- Easier debugging and error handling
- More control over security

**Cons:**
- Inconsistent with our MCP-based architecture
- Limited to Electron environment only
- Doesn't benefit from MCP's standardized interface

## Recommendation

For the MVP, we should implement **Option 1 (Local MCP Server for Bash)** to maintain consistency with our architecture and enable future extensibility.

## Implementation Plan

Here's how we'll implement the local Bash MCP server:

### 1. Create a new MCP server module

Create a new module in `apps/coder-desktop/src/mcp-servers/bash-server` with:

```typescript
import { McpAgent } from "agents/mcp";
import { McpServer } from "@modelcontextprotocol/sdk/server/mcp.js";
import { z } from "zod";
import { spawn } from "child_process";

export class BashMcpServer extends McpAgent {
  server = new McpServer({
    name: "Local Bash MCP",
    version: "0.0.1",
  });

  async init() {
    this.server.tool(
      "execute_bash",
      {
        command: z.string().describe("The bash command to execute"),
        cwd: z.string().optional().describe("Working directory for command execution"),
        timeout: z.number().optional().describe("Timeout in milliseconds"),
        env: z.record(z.string()).optional().describe("Additional environment variables"),
      },
      async (params: {
        command: string;
        cwd?: string;
        timeout?: number;
        env?: Record<string, string>;
      }) => {
        const { command, cwd, timeout, env } = params;
        
        try {
          // Implementation details for executing the bash command securely
          const result = await executeBashCommand(command, cwd, timeout, env);
          
          return {
            content: [{
              type: "text",
              text: JSON.stringify(result)
            }]
          };
        } catch (error) {
          console.error(`Bash execution error:`, error);
          
          return {
            content: [{
              type: "text",
              text: JSON.stringify({
                error: error instanceof Error ? error.message : String(error),
                exitCode: error.exitCode || null
              })
            }]
          };
        }
      }
    );
  }
}

// Helper function to execute bash commands
async function executeBashCommand(
  command: string,
  cwd?: string,
  timeout?: number, 
  env?: Record<string, string>
): Promise<{ stdout: string; stderr: string; exitCode: number }> {
  return new Promise((resolve, reject) => {
    // Validate command for security
    // This is critical to prevent malicious commands
    if (isMaliciousCommand(command)) {
      reject(new Error("Potentially unsafe command detected"));
      return;
    }

    // Merge environment variables
    const mergedEnv = { ...process.env, ...env };
    
    // Set up timeout handling
    const timeoutMs = timeout || 30000; // Default 30 second timeout
    let timeoutId: NodeJS.Timeout | null = null;
    
    // Spawn bash process
    const childProcess = spawn("bash", ["-c", command], {
      cwd: cwd || process.cwd(),
      env: mergedEnv,
      shell: true
    });
    
    let stdout = "";
    let stderr = "";
    
    childProcess.stdout.on("data", (data) => {
      stdout += data.toString();
    });
    
    childProcess.stderr.on("data", (data) => {
      stderr += data.toString();
    });
    
    childProcess.on("error", (error) => {
      if (timeoutId) clearTimeout(timeoutId);
      reject(error);
    });
    
    childProcess.on("close", (code) => {
      if (timeoutId) clearTimeout(timeoutId);
      resolve({
        stdout,
        stderr,
        exitCode: code || 0
      });
    });
    
    // Set up timeout
    if (timeout) {
      timeoutId = setTimeout(() => {
        childProcess.kill();
        reject(new Error(`Command timed out after ${timeoutMs}ms`));
      }, timeoutMs);
    }
  });
}

// Basic security check for commands
function isMaliciousCommand(command: string): boolean {
  // Blacklist approach - block known dangerous commands
  const blacklist = [
    /\brm\s+-rf\b/, // Delete files recursively
    /\bchmod\s+777\b/, // Insecure permissions
    /\bcurl\s+.*\|\s*sh\b/, // Pipe from internet to shell
    /\bwget\s+.*\|\s*sh\b/, // Pipe from internet to shell
    // Add more patterns as needed
  ];
  
  return blacklist.some(pattern => pattern.test(command));
}

export default {
  fetch: async (request: Request, env: any, ctx: any) => {
    const url = new URL(request.url);

    // Handle the homepage route
    if (url.pathname === "/") {
      return new Response("Local Bash MCP Server - Execute bash commands securely", {
        headers: { "Content-Type": "text/plain" },
      });
    }

    // Handle the SSE route
    return BashMcpServer.mount("/sse", {
      corsOptions: {
        origin: "*",
        methods: "GET,POST",
        headers: "*",
      },
    }).fetch(request, env, ctx);
  }
};
```

### 2. Set Up Local Server Launcher in Coder App

Add code to launch the Bash MCP server locally when the Coder app starts:

```typescript
// In apps/coder/src/main.ts or appropriate startup file

import { spawn } from 'child_process';
import { join } from 'path';

// Start the local Bash MCP server
export function startBashMcpServer() {
  const serverProcess = spawn('node', [
    join(__dirname, 'mcp-servers/bash-server/index.js')
  ], {
    stdio: 'pipe',
    env: { ...process.env, PORT: '8123' }  // Run on port 8123
  });
  
  serverProcess.stdout.on('data', (data) => {
    console.log(`Bash MCP Server: ${data}`);
  });
  
  serverProcess.stderr.on('data', (data) => {
    console.error(`Bash MCP Server Error: ${data}`);
  });
  
  // Handle server process termination
  serverProcess.on('close', (code) => {
    console.log(`Bash MCP Server process exited with code ${code}`);
    // Could implement restart logic here if needed
  });
  
  // Clean up on app exit
  process.on('exit', () => {
    serverProcess.kill();
  });
  
  return {
    url: 'http://localhost:8123/sse',
    process: serverProcess
  };
}
```

### 3. Register with MCP Client Manager

Configure the McpClientManager to connect to this local server:

```typescript
// In appropriate client setup code

import { mcpClientManager } from '@openagents/core/mcp/client';

export async function setupMcpTools() {
  // Connect to GitHub MCP server
  await mcpClientManager.connectToServer('https://mcp-github.openagents.com/sse', 'github');
  
  // Connect to local Bash MCP server
  await mcpClientManager.connectToServer('http://localhost:8123/sse', 'bash');
  
  // Log available tools
  const allTools = mcpClientManager.getAllTools();
  console.log('Available MCP tools:', allTools.map(t => t.name));
}
```

### 4. Security Considerations

Implementing a bash execution tool has significant security implications that require careful handling:

1. **Command Validation**: Implement robust validation to prevent dangerous commands
2. **Sandboxing**: Consider using containerization for extra security
3. **Resource Limits**: Enforce CPU/memory limits on executed commands
4. **User Permission**: Require explicit user approval for commands with potential impact
5. **Audit Logging**: Log all command executions for security review

### 5. Integration with Agent Flow

Update the agent execution flow to leverage the bash tool:

```typescript
// In autonomous coding agent implementation

async function runTestsStep(agent, repo, branch) {
  // Use the MCP bash tool to run tests
  try {
    const result = await mcpClientManager.callTool('execute_bash', {
      command: 'npm test',
      cwd: '/path/to/repo',
      timeout: 60000  // 60 second timeout
    });
    
    agent.recordResult('Tests completed', result);
    return result.exitCode === 0;
  } catch (error) {
    agent.recordError('Test execution failed', error);
    return false;
  }
}
```

## Testing Plan

1. **Unit Tests**: Test the bash execution logic in isolation
2. **Security Tests**: Test command validation and security mechanisms
3. **Integration Tests**: Test integration with the MCP architecture
4. **End-to-End Tests**: Test the full workflow with the agent using the bash tool

## Next Steps

1. Implement the Bash MCP server
2. Add security validations and command whitelisting
3. Test with simple commands
4. Integrate with the autonomous coding agent flow
5. Test with full overnight coding workflows

## Future Enhancements

1. **Command Templates**: Pre-defined templates for common operations
2. **Enhanced Security**: Advanced sandboxing and isolation
3. **Workflow Integration**: Integrate with CI/CD pipelines
4. **Parallel Execution**: Run multiple commands in parallel
5. **Progress Streaming**: Stream command output in real-time

By implementing this Bash tool as an MCP server, we maintain architectural consistency while enabling the powerful local execution capabilities needed for our autonomous coding agent's MVP.