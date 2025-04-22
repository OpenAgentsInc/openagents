import { it, describe, expect } from 'vitest';
import { 
  ExtendedMessageParam, 
  ContentBlockToolUse, 
  ContentBlockToolResult,
  ExtendedContentBlock,
  ToolDefinition,
  CustomTextBlock
} from '../src/types.js';

describe('Types', () => {
  it('should properly structure ExtendedMessageParam', () => {
    // Create a user message
    const userMessage: ExtendedMessageParam = {
      role: 'user',
      content: 'This is a test message'
    };
    
    // Validate structure
    expect(userMessage.role).toBe('user');
    expect(userMessage.content).toBe('This is a test message');
    
    // Create an assistant message with content blocks
    const assistantMessage: ExtendedMessageParam = {
      role: 'assistant',
      content: [
        { 
          type: 'text', 
          text: 'This is a text block',
          citations: null 
        }
      ]
    };
    
    // Validate structure
    expect(assistantMessage.role).toBe('assistant');
    expect(Array.isArray(assistantMessage.content)).toBe(true);
    
    const contentItem = assistantMessage.content[0] as CustomTextBlock;
    expect(contentItem.type).toBe('text');
  });
  
  it('should properly structure ContentBlockToolUse', () => {
    // Create a tool use block
    const toolUseBlock: ContentBlockToolUse = {
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
    
    // Validate structure
    expect(toolUseBlock.type).toBe('tool_use');
    expect(toolUseBlock.tool_use.id).toBe('tool_12345');
    expect(toolUseBlock.tool_use.name).toBe('GetGitHubFileContent');
    expect(toolUseBlock.tool_use.input.owner).toBe('openagentsinc');
  });
  
  it('should properly structure ContentBlockToolResult', () => {
    // Create a tool result block
    const toolResultBlock: ContentBlockToolResult = {
      type: 'tool_result',
      tool_use_id: 'tool_12345',
      content: 'This is the result of the tool execution'
    };
    
    // Validate structure
    expect(toolResultBlock.type).toBe('tool_result');
    expect(toolResultBlock.tool_use_id).toBe('tool_12345');
    expect(toolResultBlock.content).toBe('This is the result of the tool execution');
  });
  
  it('should properly structure ToolDefinition', () => {
    // Create a tool definition matching the latest Anthropic API format
    const toolDef: ToolDefinition = {
      name: 'GetGitHubFileContent',
      description: 'Fetches a file from GitHub',
      input_schema: {
        type: 'object',
        properties: {
          owner: {
            type: 'string',
            description: 'Repository owner'
          },
          repo: {
            type: 'string',
            description: 'Repository name'
          },
          path: {
            type: 'string',
            description: 'File path'
          }
        },
        required: ['owner', 'repo', 'path']
      }
    };
    
    // Validate structure
    expect(toolDef.name).toBe('GetGitHubFileContent');
    expect(toolDef.description).toBe('Fetches a file from GitHub');
    expect(toolDef.input_schema.type).toBe('object');
    expect(toolDef.input_schema.properties.owner.type).toBe('string');
    expect(toolDef.input_schema.required).toContain('path');
  });
  
  it('should properly handle union type ExtendedContentBlock', () => {
    // Create instances of each content block type
    const textBlock: ExtendedContentBlock = { 
      type: 'text', 
      text: 'This is text',
      citations: null
    };
    
    const toolUseBlock: ExtendedContentBlock = {
      type: 'tool_use',
      tool_use: {
        id: 'tool_12345',
        name: 'GetGitHubFileContent',
        input: { owner: 'test', repo: 'test', path: 'test.md' }
      }
    };
    
    const toolResultBlock: ExtendedContentBlock = {
      type: 'tool_result',
      tool_use_id: 'tool_12345',
      content: 'This is the result'
    };
    
    // Verify each can be treated as the union type
    expect(textBlock.type).toBe('text');
    expect(toolUseBlock.type).toBe('tool_use');
    expect(toolResultBlock.type).toBe('tool_result');
    
    // Test type narrowing
    function processBlock(block: ExtendedContentBlock): string {
      if (block.type === 'text') {
        return block.text;
      } else if (block.type === 'tool_use') {
        return block.tool_use.name;
      } else if (block.type === 'tool_result') {
        return block.content;
      }
      return '';
    }
    
    expect(processBlock(textBlock)).toBe('This is text');
    expect(processBlock(toolUseBlock)).toBe('GetGitHubFileContent');
    expect(processBlock(toolResultBlock)).toBe('This is the result');
  });
});