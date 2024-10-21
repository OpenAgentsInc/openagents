Sidebar shows dropdowns to select team and project.

Team has many projects. Project has many threads (chats).

When a project is selected, the user's list of threads are shown.

Clicking a thread navigates to /chat/{id} and loads the chat in the main view. It's done via HTMX, swapping in HTML and updating the URL without making a full page navigation.
