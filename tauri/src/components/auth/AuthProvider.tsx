import { ConvexAuthProvider } from "@convex-dev/auth/react";
import { Authenticated, Unauthenticated, useConvexAuth } from "convex/react";
import { convexClient } from "@/lib/convexClient";
import { SignIn } from "./SignIn";
import { useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";

/**
 * AuthTokenSync - Syncs Convex auth token to Rust backend
 *
 * When user is authenticated, gets the token from Convex client
 * and passes it to Rust via set_convex_auth Tauri command.
 */
function AuthTokenSync({ children }: { children: React.ReactNode }) {
  const { isAuthenticated, isLoading } = useConvexAuth();

  useEffect(() => {
    const syncToken = async () => {
      if (isAuthenticated) {
        // Get the auth token from Convex client
        // The ConvexReactClient stores the token internally
        // We can access it via the _authToken property (internal API)
        const client = convexClient as any;
        const token = client._authToken;

        if (token) {
          try {
            await invoke("set_convex_auth", { token });
            console.log("Auth token synced to Rust backend");
          } catch (error) {
            console.error("Failed to sync auth token to Rust:", error);
          }
        } else {
          console.warn("User authenticated but no token found - this may indicate an auth issue");
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
  }, [isAuthenticated, isLoading]);

  return <>{children}</>;
}

/**
 * AuthDebugger - Logs auth state for debugging
 */
function AuthDebugger({ children }: { children: React.ReactNode }) {
  const { isAuthenticated, isLoading } = useConvexAuth();

  useEffect(() => {
    console.log("[AUTH DEBUG] Auth state:", { isAuthenticated, isLoading });
    if (isAuthenticated) {
      const client = convexClient as any;
      console.log("[AUTH DEBUG] Has token:", !!client._authToken);
    }
  }, [isAuthenticated, isLoading]);

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
