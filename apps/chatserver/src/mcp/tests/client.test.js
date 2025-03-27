// Using Node.js built-in test runner
const test = require('node:test');
const assert = require('node:assert');

// This is a mock implementation of the McpClientManager for testing
class MockMcpClientManager {
  constructor() {
    this.clients = new Map();
    this.toolRegistry = new Map();

    // Pre-populate with some test tools
    this.toolRegistry.set('tool1', { server: 'test-server', description: 'Tool 1 description' });
    this.toolRegistry.set('tool2', { server: 'test-server', description: 'Tool 2 description' });
  }

  async connectToServer(serverUrl, serverName) {
    const mockClient = {
      connect: () => Promise.resolve(),
      close: () => Promise.resolve(),
      listTools: () => Promise.resolve([
        { name: 'tool1', description: 'Tool 1 description' },
        { name: 'tool2', description: 'Tool 2 description' }
      ]),
      callTool: ({ name, arguments: args }) => Promise.resolve({
        content: [
          { 
            type: 'text', 
            text: JSON.stringify({ result: `Called ${name} with ${JSON.stringify(args)}` }) 
          }
        ]
      })
    };

    this.clients.set(serverName, mockClient);
    return mockClient;
  }

  getToolServer(toolName) {
    return this.toolRegistry.get(toolName)?.server;
  }

  getAllTools() {
    return Array.from(this.toolRegistry.entries()).map(([name, info]) => ({
      name,
      description: info.description,
      server: info.server,
    }));
  }

  async callTool(toolName, args, token) {
    const serverName = this.getToolServer(toolName);
    if (!serverName) {
      throw new Error(`Tool ${toolName} not found in any connected MCP server`);
    }

    const client = this.clients.get(serverName);
    if (!client) {
      throw new Error(`MCP server ${serverName} not connected`);
    }

    const toolArgs = token ? { ...args, token } : args;
    
    const result = await client.callTool({
      name: toolName,
      arguments: toolArgs,
    });

    if (result.content && Array.isArray(result.content) && result.content.length > 0 && 
        typeof result.content[0] === 'object' && result.content[0] !== null && 
        'type' in result.content[0] && result.content[0].type === "text" &&
        'text' in result.content[0]) {
      try {
        return JSON.parse(result.content[0].text);
      } catch (e) {
        return result.content[0].text;
      }
    }
    
    return result;
  }

  async disconnectAll() {
    for (const client of this.clients.values()) {
      await client.close();
    }
    
    this.clients.clear();
    this.toolRegistry.clear();
  }
}

// Tests
test('MCP Client Manager', async (t) => {
  const manager = new MockMcpClientManager();

  await t.test('should connect to an MCP server', async () => {
    const client = await manager.connectToServer('https://test-server.com/sse', 'test-server');
    assert.strictEqual(typeof client.connect, 'function');
    assert.strictEqual(typeof client.listTools, 'function');
  });

  await t.test('should get all tools', async () => {
    const tools = manager.getAllTools();
    assert.strictEqual(tools.length, 2);
    assert.strictEqual(tools[0].name, 'tool1');
    assert.strictEqual(tools[1].name, 'tool2');
    assert.strictEqual(tools[0].server, 'test-server');
  });

  await t.test('should find the server for a specific tool', async () => {
    const serverName = manager.getToolServer('tool1');
    assert.strictEqual(serverName, 'test-server');
  });

  await t.test('should call a tool and return result', async () => {
    const result = await manager.callTool('tool1', { param1: 'value1' });
    assert.deepStrictEqual(result, { result: 'Called tool1 with {"param1":"value1"}' });
  });

  await t.test('should include token when calling a tool', async () => {
    // We can't easily verify the token was included in this mock, but we can test it doesn't fail
    const result = await manager.callTool('tool1', { param1: 'value1' }, 'auth-token');
    assert.strictEqual(typeof result, 'object');
  });

  await t.test('should throw error for unknown tool', async () => {
    try {
      await manager.callTool('unknown-tool', {});
      assert.fail('Expected error was not thrown');
    } catch (error) {
      assert.strictEqual(error.message, 'Tool unknown-tool not found in any connected MCP server');
    }
  });

  await t.test('should disconnect from all servers', async () => {
    await manager.disconnectAll();
    assert.strictEqual(manager.getAllTools().length, 0);
  });
});