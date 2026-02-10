/**
 * These are configuration settings for the dev environment.
 *
 * Do not include API secrets in this file or anywhere in your JS.
 *
 * https://reactnative.dev/docs/security#storing-sensitive-info
 */
export default {
  API_URL: "https://api.rss2json.com/v1/",
  // Use production auth for dev by default; override with EXPO_AUTH_API_URL if needed.
  authApiUrl: "https://openagents.com",
}
