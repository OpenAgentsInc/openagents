# Issue #828: Model Grid and Visibility Implementation

## Overview

This issue involves refactoring the model handling system in OpenAgents to improve user experience and provide more flexibility in model selection. Instead of having a single "default model", we need to implement a system where models can be in one of three states:

1. **Selected**: The currently active model (replacing the concept of "default model")
2. **Available**: Models that are visible in the model selection dropdown
3. **Hidden**: Models that aren't shown in the dropdown

## Key Changes Required

1. **Database Schema**:
   - Update the Settings schema to replace `defaultModel` with `selectedModelId`
   - Add a new field to store visible model IDs (`visibleModelIds: string[]`)

2. **Repository Methods**:
   - Update SettingsRepository to handle selecting models
   - Add methods for showing/hiding models
   - Update optimistic UI response handling

3. **UI Components**:
   - Create a new `ModelGrid` data table in the ModelsPage
   - Update the ModelSelect component to only show visible models
   - Add action buttons for selecting models and toggling visibility

4. **Transition Strategy**:
   - Ensure backward compatibility for existing users
   - Convert existing "defaultModel" settings to the new format
   - Set reasonable defaults for visible models

## Implementation Approach

1. First, update the database types and schema to support the new model
2. Modify the SettingsRepository to handle the new model attributes
3. Update the useSettings hook to expose the new model management methods
4. Implement the ModelGrid UI component
5. Update the ModelSelect component to filter by visibility
6. Add tests to verify the new functionality

This implementation will give users more control over which models are displayed, making the UX cleaner and more focused while still allowing easy access to all available models through the settings page.