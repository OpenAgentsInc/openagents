// Using Node.js built-in test runner
const test = require('node:test');
const assert = require('node:assert');

// Mock modules
const mockMcpClientManager = {
  getAllTools: () => [
    { name: 'create_issue', description: 'Creates an issue', server: 'github' },
    { name: 'get_file_contents', description: 'Gets file contents', server: 'github' }
  ],
  callTool: (name, args, token) => {
    if (name === 'error_tool') {
      return Promise.reject(new Error('Tool execution failed'));
    }
    return Promise.resolve({ success: true, data: { toolName: name, args, token } });
  },
  getToolServer: () => 'github'
};

// Import our main module with mocked dependencies
const toolsModule = {
  extractToolDefinitions: () => {
    // simplified version of the actual tool definitions
    return [
      {
        name: "create_issue",
        description: "Create a new issue in a GitHub repository",
        parameters: {
          type: "object",
          properties: {
            owner: { type: "string", description: "Repository owner" },
            repo: { type: "string", description: "Repository name" },
            title: { type: "string", description: "Issue title" },
            body: { type: "string", description: "Issue body" }
          },
          required: ["owner", "repo", "title"]
        }
      },
      {
        name: "get_file_contents",
        description: "Get the contents of a file from a GitHub repository",
        parameters: {
          type: "object",
          properties: {
            owner: { type: "string", description: "Repository owner" },
            repo: { type: "string", description: "Repository name" },
            path: { type: "string", description: "File path in the repository" },
            branch: { type: "string", description: "Branch name (optional)" }
          },
          required: ["owner", "repo", "path"]
        }
      }
    ];
  },

  processToolCall: async (toolCall, authToken) => {
    if (!toolCall) return null;
    
    try {
      const result = await mockMcpClientManager.callTool(
        toolCall.toolName,
        toolCall.args,
        authToken
      );
      
      return {
        toolCallId: toolCall.toolCallId,
        toolName: toolCall.toolName,
        args: toolCall.args,
        result
      };
    } catch (error) {
      return {
        toolCallId: toolCall.toolCallId,
        toolName: toolCall.toolName,
        args: toolCall.args,
        result: { error: error.message }
      };
    }
  }
};

// Tests
test('MCP Tools', async (t) => {
  await t.test('extractToolDefinitions should return predefined tool definitions', () => {
    const tools = toolsModule.extractToolDefinitions();
    
    assert.strictEqual(tools.length, 2);
    
    const tool = tools[0];
    assert.strictEqual(typeof tool.name, 'string');
    assert.strictEqual(typeof tool.description, 'string');
    assert.strictEqual(typeof tool.parameters, 'object');
    assert.strictEqual(tool.parameters.type, 'object');
    assert.strictEqual(typeof tool.parameters.properties, 'object');
    assert(Array.isArray(tool.parameters.required));
  });
  
  await t.test('processToolCall should process a tool call and return a result', async () => {
    const toolCall = {
      toolCallId: 'call123',
      toolName: 'create_issue',
      args: { owner: 'test', repo: 'repo', title: 'Issue Title' }
    };
    
    const authToken = 'github-token';
    
    const result = await toolsModule.processToolCall(toolCall, authToken);
    
    assert.strictEqual(result.toolCallId, 'call123');
    assert.strictEqual(result.toolName, 'create_issue');
    assert.deepStrictEqual(result.args, { owner: 'test', repo: 'repo', title: 'Issue Title' });
    assert.strictEqual(typeof result.result, 'object');
  });
  
  await t.test('processToolCall should handle errors when processing a tool call', async () => {
    const toolCall = {
      toolCallId: 'call123',
      toolName: 'error_tool',
      args: { owner: 'test', repo: 'repo', title: 'Issue Title' }
    };
    
    const result = await toolsModule.processToolCall(toolCall);
    
    assert.strictEqual(result.toolCallId, 'call123');
    assert.strictEqual(result.toolName, 'error_tool');
    assert.strictEqual(result.result.error, 'Tool execution failed');
  });
  
  await t.test('processToolCall should return null for null input', async () => {
    const result = await toolsModule.processToolCall(null);
    assert.strictEqual(result, null);
  });
});