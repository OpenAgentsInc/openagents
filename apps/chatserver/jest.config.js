/** @type {import('jest').Config} */
module.exports = {
  testEnvironment: 'node',
  testMatch: ['**/*.test.ts'],
  moduleFileExtensions: ['ts', 'js', 'json'],
  transform: {
    '^.+\\.tsx?$': 'ts-jest'
  },
  moduleNameMapper: {
    // Mock the MCP SDK and core
    '@modelcontextprotocol/sdk/client/index.js': '<rootDir>/src/__mocks__/mcp-sdk.ts',
    '@modelcontextprotocol/sdk/client/sse.js': '<rootDir>/src/__mocks__/mcp-transport.ts',
    '@openagents/core': '<rootDir>/src/__mocks__/core.ts',
  },
};