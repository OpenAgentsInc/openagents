## Sidebar

1. **sidebar-header**
   - Open/Close sidebar toggle
   - Create new chat button

2. **team-switcher**
   - Dropdown to select the current team
   - Teams are the highest level of organization in our app
   - Dropdown to select the current project
   - Each team can have one or more projects

3. **chats-section**
   - Displays the list of threads/conversations for the selected project
   - Fills the remaining container space
   - Scrollable if content exceeds the available space

4. **sidebar-footer**
   - Displays current user information
   - Clicks to expand into a menu with:
     - User settings
     - Subscription management
     - Logout option

## HTMX Usage

We are using HTMX to dynamically load and update content in the sidebar without full page reloads. This improves the user experience by making the application feel more responsive and reducing server load.

1. **Team and Project Loading**
   - The initial page load does not contain teams and projects
   - HTMX endpoint returns teams and projects for the active team
   - Switching teams updates the active team and projects
   - Switching projects updates the active project

2. **Chat List Loading**
   - Currently, demo chats are hardcoded
   - Plan to implement HTMX-based loading for chat lists

## Plan for Fetching Demo Chats

To implement HTMX-based loading for demo chats and integrate it with the team fetching process:

1. Create a new HTMX endpoint for fetching chats
   - Route: `/api/chats`
   - Controller: `ChatController@index`
   - Method: GET
   - Parameters: `team_id`, `project_id` (optional)

2. Update the `chats-section.blade.php` component:
   - Add HTMX attributes for loading chats
   - Example:
     ```html
     <div hx-get="/api/chats"
          hx-trigger="load, teamChanged from:body"
          hx-include="[name='team_id']"
          hx-target="#chat-list">
       <!-- Chat list content -->
     </div>
     ```

3. Implement the `ChatController@index` method:
   - Fetch chats based on the provided `team_id` and `project_id` (if available)
   - Return a partial view with the list of chats

4. Create a new partial view for rendering the chat list:
   - File: `resources/views/partials/chat-list.blade.php`
   - This view will contain the HTML structure for the chat list

5. Update the team switching logic:
   - When a team is switched, trigger an event to reload the chat list
   - Example:
     ```javascript
     document.body.dispatchEvent(new Event('teamChanged'));
     ```

6. Implement lazy loading for chats:
   - Load a limited number of chats initially
   - Add a "Load More" button or implement infinite scrolling using HTMX

By implementing this plan, we can dynamically load and update the chat list based on the selected team and project, improving performance and user experience.