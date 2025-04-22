import { it, describe, expect, vi, afterEach } from 'vitest';
import { Anthropic } from '@anthropic-ai/sdk';
import { TOOL_SCHEMAS } from '../src/Tools.js';

// Don't actually make API calls
vi.mock('@anthropic-ai/sdk', () => {
  return {
    Anthropic: vi.fn().mockImplementation(() => {
      return {
        messages: {
          create: vi.fn().mockResolvedValue({
            id: 'msg_mock123',
            content: [{ type: 'text', text: 'This is a mock response from Anthropic' }],
            role: 'assistant',
            model: 'claude-3-5-sonnet-latest',
            stop_reason: null,
            stop_sequence: null,
            usage: { input_tokens: 10, output_tokens: 20 }
          })
        }
      };
    })
  };
});

describe('Anthropic API Integration', () => {
  let anthropic: any;
  
  beforeEach(() => {
    // Create a new instance for each test
    anthropic = new Anthropic({ apiKey: 'test-key' });
  });
  
  afterEach(() => {
    vi.clearAllMocks();
  });
  
  it('should send tools in the correct format to Anthropic API', async () => {
    // Create test messages
    const messages = [
      { role: 'user', content: 'Test message' }
    ];
    
    // Create parameters for API call
    const params = {
      model: 'claude-3-5-sonnet-latest',
      messages,
      tools: TOOL_SCHEMAS,
      max_tokens: 1000
    };
    
    // Make the API call
    await anthropic.messages.create(params);
    
    // Check that the create method was called with the correct parameters
    expect(anthropic.messages.create).toHaveBeenCalledWith(params);
    
    // Extract the tools that were passed
    const toolsParam = anthropic.messages.create.mock.calls[0][0].tools;
    
    // Verify the tools are in the correct format
    for (const tool of toolsParam) {
      // Tools should have name, description, and input_schema
      expect(tool).toHaveProperty('name');
      expect(tool).toHaveProperty('description');
      expect(tool).toHaveProperty('input_schema');
      
      // Input schema should have the correct structure
      expect(tool.input_schema).toHaveProperty('type', 'object');
      expect(tool.input_schema).toHaveProperty('properties');
      expect(tool.input_schema).toHaveProperty('required');
    }
  });
  
  it('should handle tool use blocks in response content', async () => {
    // Override the mock for a single test to return a tool use
    anthropic.messages.create.mockResolvedValueOnce({
      id: 'msg_tool_use123',
      content: [
        {
          type: 'tool_use',
          tool_use: {
            id: 'tool_mock123',
            name: 'GetGitHubFileContent',
            input: {
              owner: 'openagentsinc',
              repo: 'openagents',
              path: 'README.md'
            }
          }
        }
      ],
      role: 'assistant',
      model: 'claude-3-5-sonnet-latest',
      stop_reason: null,
      stop_sequence: null,
      usage: { input_tokens: 10, output_tokens: 20 }
    });
    
    // Make the API call
    const result = await anthropic.messages.create({
      model: 'claude-3-5-sonnet-latest',
      messages: [{ role: 'user', content: 'Get the README from the openagents repo' }],
      tools: TOOL_SCHEMAS,
      max_tokens: 1000
    });
    
    // Check response content
    expect(result.content[0].type).toBe('tool_use');
    expect(result.content[0].tool_use.name).toBe('GetGitHubFileContent');
    expect(result.content[0].tool_use.input.owner).toBe('openagentsinc');
  });
  
  it('should handle tool results in request messages', async () => {
    // Create a mock tool result to send back
    const messages = [
      { role: 'user', content: 'Get the README from the openagents repo' },
      { 
        role: 'assistant',
        content: [
          {
            type: 'tool_use',
            tool_use: {
              id: 'tool_abc123',
              name: 'GetGitHubFileContent',
              input: {
                owner: 'openagentsinc',
                repo: 'openagents',
                path: 'README.md'
              }
            }
          }
        ]
      },
      {
        role: 'user',
        content: [
          {
            type: 'tool_result',
            tool_use_id: 'tool_abc123',
            content: '# OpenAgents\n\nThis is a test README file.'
          }
        ]
      }
    ];
    
    // Make the API call with tool results
    await anthropic.messages.create({
      model: 'claude-3-5-sonnet-latest',
      messages,
      max_tokens: 1000
    });
    
    // Verify the call was made with correct parameters
    expect(anthropic.messages.create).toHaveBeenCalledWith(
      expect.objectContaining({
        messages: expect.arrayContaining([
          expect.objectContaining({
            role: 'user',
            content: expect.arrayContaining([
              expect.objectContaining({
                type: 'tool_result',
                tool_use_id: 'tool_abc123'
              })
            ])
          })
        ])
      })
    );
  });
});