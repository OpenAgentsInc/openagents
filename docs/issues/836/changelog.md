# Changelog: Settings Layout Redesign

## Bug Fixes
- Fixed issue with missing Lucide icon export by replacing the non-existent `Prompt` icon with `MessageSquare`

## Changes
- **UI Improvement**: Refactored the settings layout to match the main chat interface layout
- **Navigation**: Changed horizontal tabs to vertical sidebar navigation for better consistency
- **Header**: Replaced "Coder 0.0.1" header with a "Back to Chat" button
- **Visual Design**: Updated the settings pages to share the same visual structure as the main application
- **UX Improvement**: Added icons to each settings navigation item for better visual recognition
- **Organization**: Separated API Keys into a dedicated page for better organization and security focus
- **Default Navigation**: Changed default settings route to point to API Keys page

## Files Changed
- `/apps/coder/src/pages/settings/SettingsLayout.tsx` - Completely refactored to use the sidebar layout
- `/apps/coder/src/pages/settings/ModelsPage.tsx` - Updated to focus only on model configuration
- `/apps/coder/src/pages/settings/ApiKeysPage.tsx` - New page for API key management
- `/apps/coder/src/routes/routes.tsx` - Added new route for API Keys

## Before & After
Before: Settings used horizontal tabs and had a different layout than the main interface.
After: Settings uses a sidebar layout identical to the main chat interface, with vertical navigation.

## Implementation Notes
- No changes were required to individual settings pages or routes
- Reused the same sidebar components from the main layout
- Added appropriate icons for each settings section
- Maintained all existing functionality

## Issue Reference
Resolves issue #836: "Settings layout should match main layout"