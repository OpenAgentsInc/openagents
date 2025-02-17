# Implementing OAuth in a Rust Backend with a React/Vite Frontend Using Axum

Implementing OAuth2 authentication in a Rust backend (using Axum) with a React/Vite frontend involves coordinating the OAuth flow across multiple providers, maintaining user sessions securely, and structuring your code on both server and client. Below are best practices and recommendations in key areas:

---

## Supporting Multiple OAuth Providers (Google, GitHub, etc.)

- **Abstract Provider Configurations:**
  Design your Rust backend to handle multiple OAuth providers by abstracting provider-specific details (client IDs, secrets, authorization and token URLs, scopes). For example, define an enum or configuration struct for each provider containing its OAuth endpoints and scopes. This allows you to use the same code path for the OAuth flow with different provider settings.

- **Multiple Auth Endpoints:**
  Expose distinct endpoints for each provider’s login and callback. For instance:
  - `/auth/github/login` and `/auth/github/callback`
  - `/auth/google/login` and `/auth/google/callback`
  Each login endpoint initiates the OAuth redirect to the provider’s authorization URL, and each callback endpoint processes the provider’s response.

- **User Account Linking/Creation:**
  After a successful OAuth login, create or update a user record in your database for that provider. Use the provider name and the user’s external ID (e.g., GitHub user ID, Google sub) in the user record for future reference.

- **Unified Flow Logic:**
  Encapsulate the common OAuth flow: redirecting to the provider, exchanging the code for tokens, fetching user info, and creating a session. This can be implemented in a shared function or an auth service module. By centralizing this logic, adding a new provider becomes mainly a matter of adding its configuration.

---

## Session Token Management on the Backend

- **Session vs. Token-Based Auth:**
  Decide whether to use server-side sessions or token-based stateless authentication. In a session-based approach, the server tracks logged-in users (e.g., via an in-memory store or database) and issues a cookie as a session identifier. In a token-based approach, the server issues a JWT (or similar token) that encodes the session state. Many Rust examples prefer JWTs for stateless authentication. With JWTs, generate a token containing a user identifier (and an expiration claim) after OAuth login. The backend can use the [`jsonwebtoken`](https://crates.io/crates/jsonwebtoken) crate to create and sign the token. Store this JWT in an HTTP-only cookie before redirecting the user back to the frontend.

- **Using Server Sessions:**
  If you prefer a server-managed session, consider using middleware (for example, with [`axum-extra`](https://crates.io/crates/axum-extra)'s session support) or a database-backed session store. Generate a random session ID, store session data (like user ID and provider tokens) on the server, and send the session ID in a cookie. This approach allows easy server-side invalidation of sessions.

- **Cookie Configuration:**
  Configure cookies with security in mind. Mark the session cookie as **HttpOnly** (to prevent access via JavaScript) and set the **Secure** flag in production (ensuring it’s only sent over HTTPS). If your frontend and backend are on different domains or ports (common in development), set `SameSite=None` with `Secure=true` in production. During development on HTTP, adjust these settings appropriately.

---

## Security Considerations

- **CSRF Protection with State Parameter:**
  Always use the OAuth2 `state` parameter when redirecting to the OAuth provider. Generate a random token on the initial login request and verify it upon callback. This prevents CSRF attacks by ensuring the callback is tied to an initiated login attempt. You can store the generated state in a server-side session or in a temporary cookie.

- **OAuth Flow Security:**
  Use the Authorization Code flow (with PKCE if the frontend initiates it) rather than the implicit flow. Perform the code-to-token exchange on the backend so that your OAuth client secret is never exposed to the frontend.

- **Secure Storage of Credentials:**
  Store sensitive credentials (OAuth client secrets, JWT signing keys) in secure configuration (e.g., environment variables or a secrets manager). Do not hard-code these values or expose them in the frontend. Use crates like [`dotenv`](https://crates.io/crates/dotenv) to load these values at runtime.

- **Scope and Permissions:**
  Request only the minimal OAuth scopes needed for your app. For example, for basic profile info, request the `email` and `profile` scopes. Limiting scopes reduces exposure of user data.

- **HTTPS and SameSite:**
  Ensure that your OAuth flow runs over HTTPS in production. OAuth tokens and session cookies should only be transmitted over secure connections. Use appropriate SameSite cookie settings (typically `Lax` for OAuth flows).

- **Logout and Token Revocation:**
  Provide a logout endpoint that clears the session cookie or invalidates the session token. If applicable, also revoke the provider’s token by calling its revocation endpoint.

---

## Recommended Rust OAuth Libraries (vs. Rolling Your Own)

- **Using an OAuth2 Crate:**
  The [`oauth2` crate](https://crates.io/crates/oauth2) is a widely used, production-ready library for OAuth2 in Rust. It provides an extensible, strongly-typed API for setting up OAuth clients, generating authorization URLs (with state and PKCE), handling token exchanges, and refreshing tokens. It supports async operations and is framework-agnostic, so it works well with Axum.

- **Other Libraries:**
  If you plan to integrate with OpenID Connect (OIDC), consider the [`openidconnect` crate](https://crates.io/crates/openidconnect) for handling ID tokens and provider discovery. For service accounts, [`yup-oauth2`](https://crates.io/crates/yup-oauth2) might be useful, though it’s more suited for server-to-server authentication.

- **Rolling Your Own:**
  It’s possible to implement OAuth2 without a specialized crate by using an HTTP client like [`reqwest`](https://crates.io/crates/reqwest) to make HTTP requests to OAuth endpoints and a JWT library for token handling. However, using a well-tested library (like `oauth2`) is generally recommended to avoid subtle bugs.

- **Combining Approaches:**
  Use the OAuth library for core operations (building URLs, exchanging tokens) and manage session/database logic yourself. For example, use the library to obtain the authorization URL and token response, then create or update your user record and generate a session token (JWT or session ID) for your own application.

---

## Handling the OAuth Flow: Frontend to Backend

Managing the OAuth flow across the React frontend and Rust backend (using Axum) requires careful coordination. Here’s a typical flow and how to implement it:

1. **User Initiates Login (Frontend):**
   In your React app, provide buttons for each login provider (e.g., “Login with GitHub”, “Continue with Google”). When clicked, you have two main options:

   - **Redirect to Backend Login Endpoint:**
     Have the React code set `window.location.href` to something like `"<BACKEND_URL>/auth/github/login"`. The Rust backend’s `/auth/github/login` handler constructs the GitHub authorization URL (using the client ID, redirect URI, scopes, and a generated state) and responds with an HTTP redirect.

   - **Direct to Provider (with Frontend State):**
     Alternatively, the frontend can construct the provider’s URL if you expose the client ID (never the secret) in your config. This is less secure than having the backend initiate the flow.

2. **User Authorizes and Provider Redirects (Provider → Backend):**
   The OAuth provider prompts the user to sign in and authorize your app. After consent, it redirects the user to the **redirect URI** you specified (e.g., `/auth/github/callback`) with an authorization `code` and the `state` parameter.
   Your Axum handler for this route should:
   - Verify the `state` parameter to prevent CSRF.
   - Exchange the authorization code for tokens using the provider’s token endpoint.
   - Parse the token response and handle errors if the exchange fails.

3. **Fetch User Info & Finalize Login (Backend):**
   With the access (and possibly ID) token, the backend fetches the user’s profile information from the provider. Then:
   - **Find or Create the User:**
     Use the provider’s unique ID (or email) to find an existing user in your database. If the user doesn’t exist, create a new record.
   - **Generate a Session Token:**
     Create a session token (typically a JWT) that includes the user’s ID and claims. Sign this token with your secret key.
   - **Set the Cookie:**
     Attach the session token in an HTTP-only cookie on the response. In Axum, use the response builder to set a cookie with properties like `HttpOnly`, `Secure`, and appropriate `SameSite` settings.
   - **Redirect to Frontend:**
     Redirect the user’s browser back to the frontend (e.g., to `/profile` or the chat interface) using a 302 redirect. If you stored a “post-login redirect path” in the `state` parameter, use it here.

4. **Frontend Receives Redirect and Session:**
   Once redirected, the user returns to your React app. The session cookie is automatically sent with requests to the backend.
   - On the protected route (e.g., a profile or chat page), immediately make a request to an endpoint like `/api/users/me` (with `credentials: 'include'`) to confirm the session and retrieve user data.
   - Store the returned user data in a React context or global state, so your UI knows the user is authenticated.
   - Use this state to protect further routes and content.

5. **Logout Flow:**
   Implement a logout endpoint on the backend (e.g., `/auth/logout`) that clears the session cookie (by setting an expired cookie). On the frontend, provide a logout button that calls this endpoint and then updates the client’s state accordingly.

---

## Structuring the Code in Rust (Axum) and React

**Rust Backend (Axum):**

- **Configuration:**
  - Store OAuth credentials (client IDs, secrets, redirect URIs) in configuration files or environment variables.
  - Load these at startup into a configuration struct (e.g., `Config { google_client_id, google_client_secret, github_client_id, ... }`).
  - Also configure your JWT secret and token expiry settings here.

- **OAuth Routes:**
  - Implement route handlers for each provider’s login and callback in Axum. For example, `GET /auth/github/login` and `GET /auth/github/callback`.
  - Organize these handlers in a dedicated module (e.g., `auth_handlers.rs`).
  - Use Axum’s router to group these endpoints, for example:
    ```rust
    let app = Router::new()
      .nest("/auth", auth_routes)
      .route("/api/users/me", get_me_handler);
    ```
  - In the login handler, generate the authorization URL (using the `oauth2` crate or your own implementation) and issue a redirect.
  - In the callback handler, exchange the code for tokens, look up or create the user, generate a session token, set the cookie, and finally redirect the user back to the frontend.

- **State Management:**
  - Use Axum’s shared state (via `axum::extract::Extension`) to pass around your configuration, database connection pool, or OAuth client instances.

- **Session Verification Middleware:**
  - Implement middleware (or use an extractor) to verify the session token (e.g., decode the JWT from the cookie) on protected routes.
  - This middleware should validate the token’s signature and expiration, and then attach the user’s identity to the request context.

- **Database Integration:**
  - Set up your user model and database interactions (using an ORM like Diesel or SQLx) to store and manage user information.

- **Testing:**
  - Test the OAuth flow locally by running your Axum server and React dev server, and simulate OAuth logins using test credentials.

---

**React Frontend (Vite + React):**

- **Environment Config:**
  - Store the base URL of your backend API and any public OAuth configuration (like client IDs) in environment variables (e.g., `.env` files with `VITE_API_URL`).

- **Auth Context/State:**
  - Create a context or state management solution (using React Context, Zustand, or Redux) to manage authentication status and user data.
  - On app startup or on protected route load, call the backend (e.g., `/api/users/me`) to verify if the user is authenticated by sending credentials (with `fetch` or Axios configured with `credentials: 'include'`).

- **Login Buttons:**
  - In your login component, implement buttons for each OAuth provider. For example:
    ```jsx
    import { Github } from "lucide-react";
    import { Button } from "@/components/ui/button";

    function LoginScreen() {
      const handleGitHubLogin = () => {
        window.location.href = `${import.meta.env.VITE_API_URL}/auth/github/login`;
      };

      return (
        <div className="fixed inset-0 dark bg-black flex items-center justify-center">
          <Card className="-mt-12 w-full max-w-sm mx-4">
            <CardHeader className="text-center">
              <CardTitle className="text-2xl text-white">OpenAgents Chat</CardTitle>
            </CardHeader>
            <CardContent>
              <Button className="w-full" size="lg" onClick={handleGitHubLogin}>
                <Github />
                Log in with GitHub
              </Button>
            </CardContent>
          </Card>
        </div>
      );
    }
    ```
  - The backend login endpoint handles the redirect to GitHub (or another provider).

- **Handling Post-Login Redirect:**
  - Once the user is redirected back to the frontend, the session cookie is sent along with requests.
  - Immediately fetch the authenticated user’s data and update your auth context.

- **Protected Routes:**
  - Use React Router to guard routes that require authentication. For example, create a component that checks for authentication and either renders the protected component or redirects to the login page.

- **Logout Button:**
  - Provide a logout button that calls the backend logout endpoint, clears the auth context, and navigates the user back to the login page.

---

By following these practices – using a robust OAuth library (or carefully implementing the flow), securing session tokens, protecting against CSRF, and coordinating redirects – you can build a secure OAuth login in your Axum/React application. This structure (with the Rust backend handling multi-provider OAuth and session management, and the React frontend managing user interaction and state) scales well to additional providers and keeps sensitive logic on the server.

---

**Sources & References:**

- **OAuth2 Crate:** [https://crates.io/crates/oauth2](https://crates.io/crates/oauth2)
- **JSON Web Tokens:** [`jsonwebtoken` crate](https://crates.io/crates/jsonwebtoken)
- **Axum Documentation:** [https://docs.rs/axum](https://docs.rs/axum)
- **Session Management in Axum:** [`axum-extra`](https://crates.io/crates/axum-extra)
- **General OAuth2 Best Practices:** Refer to the OAuth2 RFC 6749 and community guides for secure implementation details.
