import { McpClientManager } from './client';
import { Client } from "@modelcontextprotocol/sdk/client/index.js";

// Use Jest's mocking capabilities
jest.mock("@modelcontextprotocol/sdk/client/index.js");
jest.mock("@modelcontextprotocol/sdk/client/sse.js");

describe('McpClientManager', () => {
  let manager: McpClientManager;
  
  // Mock implementation for MCP Client
  const mockListTools = jest.fn().mockResolvedValue([
    { name: 'tool1', description: 'Tool 1 description' },
    { name: 'tool2', description: 'Tool 2 description' }
  ]);
  
  const mockCallTool = jest.fn().mockImplementation(({ name, arguments: args }) => {
    return Promise.resolve({
      content: [
        { 
          type: 'text', 
          text: JSON.stringify({ result: `Called ${name} with ${JSON.stringify(args)}` }) 
        }
      ]
    });
  });

  beforeEach(() => {
    jest.clearAllMocks();
    
    // Setup MCP client mock behavior
    (Client as jest.Mock).mockImplementation(() => ({
      connect: jest.fn().mockResolvedValue(undefined),
      close: jest.fn().mockResolvedValue(undefined),
      listTools: mockListTools,
      callTool: mockCallTool
    }));
    
    manager = new McpClientManager();
  });

  it('should connect to an MCP server', async () => {
    const client = await manager.connectToServer('https://test-server.com/sse', 'test-server');
    
    expect(Client).toHaveBeenCalled();
    expect(client.connect).toHaveBeenCalled();
    expect(client.listTools).toHaveBeenCalled();
  });

  it('should discover tools from server', async () => {
    await manager.connectToServer('https://test-server.com/sse', 'test-server');
    
    // Get all tools
    const tools = manager.getAllTools();
    
    expect(tools).toHaveLength(2);
    expect(tools[0].name).toBe('tool1');
    expect(tools[1].name).toBe('tool2');
    expect(tools[0].server).toBe('test-server');
  });

  it('should find the server for a specific tool', async () => {
    await manager.connectToServer('https://test-server.com/sse', 'test-server');
    
    const serverName = manager.getToolServer('tool1');
    
    expect(serverName).toBe('test-server');
  });

  it('should call a tool and return result', async () => {
    const client = await manager.connectToServer('https://test-server.com/sse', 'test-server');
    
    const result = await manager.callTool('tool1', { param1: 'value1' });
    
    expect(client.callTool).toHaveBeenCalledWith({
      name: 'tool1',
      arguments: { param1: 'value1' }
    });
    
    expect(result).toEqual({ result: 'Called tool1 with {"param1":"value1"}' });
  });

  it('should include token when provided', async () => {
    const client = await manager.connectToServer('https://test-server.com/sse', 'test-server');
    
    await manager.callTool('tool1', { param1: 'value1' }, 'auth-token');
    
    expect(client.callTool).toHaveBeenCalledWith({
      name: 'tool1',
      arguments: { param1: 'value1', token: 'auth-token' }
    });
  });

  it('should throw error for unknown tool', async () => {
    await manager.connectToServer('https://test-server.com/sse', 'test-server');
    
    await expect(manager.callTool('unknown-tool', {}))
      .rejects.toThrow('Tool unknown-tool not found in any connected MCP server');
  });

  it('should disconnect from all servers', async () => {
    const client = await manager.connectToServer('https://test-server.com/sse', 'test-server');
    
    await manager.disconnectAll();
    
    expect(client.close).toHaveBeenCalled();
    
    // Should be empty after disconnect
    expect(manager.getAllTools()).toHaveLength(0);
  });
});