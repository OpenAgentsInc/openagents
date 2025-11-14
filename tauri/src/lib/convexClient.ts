import { ConvexReactClient } from "convex/react";

// Get Convex URL from environment variable
// Must be set in .env.local as VITE_CONVEX_URL
const CONVEX_URL = import.meta.env.VITE_CONVEX_URL;

if (!CONVEX_URL) {
  throw new Error(
    "VITE_CONVEX_URL is not set. Add it to .env.local:\n" +
    "VITE_CONVEX_URL=https://your-deployment.convex.cloud"
  );
}

// Create singleton Convex client
export const convexClient = new ConvexReactClient(CONVEX_URL);
