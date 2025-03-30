import React from "react";
import BaseLayout from "@/layouts/BaseLayout";
import { Outlet as RouterOutlet, createRootRoute } from "@tanstack/react-router";
import { react19 } from "@openagents/core";

// Make React Router components compatible with React 19
const Outlet = react19.router(RouterOutlet);

export const RootRoute = createRootRoute({
  component: Root,
});

function Root() {
  return (
    <BaseLayout>
      <Outlet />
    </BaseLayout>
  );
}
