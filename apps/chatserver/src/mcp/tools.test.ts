import { extractToolDefinitions, processToolCall } from './tools';
import { mcpClientManager } from './client';

// Mock the mcpClientManager
jest.mock('./client', () => ({
  mcpClientManager: {
    getAllTools: jest.fn().mockReturnValue([
      { name: 'create_issue', description: 'Creates an issue', server: 'github' },
      { name: 'get_file_contents', description: 'Gets file contents', server: 'github' }
    ]),
    callTool: jest.fn().mockImplementation((name, args, token) => {
      return Promise.resolve({ success: true, data: { toolName: name, args, token } });
    }),
    getToolServer: jest.fn().mockReturnValue('github')
  }
}));

describe('MCP Tools', () => {
  beforeEach(() => {
    jest.clearAllMocks();
  });
  
  describe('extractToolDefinitions', () => {
    it('should return predefined tool definitions', () => {
      const tools = extractToolDefinitions();
      
      // Verify we have the expected tools
      expect(tools.length).toBeGreaterThan(0);
      
      // Verify tool structure
      const tool = tools[0];
      expect(tool).toHaveProperty('name');
      expect(tool).toHaveProperty('description');
      expect(tool).toHaveProperty('parameters');
      expect(tool.parameters).toHaveProperty('type');
      expect(tool.parameters).toHaveProperty('properties');
      expect(tool.parameters).toHaveProperty('required');
    });
  });
  
  describe('processToolCall', () => {
    it('should process a tool call and return a result', async () => {
      const toolCall = {
        toolCallId: 'call123',
        toolName: 'create_issue',
        args: { owner: 'test', repo: 'repo', title: 'Issue Title' }
      };
      
      const authToken = 'github-token';
      
      const result = await processToolCall(toolCall, authToken);
      
      // Verify mcpClientManager was called correctly
      expect(mcpClientManager.callTool).toHaveBeenCalledWith(
        'create_issue',
        { owner: 'test', repo: 'repo', title: 'Issue Title' },
        'github-token'
      );
      
      // Verify result structure
      expect(result).toHaveProperty('toolCallId', 'call123');
      expect(result).toHaveProperty('toolName', 'create_issue');
      expect(result).toHaveProperty('args');
      expect(result).toHaveProperty('result');
    });
    
    it('should handle errors when processing a tool call', async () => {
      // Setup the mock to throw an error
      (mcpClientManager.callTool as jest.Mock).mockRejectedValueOnce(
        new Error('Tool execution failed')
      );
      
      const toolCall = {
        toolCallId: 'call123',
        toolName: 'create_issue',
        args: { owner: 'test', repo: 'repo', title: 'Issue Title' }
      };
      
      const result = await processToolCall(toolCall);
      
      // Verify error handling
      expect(result).toHaveProperty('toolCallId', 'call123');
      expect(result).toHaveProperty('result.error', 'Tool execution failed');
    });
    
    it('should return null for null input', async () => {
      const result = await processToolCall(null as any);
      expect(result).toBeNull();
    });
  });
});