# MCP Integration Tests

This document provides information about testing the MCP integration in the chat server.

## Overview

The test suite covers the core functionality of the MCP client and tools integration with the chat server:

1. **MCP Client Manager Tests**: Test the client manager's ability to connect to MCP servers, discover tools, and execute tool calls.
2. **MCP Tools Tests**: Test the tools module's functionality for extracting tool definitions and processing tool calls.

## Running Tests

Tests are implemented using Node.js's built-in test runner for simplicity and to avoid dependency issues.

To run all tests:

```bash
cd apps/chatserver
yarn test
```

To run tests in watch mode (automatically re-run when files change):

```bash
cd apps/chatserver
yarn test:watch
```

## Test Details

### MCP Client Manager Tests

The `client.test.js` file tests the `McpClientManager` class which is responsible for managing connections to MCP servers and routing tool calls:

| Test | Description | 
|------|-------------|
| Should connect to an MCP server | Verifies that the client manager can connect to a server and retrieve tools |
| Should get all tools | Tests retrieving all registered tools from connected servers |
| Should find the server for a specific tool | Tests looking up which server provides a specific tool |
| Should call a tool and return result | Tests calling a tool and getting a result back |
| Should include token when calling a tool | Verifies authentication tokens are passed to tool calls |
| Should throw error for unknown tool | Tests error handling for non-existent tools |
| Should disconnect from all servers | Tests cleanup functionality |

### MCP Tools Tests

The `tools.test.js` file tests the MCP tools utility functions that help with tool execution:

| Test | Description |
|------|-------------|
| Should return predefined tool definitions | Tests that tool definitions are properly formatted for LLMs |
| Should process a tool call and return a result | Tests the proper handling of tool calls including generating results |
| Should handle errors when processing a tool call | Tests error handling during tool execution |
| Should return null for null input | Tests proper handling of invalid inputs |

## Testing Architecture

The tests use a simplified mocking approach instead of complex mocking libraries:

1. **Mocked Dependencies**: The tests use mock implementations of external dependencies like the MCP client.
2. **Isolated Tests**: Each module is tested in isolation to ensure proper behavior.
3. **Comprehensive Coverage**: The tests cover normal operation, edge cases, and error scenarios.

## Sample Test Output

```
TAP version 13
# Subtest: MCP Client Manager
    # Subtest: should connect to an MCP server
    ok 1 - should connect to an MCP server
      ...
    # Subtest: should get all tools
    ok 2 - should get all tools
      ...
    # Subtest: should find the server for a specific tool
    ok 3 - should find the server for a specific tool
      ...
    # Subtest: should call a tool and return result
    ok 4 - should call a tool and return result
      ...
    # Subtest: should include token when calling a tool
    ok 5 - should include token when calling a tool
      ...
    # Subtest: should throw error for unknown tool
    ok 6 - should throw error for unknown tool
      ...
    # Subtest: should disconnect from all servers
    ok 7 - should disconnect from all servers
      ...
    1..7
ok 1 - MCP Client Manager
  ...
# Subtest: MCP Tools
    # Subtest: extractToolDefinitions should return predefined tool definitions
    ok 1 - extractToolDefinitions should return predefined tool definitions
      ...
    # Subtest: processToolCall should process a tool call and return a result
    ok 2 - processToolCall should process a tool call and return a result
      ...
    # Subtest: processToolCall should handle errors when processing a tool call
    ok 3 - processToolCall should handle errors when processing a tool call
      ...
    # Subtest: processToolCall should return null for null input
    ok 4 - processToolCall should return null for null input
      ...
    1..4
ok 2 - MCP Tools
  ...
1..2
# tests 13
# suites 0
# pass 13
# fail 0
# cancelled 0
# skipped 0
# todo 0
```

## Adding New Tests

When adding new functionality to the MCP integration, follow these guidelines for testing:

1. Create new test files in the `src/mcp/tests` directory for new modules
2. Import new test files in `src/mcp/tests/index.js`
3. Use the Node.js test framework's `test()` and `assert` functions
4. Mock external dependencies to isolate the code being tested
5. Test both normal operation and error cases

For example:

```javascript
const test = require('node:test');
const assert = require('node:assert');

test('New feature', async (t) => {
  await t.test('should handle normal case', async () => {
    // Test implementation
    assert.strictEqual(result, expectedValue);
  });
  
  await t.test('should handle error case', async () => {
    // Test implementation
    try {
      // Code that should throw
      assert.fail('Expected error was not thrown');
    } catch (error) {
      assert.strictEqual(error.message, 'Expected error message');
    }
  });
});