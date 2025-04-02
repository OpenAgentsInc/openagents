# Issue 836: Settings Layout Refactoring

## Current Implementation
The current settings layout differs from the main chat layout:
- Uses horizontal tabs at the top for navigation between settings sections
- Doesn't use the same sidebar component as the main layout
- Has a different visual style than the main chat interface
- Has a simple "Back to Chat" link in the top-left instead of matching the main UI

## Requirements
From issue #836, we need to:
1. Make Settings layout match MainLayout with the same sidebar component
2. Change the horizontal tabs into vertical sidebar items
3. Replace the "Coder 0.0.1" text with a "Back to Chat" button
4. Keep all existing functionality intact

## Technical Plan

### Files to Modify:
1. `apps/coder/src/pages/settings/SettingsLayout.tsx` - Primary file that needs to be restructured
2. No changes needed to the individual settings pages as the content and functionality can remain the same

### Implementation Details:
1. Refactor `SettingsLayout.tsx` to:
   - Use `SidebarProvider`, `Sidebar` and related components from MainLayout
   - Add sidebar navigation items (Models, Local Models, Prompts, Preferences)
   - Add "Back to Chat" button in the header section
   - Keep the existing header and main content area pattern
   - Match the styling from MainLayout

### Testing Approach:
1. Verify navigation works correctly between all settings pages
2. Ensure the "Back to Chat" button works
3. Check that styling is consistent with the main chat interface
4. Ensure all functionality in settings pages works properly

## Expected Outcome
The settings pages will have a consistent look and feel with the main chat interface, making the application more cohesive and improving the user experience.