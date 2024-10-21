# OpenAgents.com flow

- User visits homepage openagents.com
  - Handled by app/Http/Controllers/DashboardController.php
  - If unauthenticated, user sees `homepage` view
  - If authenticated, user sees `components.dashboard.dashboard` view
