import { createAuthClient } from "better-auth/react"; // Use the React client
import { genericOAuthClient } from "better-auth/client/plugins"

export const authClient: ReturnType<typeof createAuthClient> = createAuthClient({
  plugins: [
    genericOAuthClient(), // Add OAuth2 client plugin
  ],
});

// Export specific methods for convenience
export const {
  signIn,
  signUp,
  signOut,
  getSession,
  useSession,
} = authClient;
