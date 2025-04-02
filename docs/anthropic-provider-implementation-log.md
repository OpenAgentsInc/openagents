# Anthropic Provider Implementation Log

## Implementation Date: 2024-04-02

## Overview

This document logs the implementation of Anthropic Claude models in the OpenAgents application. The integration adds direct access to Anthropic's Claude models without relying on OpenRouter as an intermediary.

## Implementation Steps Completed

### 1. Package Installation
- ✅ Added `@ai-sdk/anthropic` package for API integration

### 2. Model Definitions
- ✅ Added Claude models to the MODELS array in `packages/core/src/chat/MODELS.ts`:
  - Claude 3.7 Sonnet
  - Claude 3.5 Sonnet
  - Claude 3 Opus
  - Claude 3 Haiku
- ✅ Ensured model names have "(Anthropic)" suffix to differentiate from OpenRouter versions

### 3. Server Integration
- ✅ Updated `apps/coder/src/server/server.ts` with the following changes:
  - Added anthropic imports: `import { anthropic, createAnthropic } from '@ai-sdk/anthropic';`
  - Added ANTHROPIC_API_KEY to environment interface
  - Added extraction of Anthropic API key from request
  - Added verification and logging for Anthropic API key presence
  - Created Anthropic client with the API key
  - Added Anthropic case to provider selection logic
  - Added error handling for missing Anthropic API key

### 4. Important Implementation Notes

#### Model Naming
To avoid confusion between the same models available on both OpenRouter and Anthropic:
- Added "(Anthropic)" suffix to names of Anthropic-specific models
- Example: "Claude 3.7 Sonnet (Anthropic)" vs "Claude 3.7 Sonnet" on OpenRouter

#### API Key Handling
- Implemented the same API key extraction pattern used for OpenRouter
- Added proper fallback to environment variables if API key not provided in request
- Added error messages directing users to Settings > API Keys when key is missing

#### Model Functionality
- Ensured models are properly configured with correct context windows (200K tokens)
- Enabled tool support for all Claude models

### 5. Testing Results

Manual testing verified:
- API key extraction and validation works correctly
- Models appear correctly in the selection dropdown with the "(Anthropic)" suffix
- Error messages are shown properly when API key is missing
- Models function correctly with valid API key

## Future Improvements

1. Add streaming optimization for Anthropic models
2. Add support for image uploads to Anthropic models
3. Improve error handling with more specific error messages for different API errors
4. Add Claude-specific features like built-in reasoning via Anthropic headers

## Usability Improvements

### "Show All Models" Button
- Added a "Show All Models" button to the Settings > Models page
- This allows users to easily make all models visible in the selector with one click
- Particularly useful for ensuring newly added models like the Anthropic Claude models appear in the selector
- Implemented as a simple action that gets all model IDs and updates the visibility settings
- Uses Sonner toast notifications instead of alerts for a better user experience

### Model Detection Fixes
- Fixed an issue where Anthropic Claude models were being incorrectly routed to LMStudio
- Added proper detection of Anthropic models in the ModelSelect component by checking for `claude-` prefix
- Updated dynamic model creation to correctly set provider to 'anthropic' for Claude models
- Added dedicated localStorage key for Anthropic models for improved persistence
- Fixed Anthropic version header conflicts by removing duplicate headers
- Added validation to prevent routing Claude models to LMStudio
- Improved logging to better diagnose incorrect model routing
- Skipped unnecessary LMStudio checks for Anthropic models to prevent errors

## Conclusion

The Anthropic provider implementation is now complete and working as expected. Users can now use Claude models directly through Anthropic's API rather than through OpenRouter, which may offer better reliability and lower latency. 

To ensure the Anthropic models appear in the selector, users should:
1. Add their Anthropic API key in Settings > API Keys
2. Go to Settings > Models and click the "Show All Models" button
