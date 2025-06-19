# OpenRouter Provider Research Brief

## Context
OpenAgents currently has a vendored Effect AI library (`@openagentsinc/ai`) with providers for OpenAI, Anthropic, and Ollama. We need to investigate adding support for OpenRouter to enable access to a wide variety of AI models through a single API.

## Current Architecture Understanding

### Effect AI Provider Structure
The Effect AI library uses a service-oriented architecture with:
- **Core abstractions** in `packages/ai/src/core/`:
  - `AiLanguageModel.ts` - Main service interface
  - `AiChat.ts` - Chat abstractions
  - `AiTool.ts` - Tool system
  - `AiResponse.ts` - Response types
  - `AiError.ts` - Error handling

### Existing Providers
1. **OpenAI Provider** (`packages/ai/src/providers/openai/`):
   - `OpenAiClient.ts` - Configurable with custom API URL (line 94: `options.apiUrl ?? "https://api.openai.com/v1"`)
   - `OpenAiConfig.ts` - Client transformation support
   - `OpenAiLanguageModel.ts` - Language model implementation
   - Full streaming support with SSE

2. **Anthropic Provider** (`packages/ai/src/providers/anthropic/`)
3. **Ollama Provider** (`packages/ai/src/providers/ollama/`) - Local LLM support

## Research Questions

### 1. OpenRouter API Compatibility
**Primary Question**: Is OpenRouter's API compatible with OpenAI's API format?

**Research Tasks**:
- Check OpenRouter's official documentation for API specification
- Verify if OpenRouter supports the OpenAI chat completions endpoint format
- Confirm streaming support via Server-Sent Events (SSE)
- Check authentication method (Bearer token vs API key header)
- Verify response format compatibility

**Key URLs to investigate**:
- OpenRouter documentation: https://openrouter.ai/docs
- OpenRouter API reference
- Any migration guides from OpenAI to OpenRouter

### 2. Implementation Approach
**Primary Question**: Should we create a new provider or reuse the existing OpenAI provider?

**Option A - Reuse OpenAI Provider**:
- Pros: Minimal code changes, just configuration
- Cons: May miss OpenRouter-specific features
- Research: What OpenRouter-specific features would we miss?

**Option B - Create OpenRouter Provider**:
- Pros: Full control, OpenRouter-specific features
- Cons: More code to maintain
- Research: What unique features does OpenRouter offer?

**Research Tasks**:
- List OpenRouter-specific features not in OpenAI API
- Check if OpenRouter has special headers or parameters
- Investigate model routing capabilities
- Check for any rate limiting differences

### 3. Model Management
**Primary Question**: How does OpenRouter handle model selection and routing?

**Research Tasks**:
- How are models specified in OpenRouter requests?
- Does OpenRouter support model fallbacks?
- Are there special model identifiers or namespaces?
- How does pricing work across different models?
- Any model-specific parameters or constraints?

### 4. Authentication & Configuration
**Primary Question**: What configuration is needed for OpenRouter?

**Research Tasks**:
- API key format and location
- Required headers beyond authentication
- Any organization/project identifiers like OpenAI?
- Rate limiting headers or responses
- Error response formats

### 5. Effect Integration Requirements
**Primary Question**: What Effect patterns are needed for OpenRouter?

**Research Tasks**:
- Should we use the same Layer/Service pattern?
- Any special error types needed?
- Configuration schema requirements
- Telemetry/observability considerations

## Implementation Recommendations Needed

Based on your research, please provide:

1. **Recommended Approach**: 
   - Should we reuse OpenAI provider or create new one?
   - If reusing, what configuration changes are needed?
   - If new provider, what's the minimal implementation?

2. **Code Examples**:
   - Show how to configure OpenRouter with existing OpenAI provider (if compatible)
   - OR outline the structure for a new OpenRouter provider

3. **Configuration Schema**:
   - What configuration options should we expose?
   - Any OpenRouter-specific settings needed?

4. **Testing Strategy**:
   - How can we test the integration?
   - Any free tier or test endpoints available?

5. **Documentation Needs**:
   - What should we document for users?
   - Any migration guides needed?

## Success Criteria

A successful research outcome will:
1. Definitively answer whether OpenRouter is OpenAI-compatible
2. Provide clear implementation path with code examples
3. Identify any blockers or limitations
4. Include configuration examples for common use cases
5. Address Effect-specific integration patterns

## Additional Context

- OpenAgents uses Effect.js for functional programming patterns
- We want to maintain type safety and proper error handling
- The solution should support streaming responses
- We need to preserve the existing Layer-based configuration system
- Current AI package only exports Ollama provider publicly, but OpenAI/Anthropic are available internally

Please conduct thorough research and provide actionable recommendations for implementing OpenRouter support in the OpenAgents Effect AI library.