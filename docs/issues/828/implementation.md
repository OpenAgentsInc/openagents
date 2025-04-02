# Issue #828: Model Grid Implementation

## Changes Made

### 1. Database Schema Updates
- Added `selectedModelId` field to the Settings type to replace `defaultModel` (while maintaining backward compatibility)
- Added `visibleModelIds` field to store an array of model IDs that should be visible in the dropdown
- Updated the RxDB schema to include these new fields

### 2. Repository Methods
- Added methods to the SettingsRepository:
  - `selectModel`: To set the currently active model
  - `showModel`: To make a model visible in the selector
  - `hideModel`: To hide a model from the selector
  - `toggleModelVisibility`: To toggle a model's visibility
  - `getVisibleModelIds`: To retrieve the list of visible model IDs
- Updated the existing methods to handle the new schema:
  - Modified `getSettings` to apply pending visibility updates
  - Updated `updateSettings` to handle both model selection and visibility changes
  - Enhanced `resetSettings` to initialize the new fields with sensible defaults

### 3. Settings Hook Updates
- Expanded the `useSettings` hook to expose the new model management methods:
  - Added `selectModel`, `showModel`, `hideModel`, `toggleModelVisibility`, and `getVisibleModelIds`
  - These functions maintain the same pattern as existing methods, with error handling and state updates

### 4. UI Components
- ModelSelect Component:
  - Modified to filter models based on visibility settings
  - Only shows models that are in the `visibleModelIds` array
  - Falls back to showing all models if no visibility settings are found

- ModelsPage Component:
  - Added a tabbed interface with "Models" and "API Keys" tabs
  - Implemented a Model Grid with:
    - Model selection column (replacing the "default model" dropdown)
    - Visibility toggle column
    - Sortable columns for author, provider, name, and ID
    - Description column
    - Search/filter functionality
  - Updated event handlers to use the new model management methods

### 5. Backward Compatibility
- Maintained the `defaultModel` field alongside `selectedModelId`
- Ensured that operations on one field are mirrored to the other
- Added fallback code to handle existing settings that don't have the new fields

## Key Functions

1. **Model Selection**:
   ```typescript
   const handleModelSelection = async (modelId: string) => {
     // Updates both the UI state and persisted settings
     setSelectedModelId(modelId);
     const result = await selectModel(modelId);
     // If the selected model is not visible, make it visible
     if (visibleModelIds.indexOf(modelId) === -1) {
       handleToggleModelVisibility(modelId);
     }
   };
   ```

2. **Visibility Toggle**:
   ```typescript
   const handleToggleModelVisibility = async (modelId: string) => {
     // Prevent hiding the selected model or the last visible model
     if (visibleModelIds.includes(modelId)) {
       if (modelId === selectedModelId || visibleModelIds.length <= 1) {
         return;
       }
     }
     // Update local state immediately for better UX
     setVisibleModelIds(prev => 
       prev.includes(modelId) 
         ? prev.filter(id => id !== modelId) 
         : [...prev, modelId]
     );
     // Persist the change
     await toggleModelVisibility(modelId);
   };
   ```

3. **Filtering Models**:
   ```typescript
   // In ModelSelect component
   useEffect(() => {
     if (settings && settings.visibleModelIds && settings.visibleModelIds.length > 0) {
       const filteredModels = MODELS.filter(model => 
         settings.visibleModelIds!.includes(model.id)
       );
       setVisibleModels(filteredModels);
     } else {
       setVisibleModels(MODELS);
     }
   }, [settings]);
   ```

## Technical Notes

- We keep both `defaultModel` and `selectedModelId` in sync to ensure backward compatibility
- The ModelGrid UI includes safeguards to prevent:
  - Hiding the currently selected model
  - Hiding all models (at least one must be visible)
- The implementation favors optimistic UI updates with error handling to roll back changes if persistence fails
- On initial setup, we populate the visible models with the top 5 most recent models to provide a good default experience