/**
 * Specialized tool definitions for the Coder Agent
 * Includes tools for code analysis, repository management, and development operations
 */
import { tool } from "ai";
import { z } from "zod";
import { coderAgentContext } from "./coder-agent";

// HTTP client for calling MCP services
// Since we can't directly import from chatserver due to package boundaries,
// we'll implement a lightweight MCP client for tool calls
class MCPToolClient {
  private baseUrl: string;
  
  constructor(baseUrl: string = 'https://mcp-github.openagents.com/sse') {
    this.baseUrl = baseUrl;
  }

  /**
   * Call an MCP tool with parameters and optional auth token
   */
  async callTool(toolName: string, args: Record<string, any>, token?: string): Promise<any> {
    const callId = this.generateCallId();
    console.log(`🔄 [${callId}] Calling MCP tool '${toolName}'`);
    
    try {
      // Using JSON payload for tool calls
      const payload = {
        name: toolName,
        arguments: args,
        _meta: {
          ...(token ? { token } : {}),
          requestId: callId
        }
      };

      // Make a direct fetch request to the MCP GitHub server
      const response = await fetch('https://mcp-github.openagents.com/api/tool', {
        method: 'POST',
        headers: {
          'Content-Type': 'application/json',
          ...(token ? { 'Authorization': `Bearer ${token}` } : {})
        },
        body: JSON.stringify(payload)
      });

      if (!response.ok) {
        const errorText = await response.text();
        throw new Error(`MCP tool call failed with status ${response.status}: ${errorText}`);
      }

      const result = await response.json();
      console.log(`✅ [${callId}] MCP tool '${toolName}' succeeded`);
      return result;
    } catch (error) {
      console.error(`🚨 [${callId}] MCP tool '${toolName}' error:`, error);
      throw error;
    }
  }
  
  // Helper method to generate a call ID without crypto.randomUUID in global scope
  private generateCallId(): string {
    // Simple implementation that doesn't rely on crypto in global scope
    const timestamp = Date.now().toString(36);
    const random = Math.floor(Math.random() * 1000000).toString(36);
    return `${timestamp}-${random}`;
  }
}

// Instead of creating the client in global scope, we'll create a function to get it
function getMCPClient(): MCPToolClient {
  return new MCPToolClient();
}

/**
 * Get Repository Info
 * Returns basic information about a GitHub repository
 */
const getRepositoryInfo = tool({
  description: "Get information about a GitHub repository",
  parameters: z.object({
    owner: z.string().describe("The owner of the repository"),
    repo: z.string().describe("The name of the repository")
  }),
  execute: async ({ owner, repo }, options) => {
    console.log(`Getting info for repository ${owner}/${repo}`);
    
    try {
      // Create client on demand rather than in global scope
      const mcpClient = getMCPClient();
      
      // Extract auth token if available
      // TypeScript: Since headers isn't part of ToolExecutionOptions type, use type assertion
      const headers = (options as any)?.headers;
      const authToken = headers?.['X-GitHub-Token'] || 
                       (headers?.Authorization?.replace('Bearer ', ''));
      
      // Call the MCP tool via our client
      const result = await mcpClient.callTool('get_repository', {
        owner: owner,
        repo: repo
      }, authToken);
      
      return result;
    } catch (error) {
      console.error(`Failed to get repository info: ${error}`);
      return {
        error: `Failed to get repository info: ${error instanceof Error ? error.message : String(error)}`,
        // Fallback data
        name: repo,
        owner: owner,
        description: "Could not fetch repository information",
        defaultBranch: "main",
        isPrivate: false,
      };
    }
  }
});

/**
 * Set Project Context
 * Sets the current repository context for the agent to use in subsequent operations
 */
const setProjectContext = tool({
  description: "Set the current project/repository context for the agent",
  parameters: z.object({
    owner: z.string().describe("The owner of the repository"),
    repo: z.string().describe("The name of the repository"),
    branch: z.string().optional().describe("The branch to work with (optional)"),
    path: z.string().optional().describe("The base path within the repository (optional)")
  }),
  execute: async ({ owner, repo, branch, path }) => {
    const agent = coderAgentContext.getStore();
    if (!agent) {
      throw new Error("No agent context found");
    }
    
    agent.setProjectContext({
      repoOwner: owner,
      repoName: repo,
      branch: branch,
      path: path
    });
    
    return `Project context set to ${owner}/${repo}${branch ? ` (branch: ${branch})` : ''}${path ? ` (path: ${path})` : ''}`;
  }
});

/**
 * Get File Contents
 * Retrieves the contents of a file from a GitHub repository
 */
const getFileContents = tool({
  description: "Get the contents of a file from a GitHub repository",
  parameters: z.object({
    owner: z.string().optional().describe("The owner of the repository (optional if project context is set)"),
    repo: z.string().optional().describe("The name of the repository (optional if project context is set)"),
    path: z.string().describe("The path to the file within the repository"),
    ref: z.string().optional().describe("The git reference (branch, tag, commit) to get the file from (optional)")
  }),
  execute: async ({ owner, repo, path, ref }, options) => {
    // Get project context if owner or repo is not provided
    const agent = coderAgentContext.getStore();
    if (!agent) {
      throw new Error("No agent context found");
    }
    
    const context = agent.getProjectContext();
    
    // Use provided values or fall back to context
    const repoOwner = owner || context.repoOwner;
    const repoName = repo || context.repoName;
    const gitRef = ref || context.branch;
    
    if (!repoOwner || !repoName) {
      throw new Error("Repository owner and name must be provided either directly or via project context");
    }
    
    console.log(`Getting file: ${path} from ${repoOwner}/${repoName}${gitRef ? ` (ref: ${gitRef})` : ''}`);
    
    try {
      // Create client on demand rather than in global scope
      const mcpClient = getMCPClient();
      
      // Extract auth token if available
      // TypeScript: Since headers isn't part of ToolExecutionOptions type, use type assertion
      const headers = (options as any)?.headers;
      const authToken = headers?.['X-GitHub-Token'] || 
                       (headers?.Authorization?.replace('Bearer ', ''));
      
      // Call the MCP tool via our client
      const result = await mcpClient.callTool('get_file_contents', {
        owner: repoOwner,
        repo: repoName,
        path: path,
        ...(gitRef ? { ref: gitRef } : {})
      }, authToken);
      
      return result.content;
    } catch (error) {
      console.error(`Failed to get file contents: ${error}`);
      return `// Error fetching ${path} from ${repoOwner}/${repoName}:
// ${error instanceof Error ? error.message : String(error)}`;
    }
  }
});

/**
 * Search Code
 * Searches for code in the repository based on a query
 */
const searchCode = tool({
  description: "Search for code in a GitHub repository",
  parameters: z.object({
    owner: z.string().optional().describe("The owner of the repository (optional if project context is set)"),
    repo: z.string().optional().describe("The name of the repository (optional if project context is set)"),
    query: z.string().describe("The search query to find code"),
    path: z.string().optional().describe("Filter results to this path prefix")
  }),
  execute: async ({ owner, repo, query, path }, options) => {
    // Get project context if owner or repo is not provided
    const agent = coderAgentContext.getStore();
    if (!agent) {
      throw new Error("No agent context found");
    }
    
    const context = agent.getProjectContext();
    
    // Use provided values or fall back to context
    const repoOwner = owner || context.repoOwner;
    const repoName = repo || context.repoName;
    
    if (!repoOwner || !repoName) {
      throw new Error("Repository owner and name must be provided either directly or via project context");
    }
    
    console.log(`Searching for "${query}" in ${repoOwner}/${repoName}${path ? ` (path: ${path})` : ''}`);
    
    try {
      // Create client on demand rather than in global scope
      const mcpClient = getMCPClient();
      
      // Extract auth token if available
      // TypeScript: Since headers isn't part of ToolExecutionOptions type, use type assertion
      const headers = (options as any)?.headers;
      const authToken = headers?.['X-GitHub-Token'] || 
                       (headers?.Authorization?.replace('Bearer ', ''));
      
      // Call the MCP tool via our client
      const result = await mcpClient.callTool('search_code', {
        owner: repoOwner,
        repo: repoName,
        query: query,
        ...(path ? { path: path } : {})
      }, authToken);
      
      return result;
    } catch (error) {
      console.error(`Failed to search code: ${error}`);
      return {
        error: `Failed to search code: ${error instanceof Error ? error.message : String(error)}`,
        totalCount: 0,
        items: []
      };
    }
  }
});

/**
 * Run Command
 * Executes a shell command (this requires human confirmation)
 */
const runCommand = tool({
  description: "Run a shell command in the development environment",
  parameters: z.object({
    command: z.string().describe("The command to execute"),
    workingDirectory: z.string().optional().describe("The working directory to run the command in (optional)")
  }),
  // No execute function - this requires human confirmation
});

/**
 * Create File
 * Creates a new file in the repository (this requires human confirmation)
 */
const createFile = tool({
  description: "Create a new file in the repository",
  parameters: z.object({
    owner: z.string().optional().describe("The owner of the repository (optional if project context is set)"),
    repo: z.string().optional().describe("The name of the repository (optional if project context is set)"),
    path: z.string().describe("The path where the file should be created"),
    content: z.string().describe("The content of the file"),
    commitMessage: z.string().optional().describe("The commit message (optional)"),
    branch: z.string().optional().describe("The branch to commit to (optional)")
  }),
  // No execute function - this requires human confirmation
});

/**
 * Create Pull Request
 * Creates a pull request in the repository (this requires human confirmation)
 */
const createPullRequest = tool({
  description: "Create a pull request in the repository",
  parameters: z.object({
    owner: z.string().optional().describe("The owner of the repository (optional if project context is set)"),
    repo: z.string().optional().describe("The name of the repository (optional if project context is set)"),
    title: z.string().describe("The title of the pull request"),
    body: z.string().describe("The description of the pull request"),
    head: z.string().describe("The name of the branch where your changes are implemented"),
    base: z.string().describe("The name of the branch you want the changes pulled into")
  }),
  // No execute function - this requires human confirmation
});

/**
 * Export all available tools for the Coder Agent
 */
export const coderTools = {
  getRepositoryInfo,
  setProjectContext,
  getFileContents,
  searchCode,
  runCommand,
  createFile,
  createPullRequest
};

/**
 * Implementation of confirmation-required tools
 * Contains the actual logic for tools that need human approval
 */
export const coderExecutions = {
  runCommand: async ({ command, workingDirectory }: { command: string, workingDirectory?: string }) => {
    console.log(`Executing command: ${command} in directory: ${workingDirectory || 'current directory'}`);
    
    try {
      // For development purposes, just log the command
      // In production with the Electron app, this would use the command execution API
      console.log(`Would execute: ${command}`);
      
      // Format for command execution in ChatWithCommandSupport component
      return {
        command: command,
        output: `This command would be executed in the desktop client.\nIn the web version, command execution is limited.`,
        exitCode: 0
      };
    } catch (error) {
      console.error(`Command execution error: ${error}`);
      return {
        command: command,
        output: `Error: ${error instanceof Error ? error.message : String(error)}`,
        exitCode: 1
      };
    }
  },
  
  createFile: async ({ owner, repo, path, content, commitMessage, branch }: { 
    owner?: string, 
    repo?: string, 
    path: string, 
    content: string, 
    commitMessage?: string,
    branch?: string
  }) => {
    console.log(`Creating file at: ${path} with commit message: ${commitMessage || 'Create ' + path}`);
    
    try {
      // Get project context if owner or repo is not provided
      const agent = coderAgentContext.getStore();
      if (!agent) {
        throw new Error("No agent context found");
      }
      
      const context = agent.getProjectContext();
      
      // Use provided values or fall back to context
      const repoOwner = owner || context.repoOwner;
      const repoName = repo || context.repoName;
      const gitBranch = branch || context.branch;
      
      if (!repoOwner || !repoName) {
        throw new Error("Repository owner and name must be provided either directly or via project context");
      }
      
      // For now, just log what would happen
      // In production, this would call the GitHub API via MCP after human approval
      return {
        message: `File would be created at ${path} in ${repoOwner}/${repoName}`,
        path: path,
        commit: {
          message: commitMessage || `Create ${path}`,
          branch: gitBranch || 'main'
        }
      };
    } catch (error) {
      console.error(`File creation error: ${error}`);
      return {
        error: `Failed to create file: ${error instanceof Error ? error.message : String(error)}`
      };
    }
  },
  
  createPullRequest: async ({ owner, repo, title, body, head, base }: { 
    owner?: string, 
    repo?: string, 
    title: string, 
    body: string, 
    head: string, 
    base: string 
  }) => {
    console.log(`Creating PR: ${title} from ${head} to ${base}`);
    
    try {
      // Get project context if owner or repo is not provided
      const agent = coderAgentContext.getStore();
      if (!agent) {
        throw new Error("No agent context found");
      }
      
      const context = agent.getProjectContext();
      
      // Use provided values or fall back to context
      const repoOwner = owner || context.repoOwner;
      const repoName = repo || context.repoName;
      
      if (!repoOwner || !repoName) {
        throw new Error("Repository owner and name must be provided either directly or via project context");
      }
      
      // For now, just log what would happen
      // In production, this would call the GitHub API via MCP after human approval
      return {
        message: `Pull request would be created in ${repoOwner}/${repoName}`,
        number: 123, // Mock PR number
        title: title,
        html_url: `https://github.com/${repoOwner}/${repoName}/pull/123`,
        head: head,
        base: base
      };
    } catch (error) {
      console.error(`Pull request creation error: ${error}`);
      return {
        error: `Failed to create pull request: ${error instanceof Error ? error.message : String(error)}`
      };
    }
  }
};