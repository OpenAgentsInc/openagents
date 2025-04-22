import { it, describe, expect, vi } from 'vitest';
import { Effect, Layer } from 'effect';
import { GitHubTools } from '../src/AiService.js';
import { ContentBlockToolUse } from '../src/types.js';

describe('Tool Execution', () => {
  // Create a mock implementation for tool use content block
  const mockToolUseContent: ContentBlockToolUse = {
    type: 'tool_use',
    tool_use: {
      id: 'tool_12345',
      name: 'GetGitHubFileContent',
      input: {
        owner: 'openagentsinc',
        repo: 'openagents',
        path: 'README.md'
      }
    }
  };

  it('should correctly extract tool parameters from ContentBlockToolUse', () => {
    // Extract parameters as done in Server.ts
    const toolUse = mockToolUseContent.tool_use;
    const toolName = toolUse.name;
    const toolInput = toolUse.input;
    const toolUseId = toolUse.id;
    
    // Verify extracted values
    expect(toolName).toBe('GetGitHubFileContent');
    expect(toolInput).toEqual({
      owner: 'openagentsinc',
      repo: 'openagents',
      path: 'README.md'
    });
    expect(toolUseId).toBe('tool_12345');
  });
  
  it('should correctly handle file content tool with null ref parameter', () => {
    // Create a test case with optional ref parameter as undefined
    const fileParams = {
      owner: 'openagentsinc',
      repo: 'openagents',
      path: 'README.md',
      ref: undefined
    };
    
    // This should not error out due to undefined ref
    expect(() => {
      // Convert to string as the Server.ts does
      const params = {
        owner: String(fileParams.owner),
        repo: String(fileParams.repo),
        path: String(fileParams.path),
        ref: fileParams.ref ? String(fileParams.ref) : undefined
      };
      
      // Verify that the conversion happened correctly
      expect(params.owner).toBe('openagentsinc');
      expect(params.repo).toBe('openagents');
      expect(params.path).toBe('README.md');
      expect(params.ref).toBeUndefined();
    }).not.toThrow();
  });
  
  it('should convert tool input parameters to the correct types', () => {
    // Mock a tool use with various parameter types
    const mockIssueToolUse: ContentBlockToolUse = {
      type: 'tool_use',
      tool_use: {
        id: 'tool_67890',
        name: 'GetGitHubIssue',
        input: {
          owner: 'openagentsinc',
          repo: 'openagents',
          issueNumber: '42' // String instead of number to test conversion
        }
      }
    };
    
    // Extract and convert as done in Server.ts
    const toolUse = mockIssueToolUse.tool_use;
    const toolInput = toolUse.input;
    
    const issueParams = {
      owner: String(toolInput.owner),
      repo: String(toolInput.repo),
      issueNumber: Number(toolInput.issueNumber)
    };
    
    // Verify conversion
    expect(issueParams.owner).toBe('openagentsinc');
    expect(issueParams.repo).toBe('openagents');
    expect(issueParams.issueNumber).toBe(42);
    expect(typeof issueParams.issueNumber).toBe('number');
  });
});