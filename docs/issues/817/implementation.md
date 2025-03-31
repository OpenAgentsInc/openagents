# Implementation Guide for Model Selection in OpenAgents

## Components Created

1. **ModelSelect Component**
   - Created a dropdown component for model selection
   - Located at: `/apps/coder/src/components/ui/model-select.tsx`
   - Uses ShadUI Command component for a searchable dropdown
   - Displays model name, description, and plan type (free/pro)
   - Allows filtering models based on plan

2. **Models Settings Page**
   - Created a dedicated settings page for models and API keys
   - Located at: `/apps/coder/src/pages/settings/ModelsPage.tsx`
   - Features:
     - Default model selection
     - API key management for different providers
     - View of available models grouped by provider
     - Security features for API key display/hiding

3. **UI Components Index**
   - Created an index file for easier import of all UI components
   - Located at: `/apps/coder/src/components/ui/index.ts`
   - Exports all ShadUI components in a centralized manner

## Changes to Existing Files

1. **Routes Configuration** (`/apps/coder/src/routes/routes.tsx`)
   - Added a new route for the models settings page
   - Path: `/settings/models`

2. **HomePage Component** (`/apps/coder/src/pages/HomePage.tsx`)
   - Added model selection dropdown in the header
   - Connected model selection to the chat configuration
   - Added navigation to settings in the sidebar
   - Implemented useSettings hook to load default model

## Implementation Details

### Settings Repository Integration

The implementation leverages the existing settings repository in the core package, which already had:
- API key management (`setApiKey`, `getApiKey`, `deleteApiKey`)
- Default model storage (`defaultModel` field)
- Preference management (`setPreference`, `getPreference`)

### Model Selection Flow

1. The default model is loaded from settings when the app starts
2. Users can select a different model from the dropdown in the header
3. The selected model is passed to the `usePersistentChat` hook
4. The model selection is persisted across sessions

### API Key Management

1. API keys are organized by provider (OpenRouter, Anthropic, Groq)
2. Keys are stored securely in the browser's local database
3. The UI provides a way to add, view, and delete API keys
4. API keys are automatically used when selecting models from the respective provider

## Design Decisions

1. **Grouped by Provider**: Models are organized by provider for better management
2. **Pro/Free Labeling**: Models are clearly labeled as Pro or Free to indicate usage requirements
3. **Description Display**: Each model shows a short description to help users understand its capabilities
4. **Context Length Display**: Context length is shown to help users understand limitations
5. **Tools Support Indicator**: Models indicate whether they support tools functionality

## Security Considerations

1. API keys are stored securely in the browser's local database
2. Keys are displayed as password fields by default with a toggle to view
3. No API keys are transmitted unless necessary for model usage

## Future Enhancements

1. Per-thread model selection and persistence
2. Usage tracking and quota management
3. Custom system prompts per model
4. Model capability comparison view