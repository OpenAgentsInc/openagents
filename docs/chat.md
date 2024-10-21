Sidebar shows dropdowns to select team and project.

Team has many projects. Project has many threads (chats).

When a project is selected, the user's list of threads are shown.

Clicking a thread navigates to /chat/{id} and loads the chat in the main view. It's done via HTMX, swapping in HTML and updating the URL without making a full page navigation.

Pest Test Assertions:

1. Sidebar Structure:
   - Assert that the sidebar contains a dropdown for team selection
   - Assert that the sidebar contains a dropdown for project selection

2. Relationship Structure:
   - Assert that a team can have multiple projects
   - Assert that a project can have multiple threads

3. User Interface Flow:
   - Assert that selecting a project displays the user's list of threads for that project
   - Assert that the displayed threads belong to the selected project

4. Navigation:
   - Assert that clicking a thread navigates to the correct /chat/{id} URL
   - Assert that the chat content is loaded in the main view
   - Assert that navigation occurs without a full page reload (HTMX functionality)

5. HTMX Functionality:
   - Assert that the URL is updated when navigating to a chat thread
   - Assert that only the main content area is updated, not the entire page

6. Data Integrity:
   - Assert that the loaded chat content matches the selected thread
   - Assert that chat messages are displayed in the correct order

7. Error Handling:
   - Assert that attempting to access a non-existent chat ID returns an appropriate error
   - Assert that the UI handles cases where a user has no threads in a project

These test assertions cover the main functionality described in the chat documentation and will help ensure the reliability and correctness of the chat feature implementation.