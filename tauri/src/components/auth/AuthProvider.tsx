import { ConvexAuthProvider, useAuthActions, useAuth } from "@convex-dev/auth/react";
import { Authenticated, Unauthenticated, useConvexAuth } from "convex/react";
import { convexClient } from "@/lib/convexClient";
import { SignIn } from "./SignIn";
import { useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";

/**
 * AuthTokenSync - Syncs Convex auth token to Rust backend
 *
 * When user is authenticated, gets the token from Convex auth
 * and passes it to Rust via set_convex_auth Tauri command.
 */
function AuthTokenSync({ children }: { children: React.ReactNode }) {
  const { isAuthenticated, isLoading, fetchAccessToken } = useAuth();
  const { signOut } = useAuthActions();

  useEffect(() => {
    const syncToken = async () => {
      if (isAuthenticated) {
        try {
          // Get the auth token using Convex auth's fetchAccessToken
          // This is the proper API for @convex-dev/auth
          const token = await fetchAccessToken({ forceRefreshToken: false });

          if (token) {
            try {
              await invoke("set_convex_auth", { token });
              console.log("Auth token synced to Rust backend");
            } catch (error) {
              console.error("Failed to sync auth token to Rust:", error);
            }
          } else {
            console.error("User authenticated but fetchAccessToken returned null - signing out");
            // Auth state is broken, force sign out
            void signOut();
          }
        } catch (error) {
          console.error("Failed to fetch access token:", error);
          // If we can't get the token, sign out to force re-authentication
          void signOut();
        }
      } else if (!isLoading) {
        // User logged out, clear token
        try {
          await invoke("set_convex_auth", { token: null });
          console.log("Auth token cleared from Rust backend");
        } catch (error) {
          console.error("Failed to clear auth token from Rust:", error);
        }
      }
    };

    syncToken();
  }, [isAuthenticated, isLoading, fetchAccessToken, signOut]);

  return <>{children}</>;
}

/**
 * AuthDebugger - Logs auth state for debugging
 */
function AuthDebugger({ children }: { children: React.ReactNode }) {
  const { isAuthenticated, isLoading, fetchAccessToken } = useAuth();

  useEffect(() => {
    console.log("[AUTH DEBUG] Auth state:", { isAuthenticated, isLoading });
    if (isAuthenticated) {
      // Try to fetch the token to verify auth is working
      fetchAccessToken({ forceRefreshToken: false })
        .then((token) => {
          console.log("[AUTH DEBUG] Has token:", !!token);
          if (token) {
            console.log("[AUTH DEBUG] Token length:", token.length);
          }
        })
        .catch((error) => {
          console.error("[AUTH DEBUG] Failed to fetch token:", error);
        });
    }
  }, [isAuthenticated, isLoading, fetchAccessToken]);

  return <>{children}</>;
}

/**
 * AuthProvider - Wraps app with Convex Auth
 *
 * - Provides ConvexAuthProvider for authentication
 * - Shows SignIn page when unauthenticated
 * - Syncs auth token to Rust backend when authenticated
 * - Renders children when authenticated
 */
export function AuthProvider({ children }: { children: React.ReactNode }) {
  return (
    <ConvexAuthProvider client={convexClient}>
      <AuthDebugger>
        <Unauthenticated>
          <SignIn />
        </Unauthenticated>
        <Authenticated>
          <AuthTokenSync>{children}</AuthTokenSync>
        </Authenticated>
      </AuthDebugger>
    </ConvexAuthProvider>
  );
}
