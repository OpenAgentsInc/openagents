import { CONVEX_URL } from "astro:env/client";
// Auth disabled for this version — no get-session / CORS to Convex site.
// import { ConvexBetterAuthProvider } from "@convex-dev/better-auth/react";
import { ConvexReactClient, ConvexProvider } from "convex/react";
import { type FunctionComponent, type JSX } from "react";
// import { authClient } from "./auth-client";

export const convexClient = new ConvexReactClient(CONVEX_URL);

// Astro context providers don't work when used in .astro files.
// See this and other related issues: https://github.com/withastro/astro/issues/2016#issuecomment-981833594
//
// ConvexBetterAuthProvider wires Convex auth to Better Auth (Option A: direct to Convex, no proxy).
// Auth disabled: use plain ConvexProvider so no fetch to convex.site/api/auth/get-session.
export function withConvexProvider<Props extends JSX.IntrinsicAttributes>(
  Component: FunctionComponent<Props>,
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
