import { createRoute } from "@tanstack/react-router";
import { RootRoute } from "./__root";
import HomePage from "../pages/HomePage";
import ModelsPage from "../pages/settings/ModelsPage";

// TODO: Steps to add a new route:
// 1. Create a new page component in the '../pages/' directory (e.g., NewPage.tsx)
// 2. Import the new page component at the top of this file
// 3. Define a new route for the page using createRoute()
// 4. Add the new route to the routeTree in RootRoute.addChildren([...])
// 5. Add a new Link in the navigation section of RootRoute if needed

export const HomeRoute = createRoute({
  getParentRoute: () => RootRoute,
  path: "/",
  component: HomePage,
});

export const ModelsSettingsRoute = createRoute({
  getParentRoute: () => RootRoute,
  path: "/settings/models",
  component: ModelsPage,
});

export const rootTree = RootRoute.addChildren([HomeRoute, ModelsSettingsRoute]);