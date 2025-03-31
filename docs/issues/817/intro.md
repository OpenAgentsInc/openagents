# Issue #817: Add Model Selection to OpenAgents

## Overview

This issue focuses on implementing model selection functionality in the OpenAgents application, allowing users to choose between different AI models and manage their API keys. This is a critical part of the Coder MVP, providing flexibility for users to use their preferred AI models with their own API keys.

## Current State

Analyzing the codebase, I found:

1. **Settings Infrastructure**: The codebase already has a robust settings repository (`packages/core/src/db/repositories/settings-repository.ts`) using RxDB for persistence, with existing methods for managing:
   - API keys: `setApiKey`, `getApiKey`, `deleteApiKey`
   - Default model selection: `defaultModel` field in settings
   - General preferences: `setPreference`, `getPreference`

2. **Models Data**: The application already has a comprehensive models array (`packages/core/src/chat/models.ts`) defining various AI models with metadata including:
   - Provider information (OpenRouter, Groq, Anthropic)
   - Model capabilities and context lengths
   - Pricing information
   - Tool support flags

3. **UI Components**: The Coder app uses ShadUI components and already has:
   - A placeholder for model selection in the header (`apps/coder/src/pages/HomePage.tsx` line ~134)
   - A message input component that could be enhanced to show the selected model
   - A sidebar with navigation elements that could link to the settings page

4. **Routing System**: The app uses TanStack Router for navigation (`apps/coder/src/routes/routes.tsx`) and needs new routes for settings pages.

## Required Implementation

Based on the issue and codebase analysis, we need to implement:

1. **Model Selection UI Components**:
   - Enhance the existing placeholder dropdown in HomePage to be functional
   - Create a proper model selection dropdown component using ShadUI elements
   - Show the current model in the message input area

2. **Settings Pages**:
   - Create a `/settings/models` route and page
   - Implement API key management UI for different providers
   - Allow setting default model preferences

3. **Core Functionality**:
   - Connect model selection to the `usePersistentChat` hook
   - Persist model selections per thread and globally
   - Enable fetching and using the correct API keys for different providers

4. **Navigation**:
   - Add settings navigation in the sidebar
   - Create navigation to the model settings page

All UI components must follow the project's ShadUI design system and maintain the application's existing aesthetic.

## Next Steps

The implementation will focus on:
1. Creating the settings route and basic page structure
2. Implementing the model selection dropdown
3. Building the API key management interface
4. Connecting the UI to the existing settings repository
5. Enhancing the chat component to use the selected model