import React from "react";
import { createRoute } from "@tanstack/react-router";
import { RootRoute } from "./__root";
import ChatPage from "../pages/ChatPage";
import SettingsLayout from "../pages/settings/SettingsLayout";
import ModelsPage from "../pages/settings/ModelsPage";
import ApiKeysPage from "../pages/settings/ApiKeysPage";
import LocalModelsPage from "../pages/settings/LocalModelsPage";
import MCPClientsPage from "../pages/settings/MCPClientsPage";
import PromptsPage from "../pages/settings/PromptsPage";
import PreferencesPage from "../pages/settings/PreferencesPage";
import DebugPage from "../pages/settings/DebugPage";
import ToolsPage from "../pages/settings/ToolsPage";
import ChangelogPage from "../pages/ChangelogPage";
import { MainLayout } from "@/components/layout/MainLayout";
import { ChatStateProvider } from "@/providers/ChatStateProvider";
import { ModelProvider } from "@/providers/ModelProvider";
import { ApiKeyProvider } from "@/providers/ApiKeyProvider";
import { DEFAULT_SYSTEM_PROMPT } from "@openagents/core";
import { Outlet } from "@tanstack/react-router";
import { react19 } from "@openagents/core";

// Make React Router components compatible with React 19
const OutletCompat = react19.router(Outlet);

// Wrapper component that provides necessary context
function MainLayoutWrapper() {
  return (
    <ModelProvider>
      <ApiKeyProvider>
        <ChatStateProvider systemPrompt={DEFAULT_SYSTEM_PROMPT}>
          <MainLayout>
            <OutletCompat />
          </MainLayout>
        </ChatStateProvider>
      </ApiKeyProvider>
    </ModelProvider>
  );
}

// TODO: Steps to add a new route:
// 1. Create a new page component in the '../pages/' directory (e.g., NewPage.tsx)
// 2. Import the new page component at the top of this file
// 3. Define a new route for the page using createRoute()
// 4. Add the new route to the routeTree in RootRoute.addChildren([...])
// 5. Add a new Link in the navigation section of RootRoute if needed

// Main layout parent route
export const MainLayoutRoute = createRoute({
  getParentRoute: () => RootRoute,
  id: "main",
  component: MainLayoutWrapper,
});

// Home/Chat route under MainLayout
export const HomeRoute = createRoute({
  getParentRoute: () => MainLayoutRoute,
  path: "/",
  component: () => {
    // Import HomePage to ensure database initialization happens
    const HomePage = React.lazy(() => import('../pages/HomePage'));
    return (
      <React.Suspense fallback={<div></div>}>
        <HomePage />
      </React.Suspense>
    );
  },
});

// Changelog route under MainLayout
export const ChangelogRoute = createRoute({
  getParentRoute: () => MainLayoutRoute,
  path: "/changelog",
  component: ChangelogPage,
});

// Settings parent route
export const SettingsRoute = createRoute({
  getParentRoute: () => RootRoute,
  path: "/settings",
  component: SettingsLayout,
});

// Settings default route (redirects to api-keys)
export const SettingsIndexRoute = createRoute({
  getParentRoute: () => SettingsRoute,
  path: "/",
  component: () => {
    window.location.href = '/settings/api-keys';
    return null;
  }
});

// Settings child routes
export const ModelsSettingsRoute = createRoute({
  getParentRoute: () => SettingsRoute,
  path: "/models",
  component: ModelsPage,
});

export const LocalModelsSettingsRoute = createRoute({
  getParentRoute: () => SettingsRoute,
  path: "/local-models",
  component: LocalModelsPage,
});

export const MCPClientsSettingsRoute = createRoute({
  getParentRoute: () => SettingsRoute,
  path: "/mcp-clients",
  component: MCPClientsPage,
});

export const PromptsSettingsRoute = createRoute({
  getParentRoute: () => SettingsRoute,
  path: "/prompts",
  component: PromptsPage,
});

export const PreferencesSettingsRoute = createRoute({
  getParentRoute: () => SettingsRoute,
  path: "/preferences",
  component: PreferencesPage,
});

export const ApiKeysSettingsRoute = createRoute({
  getParentRoute: () => SettingsRoute,
  path: "/api-keys",
  component: ApiKeysPage,
});

export const DebugSettingsRoute = createRoute({
  getParentRoute: () => SettingsRoute,
  path: "/debug",
  component: DebugPage,
});

export const ToolsSettingsRoute = createRoute({
  getParentRoute: () => SettingsRoute,
  path: "/tools",
  component: ToolsPage,
});

// Add all routes to the route tree
export const rootTree = RootRoute.addChildren([
  MainLayoutRoute.addChildren([
    HomeRoute,
    ChangelogRoute,
  ]),
  SettingsRoute.addChildren([
    SettingsIndexRoute,
    ModelsSettingsRoute,
    ApiKeysSettingsRoute,
    ToolsSettingsRoute,
    LocalModelsSettingsRoute,
    MCPClientsSettingsRoute,
    PromptsSettingsRoute,
    PreferencesSettingsRoute,
    DebugSettingsRoute
  ])
]);
