/**
 * Shim for AI MCP stdio module in browser environments
 * 
 * This provides mock implementations of the MCP stdio functionality
 * to prevent runtime errors in the browser
 */

// Mock error class to replace McpStdioError
export class McpStdioError extends Error {
  constructor(name, message, cause) {
    super(message);
    this.name = name;
    this.cause = cause;
  }
}

// Exported mock function to satisfy imports
export function createMcpStdioTransport() {
  console.warn('createMcpStdioTransport is not available in browser environments');
  
  // Return a mock transport that satisfies the interface but does nothing
  return {
    connect: async () => {
      console.warn('MCP stdio transport cannot connect in browser environments');
      return {
        read: async () => null,
        write: async () => {},
        close: async () => {},
      };
    },
  };
}

export default {
  McpStdioError,
  createMcpStdioTransport
};