# Testing Plan for Settings Layout Redesign (Issue #836)

## Manual Testing Steps

### Navigation Testing
1. **Home to Settings**:
   - From the main chat interface, click the "Settings" button in the sidebar
   - Verify that the settings page opens with the new sidebar layout
   - Confirm that "API Models" is selected by default

2. **Settings Navigation**:
   - Click each sidebar item in the settings layout:
     - API Models
     - Local Models
     - Prompts
     - Preferences
   - Verify that each click loads the appropriate content
   - Confirm that the active item is highlighted in the sidebar
   - Check that the page title in the header changes to match the selected section

3. **Back to Chat Navigation**:
   - Click the "Back to Chat" button in the top left
   - Verify that you are returned to the main chat interface
   - Try navigating back to settings and then use the "Home" button in the sidebar footer
   - Verify that this also returns you to the main chat interface

### Visual Consistency Testing
1. **Layout Comparison**:
   - Compare the settings layout with the main chat interface
   - Verify that the sidebar width, header height, and overall structure match
   - Check that the spacing and padding are consistent

2. **Theme Testing**:
   - Test in both light and dark themes using the toggle in the sidebar footer
   - Verify that all elements render properly in both themes
   - Check that the active indicators work correctly in both themes

### Functionality Testing
1. **API Models Page**:
   - Verify that all functionality in the API Models page works correctly
   - Test adding/removing API keys
   - Test selecting models
   - Test toggling model visibility

2. **Local Models Page**:
   - Verify that all functionality in the Local Models page works correctly
   - Test connecting to LMStudio
   - Check that available models display properly

3. **Prompts Page**:
   - Verify that system prompt editing works
   - Test saving changes to the system prompt
   - Test resetting to the default system prompt

4. **Preferences Page**:
   - Verify that preference toggles work correctly
   - Test saving preferences

### Responsive Testing
1. **Window Resizing**:
   - Resize the browser window to different dimensions
   - Verify that the layout adjusts appropriately
   - Check that content remains accessible at smaller sizes

2. **Mobile View**:
   - Test the layout in mobile view (or with a narrow browser window)
   - Verify that the sidebar collapses and can be opened with the toggle
   - Check that all content is accessible in mobile view

## Edge Cases to Test
1. **Long Content**:
   - Test with very long content in the main area
   - Verify that scrolling works correctly
   - Check that the header stays fixed while content scrolls

2. **Navigation from URLs**:
   - Test navigating directly to each settings URL:
     - `/settings/models`
     - `/settings/local-models`
     - `/settings/prompts`
     - `/settings/preferences`
   - Verify that the correct page loads with the correct sidebar item selected

3. **Browser Refresh**:
   - Navigate to a settings page and refresh the browser
   - Verify that the correct page reloads with the correct sidebar item selected

## Regression Testing
1. **Main Chat Interface**:
   - Verify that the main chat interface still functions correctly after these changes
   - Test creating, selecting, and deleting threads
   - Test sending messages and receiving responses

2. **Settings Functionality**:
   - Verify that all settings are properly saved and persisted
   - Test that changes made in the settings pages affect the application as expected

## Browser Compatibility
Test the new layout in:
- Chrome
- Firefox
- Safari (if available)
- Edge (if available)