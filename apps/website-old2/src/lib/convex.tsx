// Auth disabled for this version — no get-session / CORS to Convex site.
// import { ConvexBetterAuthProvider } from "@convex-dev/better-auth/react";
import { ConvexReactClient, ConvexProvider } from "convex/react";
import type { FunctionComponent } from "react";
// import { authClient } from "./auth-client";

const CONVEX_URL = import.meta.env.CONVEX_URL as string;

export const convexClient = new ConvexReactClient(CONVEX_URL);

// Astro context providers don't work when used in .astro files.
// See this and other related issues: https://github.com/withastro/astro/issues/2016#issuecomment-981833594
//
// ConvexBetterAuthProvider wires Convex auth to Better Auth (Option A: direct to Convex, no proxy).
// Auth disabled: use plain ConvexProvider so no fetch to convex.site/api/auth/get-session.
export function withConvexProvider<Props extends object>(
  Component: FunctionComponent<Props>
) {
  return function WithConvexProvider(props: Props) {
    return (
      <ConvexProvider client={convexClient}>
        <Component {...props} />
      </ConvexProvider>
    );
    // return (
    //   <ConvexBetterAuthProvider client={convexClient} authClient={authClient}>
    //     <Component {...props} />
    //   </ConvexBetterAuthProvider>
    // );
  };
}
