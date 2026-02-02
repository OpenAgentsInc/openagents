import { createAuthClient } from "better-auth/react";
import {
  convexClient,
  crossDomainClient,
} from "@convex-dev/better-auth/client/plugins";
const CONVEX_SITE_URL = import.meta.env.CONVEX_SITE_URL as string | undefined;

export const authClient = createAuthClient({
  baseURL: CONVEX_SITE_URL,
  plugins: [convexClient(), crossDomainClient()],
});
