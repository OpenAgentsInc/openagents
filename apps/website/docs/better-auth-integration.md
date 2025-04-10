# Better Auth Integration Guide for OpenAgents Website

This document summarizes the key aspects of integrating `better-auth` into the OpenAgents website (`apps/website/`), which uses React Router v7 and is deployed on Cloudflare Workers.

## 1. Overview

- **What:** `better-auth` is a framework-agnostic TypeScript authentication/authorization library.
- **Why:** Provides comprehensive features (email/password, social login, 2FA via plugins, etc.) out-of-the-box, allowing more control than third-party services and reducing boilerplate compared to rolling our own auth. It keeps user data within our chosen database.

## 2. Installation & Configuration

Follow these steps to set up `better-auth`:

**a. Install Package:**
```bash
npm install better-auth
# Or yarn add better-auth
```
Install in the `apps/website` directory.

**b. Environment Variables:**
Add the following to your `.env` file (or Cloudflare environment variables):
```env
# Generate a strong secret: openssl rand -base64 32
BETTER_AUTH_SECRET="YOUR_STRONG_SECRET_KEY"
# Base URL of the deployed website
BETTER_AUTH_URL="https://your-deployed-url.com" # Or http://localhost:xxxx for local dev
# Database connection (Example for Turso/LibSQL)
TURSO_DATABASE_URL="YOUR_TURSO_DB_URL"
TURSO_AUTH_TOKEN="YOUR_TURSO_AUTH_TOKEN"
# Social Provider Credentials (Example for GitHub)
GITHUB_CLIENT_ID="YOUR_GITHUB_CLIENT_ID"
GITHUB_CLIENT_SECRET="YOUR_GITHUB_CLIENT_SECRET"
```

**c. Server Instance (`app/lib/auth.ts`):**
Create `app/lib/auth.ts` (or similar path recognized by `better-auth`).

```typescript
// app/lib/auth.ts
import { betterAuth } from "better-auth";
import { LibsqlDialect } from "@libsql/kysely-libsql"; // Use LibSQL for CF Workers compatibility

// Configure the LibSQL dialect for Turso or local SQLite
const dialect = new LibsqlDialect({
    url: process.env.TURSO_DATABASE_URL!,
    authToken: process.env.TURSO_AUTH_TOKEN!,
});

export const auth = betterAuth({
    // Database Configuration (using LibSQL Dialect)
    database: {
        dialect,
        type: "sqlite", // Kysely needs the base type
    },

    // Email & Password Authentication
    emailAndPassword: {
        enabled: true,
        // autoSignIn: true, // Default: sign in user after successful sign up
    },

    // Social Providers (Example: GitHub)
    socialProviders: {
        github: {
            clientId: process.env.GITHUB_CLIENT_ID!,
            clientSecret: process.env.GITHUB_CLIENT_SECRET!,
        },
        // Add other providers (Google, etc.) here if needed
    },

    // Add plugins here if used (e.g., twoFactor())
    // plugins: [],
});

// Type definition for session user (optional but recommended)
export type SessionUser = Awaited<ReturnType<typeof auth.api.getSession>>["user"];
```
*Note: Ensure `@libsql/kysely-libsql` is installed (`npm install @libsql/kysely-libsql`).*

**d. Database Setup:**
Use the `better-auth` CLI to manage database schema. Run these commands from the `apps/website` directory:

```bash
# Generate migration SQL (optional, for review or manual application)
npx @better-auth/cli generate

# Apply migrations directly to the database (requires DB connection env vars)
npx @better-auth/cli migrate
```
Run `migrate` whenever the schema needs changes (e.g., adding plugins).

**e. API Route Handler:**
Create a catch-all route to handle `better-auth` API requests (default path: `/api/auth/*`). Since we use React Router v7 (similar to Remix) on Cloudflare, we can use or adapt the Remix helper.

Create `app/routes/api.auth.$.tsx`:
```typescript
// app/routes/api.auth.$.tsx
import { type LoaderFunctionArgs, type ActionFunctionArgs } from "@remix-run/cloudflare";
import { toRemixHandler } from "better-auth/remix"; // Use the Remix helper
import { auth } from "~/lib/auth"; // Adjust path if needed

// Handle GET requests (e.g., social auth callbacks)
export async function loader({ request }: LoaderFunctionArgs) {
  return toRemixHandler(auth)(request);
}

// Handle POST requests (e.g., email/pass sign in/up)
export async function action({ request }: ActionFunctionArgs) {
  return toRemixHandler(auth)(request);
}
```
*Note: Ensure `@remix-run/cloudflare` types are available or adapt using standard `Request` and `Response` if needed.*

**f. Client Instance (`app/lib/auth-client.ts`):**
Create `app/lib/auth-client.ts` for client-side interactions.

```typescript
// app/lib/auth-client.ts
import { createAuthClient } from "better-auth/react"; // Use the React client

export const authClient = createAuthClient({
    // baseURL: "http://localhost:xxxx", // Optional: Only needed if client/server domains differ
    // plugins: [], // Add client plugins here if used (e.g., twoFactorClient())
});

// Optional: Export specific methods for convenience
export const {
    signIn,
    signUp,
    signOut,
    getSession,
    useSession,
    // Add plugin methods here if used (e.g., twoFactor)
} = authClient;

```

## 3. Core Usage

**a. Client-Side Functions (React Components):**

- **Sign Up (Email):**
  ```typescript
  import { signUp } from "~/lib/auth-client";

  const handleSignUp = async (email, password, name) => {
    const { data, error } = await signUp.email({ email, password, name });
    if (error) console.error("Sign up failed:", error.message);
    else console.log("Sign up successful:", data); // User is likely auto-signed in
  };
  ```

- **Sign In (Email):**
  ```typescript
  import { signIn } from "~/lib/auth-client";

  const handleSignIn = async (email, password) => {
    const { data, error } = await signIn.email({ email, password, callbackURL: "/dashboard" });
    if (error) console.error("Sign in failed:", error.message);
    // On success, better-auth handles redirect via callbackURL or returns data
  };
  ```

- **Sign In (Social - e.g., GitHub):**
  ```typescript
  import { signIn } from "~/lib/auth-client";

  const handleGitHubSignIn = async () => {
    // This typically redirects the user to GitHub
    await signIn.social({ provider: "github", callbackURL: "/dashboard" });
  };
  ```

- **Sign Out:**
  ```typescript
  import { signOut } from "~/lib/auth-client";

  const handleSignOut = async () => {
    await signOut({
        fetchOptions: {
            onSuccess: () => { /* Redirect or update UI */ window.location.href = "/"; }
        }
    });
  };
  ```

- **Access Session (Hook):**
  ```typescript
  import { useSession } from "~/lib/auth-client";

  function UserProfile() {
    const { data: session, isPending, error } = useSession();

    if (isPending) return <div>Loading...</div>;
    if (error) return <div>Error loading session</div>;
    if (!session?.user) return <div>Not logged in</div>;

    return <div>Welcome, {session.user.name || session.user.email}!</div>;
  }
  ```

- **Access Session (Method):**
  ```typescript
  import { getSession } from "~/lib/auth-client";

  async function checkSession() {
    const { data: session, error } = await getSession();
    // ... use session data
  }
  ```

**b. Server-Side Session Access (Loaders/Actions):**

```typescript
// Example in a Remix/RRv7 loader
import { type LoaderFunctionArgs, json } from "@remix-run/cloudflare";
import { auth } from "~/lib/auth";

export async function loader({ request }: LoaderFunctionArgs) {
  const session = await auth.api.getSession({
    headers: request.headers, // Pass request headers
  });

  if (!session?.user) {
    // Handle unauthenticated user (e.g., redirect to login)
    // throw redirect("/login");
  }

  // User is authenticated, return data
  return json({ user: session.user });
}
```

## 4. Plugins (Example: 2FA)

Plugins extend functionality. Example steps for adding 2FA:

1.  **Install Plugin:** `npm install better-auth/plugins` (if not already part of core)
2.  **Server Config (`app/lib/auth.ts`):**
    ```typescript
    import { twoFactor } from "better-auth/plugins";
    // ... other imports

    export const auth = betterAuth({
        // ... other config
        plugins: [
            twoFactor() // Add the plugin instance
        ]
    });
    ```
3.  **Database Migration:** Run `npx @better-auth/cli migrate` to add 2FA tables.
4.  **Client Config (`app/lib/auth-client.ts`):**
    ```typescript
    import { twoFactorClient } from "better-auth/client/plugins";
    // ... other imports

    export const authClient = createAuthClient({
        // ... other config
        plugins: [
            twoFactorClient({
                twoFactorPage: "/verify-2fa" // Page for 2FA code entry
            })
        ]
    });
    ```
5.  **Usage:** Use plugin-specific methods (e.g., `authClient.twoFactor.enable()`, `authClient.twoFactor.verifyTOTP()`). Refer to the specific plugin documentation.

## 5. Project Specific Notes

- **Cloudflare Workers:** Database choice is crucial. LibSQL/Turso is a good option. Ensure environment variables are set correctly in Cloudflare dashboard or `wrangler.toml`.
- **React Router v7:** Use the Remix helpers/adapters provided by `better-auth` as RRv7 is very similar. Ensure the API catch-all route (`/api/auth/$`) is correctly set up.
- **UI Components:** Integrate sign-in/sign-up forms using shadcn/ui components. Use `useSession` hook to conditionally render UI based on auth state.
- **Error Handling:** Implement proper error handling and user feedback for all auth operations (sign in, sign up, etc.).
