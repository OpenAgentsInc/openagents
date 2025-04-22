import { it, describe, expect, vi, afterEach, beforeEach } from 'vitest';
import { TOOL_SCHEMAS } from '../src/Tools.js';

// Mock the entire Anthropic module
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

// Mock the Effect module to avoid actual Effect executions
vi.mock('effect', () => {
  // Create a mock Tag class with TypeScript types
  class MockTag {
    name: string;
    constructor(name: string) {
      this.name = name;
    }
  }
  
  // Add the 'of' method to the MockTag class instead of prototype
  class MockTagWithOf extends MockTag {
    of() { 
      return {}; 
    }
  }
  
  return {
    Effect: {
      gen: vi.fn().mockImplementation(fn => fn()),
      runPromise: vi.fn().mockResolvedValue({ toolUseId: 'tool_123', toolResult: 'Mock tool result' }),
      provide: vi.fn().mockReturnValue({}),
      match: vi.fn().mockImplementation((_effect, handlers) => handlers.onSuccess('Mock success')),
      catchAll: vi.fn().mockReturnValue({}),
      fail: vi.fn().mockReturnValue({}),
      succeed: vi.fn().mockReturnValue({}),
      Tag: vi.fn().mockImplementation((name: string) => {
        return function() {
          return new MockTagWithOf(name);
        };
      })
    },
    Layer: {
      provide: vi.fn().mockReturnValue({}),
      merge: vi.fn().mockReturnValue({}),
      succeed: vi.fn().mockReturnValue({})
    },
    Console: {
      log: vi.fn().mockReturnValue({}),
      error: vi.fn().mockReturnValue({}),
      warn: vi.fn().mockReturnValue({})
    }
  };
});

// Mock readline module
vi.mock('node:readline/promises', () => {
  return {
    createInterface: vi.fn().mockReturnValue({
      question: vi.fn().mockResolvedValue('mock user input'),
      close: vi.fn()
    })
  };
});

// Mock dotenv
vi.mock('dotenv', () => {
  return {
    config: vi.fn()
  };
});

// Use vi.hoisted to ensure mocks are available before imports
const mocksSetUp = vi.hoisted(() => {
  return {
    processUserMessage: vi.fn().mockResolvedValue(undefined)
  };
});

// Explicitly mock the GitHubTools import from AiService
vi.mock('../src/AiService.js', () => {
  return {
    GitHubTools: {
      of: vi.fn().mockReturnValue({})
    },
    GitHubToolsLive: {},
    gitHubClientLayers: {}
  };
});

describe('Server', () => {
  // Mock console methods to inspect output
  const consoleLogSpy = vi.spyOn(console, 'log').mockImplementation(() => {});
  const stdoutWriteSpy = vi.spyOn(process.stdout, 'write').mockImplementation(() => true);
  
  // Import the module after setting up mocks - use an interface to avoid unused variable warning
  interface ServerMock {
    processUserMessage: (message: string) => Promise<void>;
  }
  
  // No need to store a variable if it's never used
  beforeEach(() => {
    // Initialize mock function for each test
    mocksSetUp.processUserMessage.mockReset();
  });
  
  afterEach(() => {
    consoleLogSpy.mockReset();
    stdoutWriteSpy.mockReset();
    vi.clearAllMocks();
  });
  
  it('should validate the tool schemas format', () => {
    // Check if the tools have the correct format for the latest Anthropic API
    for (const tool of TOOL_SCHEMAS) {
      expect(tool).toHaveProperty('name');
      expect(tool).toHaveProperty('description');
      expect(tool).toHaveProperty('input_schema');
      expect(tool.input_schema).toHaveProperty('type', 'object');
      expect(tool.input_schema).toHaveProperty('properties');
      expect(tool.input_schema).toHaveProperty('required');
    }
  });
  
});