# OpenAgents.com flow

- User visits homepage openagents.com
  - Handled by app/Http/Controllers/DashboardController.php
  - If unauthenticated, user sees `homepage` view
  - If authenticated, user sees `components.dashboard.dashboard` view
- Default layout
  - resources/views/components/layouts/app.blade.php
  - Shows sidebar to authed users only
  - Sidebar
    - resources/views/components/sidebar/simple-sidebar.blade.php
    - Includes sidebar-header with buttons to toggle sidebar and add chat
      - resources/views/components/sidebar/sidebar-header.blade.php
    - And chats-section which shows a list of user threads organized by team/project
    - And sidebar-footer which shows user info and a dropdown with logout, settings etc.
