// Mock for the MCP SDK client
export interface Tool {
  name: string;
  description?: string;
  inputSchema?: any;
}

// Simple type-safe mock function
type MockFunction<T extends (...args: any[]) => any> = {
  (...args: Parameters<T>): ReturnType<T>;
  implementation: T | undefined;
  mockImplementation: (impl: T) => MockFunction<T>;
  mockResolvedValue: <V>(value: V) => MockFunction<T>;
};

function createMockFn<T extends (...args: any[]) => any>(): MockFunction<T> {
  const mockFn = function(this: any, ...args: any[]) {
    return mockFn.implementation?.apply(this, args);
  } as MockFunction<T>;
  
  mockFn.implementation = undefined;
  
  mockFn.mockImplementation = function(impl: T) {
    mockFn.implementation = impl;
    return mockFn;
  };
  
  mockFn.mockResolvedValue = function<V>(value: V) {
    mockFn.implementation = (() => Promise.resolve(value)) as T;
    return mockFn;
  };
  
  return mockFn;
}

export class Client {
  constructor(clientInfo: any, capabilities: any) {}

  connect = createMockFn<() => Promise<void>>().mockResolvedValue(undefined);
  close = createMockFn<() => Promise<void>>().mockResolvedValue(undefined);
  
  listTools = createMockFn<() => Promise<Tool[]>>().mockResolvedValue([
    { name: 'tool1', description: 'Tool 1 description' },
    { name: 'tool2', description: 'Tool 2 description' }
  ]);
  
  callTool = createMockFn<(params: { name: string; arguments: any }) => Promise<any>>().mockImplementation(
    ({ name, arguments: args }) => {
      return Promise.resolve({
        content: [
          { 
            type: 'text', 
            text: JSON.stringify({ result: `Called ${name} with ${JSON.stringify(args)}` }) 
          }
        ]
      });
    }
  );
}