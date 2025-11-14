import { ConvexReactClient } from "convex/react";

// Get Convex URL from environment variable
// In production, this should be set via Tauri's environment or config
const CONVEX_URL = import.meta.env.VITE_CONVEX_URL || "https://your-deployment.convex.cloud";

// Create singleton Convex client
export const convexClient = new ConvexReactClient(CONVEX_URL);
