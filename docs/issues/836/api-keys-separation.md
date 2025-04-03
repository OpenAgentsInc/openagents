# API Keys Separation

## Summary
As part of the settings layout redesign, API Keys management has been separated from the Models page and given its own dedicated page. This change improves organization and makes it easier for users to find and manage their API keys.

## Updates to the API Keys Page
- Simplified the UI by removing tabs and displaying all providers on a single page
- Focused on the two primary providers: Anthropic and OpenRouter (removed Groq)
- Added separators between different provider sections for better visual organization

## Changes Made

1. Created a new `ApiKeysPage.tsx` component:
   - Dedicated page for managing API keys
   - Extracted API key management functionality from ModelsPage
   - Added clear organization with tabs for different providers
   - Improved security messaging and UI

2. Modified the Models page:
   - Removed the API Keys tab
   - Focused the page entirely on model selection and visibility
   - Updated reset functionality to clarify it doesn't affect API keys
   - Renamed from "API Models" to just "Models" for clarity

3. Updated routing:
   - Added a new route for `/settings/api-keys`
   - Changed the default settings route to point to API Keys instead of Models
   - Updated route tree to include the new API Keys route

4. Updated sidebar navigation:
   - Added a dedicated API Keys item with key icon
   - Updated Models item to reflect its more focused purpose
   - Positioned API Keys directly after Models in the menu for logical flow

## Benefits

1. **Improved Organization**: API keys are now clearly separated from model configuration
2. **Better Security Focus**: The API Keys page can focus on secure handling of keys
3. **Clearer Navigation**: Users can directly navigate to the API Keys page
4. **Simplified UI**: Each page has a more focused purpose with less complexity

## Implementation Notes

The API Keys page reuses much of the same code from the original implementation but now has:
- More space for API key management
- Improved security messaging
- A cleaner UI focused solely on key management
- Better organization with the Key icon in navigation

This change complements the overall settings layout redesign by further improving the organization and usability of the settings area.