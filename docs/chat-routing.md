# Chat Routing Documentation

This document explains how routing works in the OpenAgents chat application, which consists of a React frontend served by a Rust backend.

## Architecture Overview

The chat application uses a hybrid routing approach:

1. The Rust backend serves the React application at the `/chat` path
2. React Router handles client-side routing within the application
3. All routes within the chat application are prefixed with `/chat`

## Backend Setup (Rust)

In `src/server/config.rs`, the Rust server is configured to serve the Vite-built React application:

```rust
// Serve index.html for all /chat/* routes
.nest_service(
    "/chat",
    tower_http::services::fs::ServeFile::new("./chat/dist/index.html"),
)
// Serve static assets
.nest_service("/chat/assets", ServeDir::new("./chat/dist/assets").precompressed_gzip())
```

This configuration:

- Serves `index.html` for ALL routes under `/chat/*` path
- Separately serves static assets (JS, CSS, etc.) from `/chat/assets/*`
- Enables gzip compression for static assets
- This setup ensures that the SPA routing works correctly by always serving the React app

### How the Routing Works

1. When a request comes in for any `/chat/*` path:
   - The server always serves `index.html`
   - This allows React Router to handle all routing client-side
2. When a request comes in for `/chat/assets/*`:
   - The server serves the actual static files (JS, CSS, images)
   - These requests are made by the React app after it loads

## Frontend Setup (React)

### Router Configuration

In `chat/src/App.tsx`, we configure React Router with a base path that matches the backend:

```tsx
function App() {
  return (
    <BrowserRouter basename="/chat">
      <Routes>
        <Route path="/" element={<LoginScreen />} />
        <Route path="/new" element={<ChatScreen />} />
        <Route path="*" element={<Navigate to="/" replace />} />
      </Routes>
    </BrowserRouter>
  );
}
```

Key points:

- `basename="/chat"` ensures all routes are prefixed with `/chat`
- The root path (`/`) shows the login screen
- `/new` displays the chat interface
- Any unknown routes redirect to the login screen

### Available Routes

| URL Path    | Component   | Description             |
| ----------- | ----------- | ----------------------- |
| `/chat`     | LoginScreen | Initial login page      |
| `/chat/new` | ChatScreen  | Main chat interface     |
| `/chat/*`   | Navigate    | Redirects to login page |

### Components

1. **LoginScreen** (`chat/src/App.tsx`):

   - Displays the application title
   - Provides GitHub authentication button
   - Centered card layout with dark theme

2. **ChatScreen** (`chat/src/pages/ChatScreen.tsx`):
   - Full-screen chat interface
   - Message display area
   - Fixed input area at bottom
   - Maintains consistent dark theme

## How It Works

1. When a user visits any `/chat/*` URL:

   - The Rust backend serves the React application
   - React Router takes over client-side routing
   - The appropriate component is rendered based on the path

2. The `basename` configuration ensures:
   - All React Router paths are relative to `/chat`
   - Links and navigations automatically include the `/chat` prefix
   - Browser history works correctly within the `/chat` context

## Best Practices

1. Always use React Router's components for navigation:

   ```tsx
   import { Link } from "react-router-dom";
   <Link to="/new">Go to Chat</Link>;
   ```

2. For programmatic navigation:

   ```tsx
   import { useNavigate } from "react-router-dom";
   const navigate = useNavigate();
   navigate("/new");
   ```

3. Remember that all routes in components are relative to `/chat`, so you don't need to include it in your paths.

## Security Considerations

- The chat interface (`/chat/new`) should be protected by authentication
- Authentication state should be managed globally (e.g., using React Context)
- Consider implementing route guards for protected routes
