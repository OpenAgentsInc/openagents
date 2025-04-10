import { createAuthClient } from "better-auth/react"; // Use the React client

export const authClient: ReturnType<typeof createAuthClient> = createAuthClient({
  // baseURL: "http://localhost:xxxx", // Optional: Only needed if client/server domains differ
  // plugins: [], // Add client plugins here if used (e.g., twoFactorClient())
});

// Export specific methods for convenience
export const {
  signIn,
  signUp,
  signOut,
  getSession,
  useSession,
} = authClient;
