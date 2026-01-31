import { createAuthClient } from "better-auth/react";
import { convexClient } from "@convex-dev/better-auth/client/plugins";
import { PUBLIC_CONVEX_SITE_URL } from "astro:env/client";

export const authClient = createAuthClient({
  baseURL: PUBLIC_CONVEX_SITE_URL,
  plugins: [convexClient()],
});
