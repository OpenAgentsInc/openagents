// Mock for the MCP SSE transport

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

export class SSEClientTransport {
  constructor(url: URL) {}

  start = createMockFn<() => Promise<void>>().mockResolvedValue(undefined);
  close = createMockFn<() => Promise<void>>().mockResolvedValue(undefined);
  send = createMockFn<(message: any) => Promise<void>>().mockResolvedValue(undefined);
  
  onmessage: ((message: any) => void) | null = null;
  onclose: (() => void) | null = null;
  onerror: ((error: Error) => void) | null = null;
}