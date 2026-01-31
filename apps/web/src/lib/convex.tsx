import { CONVEX_URL } from "astro:env/client";
import { ConvexBetterAuthProvider } from "@convex-dev/better-auth/react";
import { ConvexReactClient } from "convex/react";
import { type FunctionComponent, type JSX } from "react";
import { authClient } from "./auth-client";

const client = new ConvexReactClient(CONVEX_URL);

// Astro context providers don't work when used in .astro files.
// See this and other related issues: https://github.com/withastro/astro/issues/2016#issuecomment-981833594
//
// ConvexBetterAuthProvider wires Convex auth to Better Auth (Option A: direct to Convex, no proxy).
export function withConvexProvider<Props extends JSX.IntrinsicAttributes>(
  Component: FunctionComponent<Props>,
) {
  return function WithConvexProvider(props: Props) {
    return (
      <ConvexBetterAuthProvider client={client} authClient={authClient}>
        <Component {...props} />
      </ConvexBetterAuthProvider>
    );
  };
}
