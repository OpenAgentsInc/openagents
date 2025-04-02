# Improve API Key Management in Coder App

## Problem Description

The current implementation of API key management in the Coder app has a disconnection between the UI for setting API keys and the actual use of these keys in model API requests.

### Current Implementation
- API keys are stored in the settings repository (packages/core/src/db/repositories/settings-repository.ts)
- The app uses a hardcoded OPENROUTER_API_KEY from env settings in package.json
- The ModelsPage.tsx component has UI for managing API keys per provider
- The useSettings hook provides methods for getting and setting API keys

### Issues Found
1. **Hardcoded Environment Variables**: The app uses a hardcoded OpenRouter API key in `package.json` instead of user-provided keys
2. **Missing Integration**: API keys stored through the UI in ModelsPage.tsx don't seem to be used for actual API calls
3. **No Provider-Specific Selection**: The system doesn't automatically select the appropriate API key based on the chosen model's provider
4. **No Validation**: There's no validation to check if an API key is valid before allowing a user to select a model
5. **Unclear Error Handling**: When API calls fail due to missing or invalid keys, the error messaging isn't user-friendly

## Proposed Solution

1. **Remove Hardcoded Key**: Remove the hardcoded OPENROUTER_API_KEY from package.json env configuration
2. **Modify API Client Initialization**: 
   - Update `apps/coder/src/server/mcp-clients.ts` to fetch API keys from the settings repository
   - Add a mechanism to select the correct API key based on the model provider

3. **Update Transport Layer**:
   - Modify `packages/core/src/mcp/transport.ts` to accept API keys via parameters
   - Add proper error handling for missing/invalid API keys

4. **UI Improvements**:
   - Add validation feedback in the ModelsPage UI when saving API keys
   - Show clear warnings when attempting to use models without setting the required API key
   - Provide better guidance on where to obtain API keys for different providers

5. **Fallback Strategy**:
   - Implement graceful degradation when API keys are missing
   - Allow users to set keys inline when attempting to use a model without a key

## Implementation Steps

1. Create a middleware function that:
   - Intercepts model selection
   - Checks for the required API key based on the provider
   - Prompts the user to enter a key if missing

2. Modify the transport layer to:
   - Access the settings repository to get provider-specific API keys
   - Include proper headers and authentication based on provider requirements
   - Return clear errors when API key issues occur

3. Update the UI to:
   - Indicate which models require API keys
   - Show the status of API keys (valid, invalid, missing)
   - Guide users through setting up keys for their preferred models

## Benefits

- **Improved Security**: No hardcoded API keys in the repository
- **Better User Experience**: Clear feedback about API key requirements
- **More Flexibility**: Users can manage multiple provider keys through the UI
- **Reduced Errors**: Fewer failed API calls due to missing/invalid keys

## Related Files

- `/packages/core/src/db/repositories/settings-repository.ts` - Storage for API keys
- `/packages/core/src/db/schema.ts` - Schema for settings including API keys
- `/apps/coder/src/pages/settings/ModelsPage.tsx` - UI for managing API keys
- `/apps/coder/src/server/mcp-clients.ts` - Client initialization where keys should be used

## Questions

- Should we add a testing facility to validate API keys before saving them?
- Do we want to implement a secure storage solution for API keys beyond the current database storage?
- Should we consider adding support for environment-based API keys for development purposes?