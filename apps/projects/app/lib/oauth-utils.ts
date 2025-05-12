/**
 * Utility functions for OAuth/OIDC flows
 */

// Base URL for OIDC endpoints
const baseURL = 'https://consentkeys.openagents.com';

/**
 * Generates a secure random string for use as state or nonce
 * @param length Length of the random string
 */
export function generateRandomString(length: number = 32): string {
  const array = new Uint8Array(length);
  crypto.getRandomValues(array);
  return Array.from(array, byte => byte.toString(16).padStart(2, '0')).join('').slice(0, length);
}

/**
 * Helper function to generate authorization URL for OAuth clients
 * This is used by client applications that want to authenticate with ConsentKeys
 */
export function generateAuthorizationUrl({
  clientId,
  redirectUri,
  state,
  scope = "openid profile email",
  nonce,
  prompt
}: {
  clientId: string;
  redirectUri: string;
  state: string;
  scope?: string;
  nonce?: string;
  prompt?: "consent" | "login" | "none" | "select_account";
}) {
  const authUrl = new URL("/api/auth/oauth2/authorize", baseURL);
  authUrl.searchParams.set("client_id", clientId);
  authUrl.searchParams.set("redirect_uri", redirectUri);
  authUrl.searchParams.set("response_type", "code");
  authUrl.searchParams.set("scope", scope);
  authUrl.searchParams.set("state", state);
  
  // Optional parameters
  if (prompt) authUrl.searchParams.set("prompt", prompt);
  if (nonce) authUrl.searchParams.set("nonce", nonce);
  
  return authUrl.toString();
}

/**
 * Sample code for how a client application would start the authorization flow
 */
export function startOAuthFlow(clientId: string, redirectUri: string) {
  // Generate a random state value for CSRF protection
  const state = generateRandomString();
  
  // Save the state in local storage to verify later
  localStorage.setItem('oauth_state', state);
  
  // Generate a nonce for replay protection (optional)
  const nonce = generateRandomString();
  localStorage.setItem('oauth_nonce', nonce);
  
  // Generate the authorization URL
  const authUrl = generateAuthorizationUrl({
    clientId,
    redirectUri,
    state,
    scope: "openid profile email",
    nonce,
    // Force consent screen if desired
    // prompt: "consent" 
  });
  
  // Redirect to authorization endpoint
  window.location.href = authUrl;
}

/**
 * Sample code for how a client application would handle the callback
 */
export async function handleOAuthCallback(code: string) {
  // Get saved state from local storage
  const savedState = localStorage.getItem('oauth_state');
  const currentState = new URLSearchParams(window.location.search).get('state');
  
  // Verify state matches to prevent CSRF
  if (savedState !== currentState) {
    throw new Error('OAuth state mismatch');
  }
  
  // Exchange code for tokens
  const tokenUrl = `${baseURL}/api/auth/oauth2/token`;
  const response = await fetch(tokenUrl, {
    method: "POST",
    headers: {
      "Content-Type": "application/x-www-form-urlencoded"
    },
    body: new URLSearchParams({
      grant_type: "authorization_code",
      code,
      client_id: "YOUR_CLIENT_ID", // Replace with actual client ID
      client_secret: "YOUR_CLIENT_SECRET", // Replace with actual client secret
      redirect_uri: "YOUR_REDIRECT_URI" // Replace with actual redirect URI
    })
  });
  
  const tokens = await response.json();
  
  // Store tokens securely
  localStorage.setItem('access_token', tokens.access_token);
  localStorage.setItem('refresh_token', tokens.refresh_token);
  localStorage.setItem('id_token', tokens.id_token);
  
  return tokens;
}

/**
 * Sample code for how a client application would get user info
 */
export async function getUserInfo(accessToken: string) {
  const userInfoUrl = `${baseURL}/api/auth/oauth2/userinfo`;
  const response = await fetch(userInfoUrl, {
    headers: {
      "Authorization": `Bearer ${accessToken}`
    }
  });
  
  return await response.json();
}