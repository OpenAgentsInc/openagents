import { createAuthClient } from "better-auth/react"; // Use the React client
import { genericOAuthClient } from "better-auth/client/plugins"

// Import OIDC utility functions
import { 
  generateRandomString, 
  generateAuthorizationUrl, 
  startOAuthFlow, 
  handleOAuthCallback, 
  getUserInfo 
} from "./oauth-utils";

// Define a portable type for the auth client
type AuthClient = {
  signIn: any;
  signUp: any;
  signOut: any;
  getSession: any;
  useSession: any;
};

export const authClient: AuthClient = createAuthClient({
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

// Export OIDC utility functions for direct use in components
export {
  generateRandomString,
  generateAuthorizationUrl,
  startOAuthFlow,
  handleOAuthCallback,
  getUserInfo
};
