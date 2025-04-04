# Server Refactoring and Error Handling Improvements (Issue #852)

## Overview

This PR implements a comprehensive refactoring of the server component, breaking down the monolithic `server.ts` file into smaller, more maintainable modules, and introducing a robust typed error handling system. The changes improve code quality, maintainability, testability, and provide a more consistent error handling experience for users.

## Key Changes

### 1. Server Modularization

- **Broke down `server.ts` (1100+ lines)** into multiple focused modules:
  - `routes/`: API endpoint handlers for chat and MCP
  - `providers/`: Provider-specific implementations
  - `streaming/`: Stream management and formatting
  - `tools/`: Tool execution utilities
  - `utils/`: Validation and helper functions
  - `errors/`: Error handling and recovery

### 2. Core Error System

- **Created a comprehensive error class hierarchy** in `packages/core/src/chat/errors/`:
  - Base `ChatError` class with common functionality
  - Provider-specific errors (Anthropic, Google, OpenRouter, Ollama)
  - Validation errors for input and format validation
  - Tool execution errors with detailed context
  - Network and connectivity errors
  - Rate limit and context length errors

### 3. Error Transformation and Handling

- **Added provider-specific error transformers** to convert raw API errors into typed errors
- **Implemented standardized error formatting** for SSE streams and JSON responses
- **Created recovery mechanisms** for certain error types (e.g., tool execution failures)
- **Improved error messaging** with user-friendly explanations and technical details

### 4. Provider Implementation

- **Extracted provider logic** into dedicated modules:
  - `anthropic.ts`: Claude models implementation
  - `google.ts`: Gemini models implementation
  - `openrouter.ts`: OpenRouter integration
  - `ollama.ts`: Local model handling

### 5. Documentation Updates

- **Updated `docs/error-handling.md`** with the new error system architecture
- **Created implementation log** documenting the refactoring process
- **Added JSDoc comments** throughout the codebase

## Benefits

1. **Improved Maintainability**:
   - Smaller, focused modules are easier to understand and modify
   - Clear separation of concerns makes responsibilities explicit
   - Reduced duplication improves consistency

2. **Better Error Handling**:
   - Typed errors provide compile-time safety
   - Consistent error propagation improves debugging
   - Better user feedback through specific error messages

3. **Enhanced Testability**:
   - Isolated components can be tested in isolation
   - Mocking dependencies becomes simpler
   - Error simulation can be done at various levels

4. **Future Extensibility**:
   - Adding new providers is straightforward
   - Error handling can evolve independently of business logic
   - New features can be added with minimal changes to existing code

## Testing

The changes have been tested with various scenarios:

- Chat conversations with different models (Claude, Gemini, etc.)
- Error handling with invalid API keys
- Context length errors with large conversations
- Tool execution failures
- Network connectivity issues
- Provider-specific error formats

## Related Issues

- Closes #852