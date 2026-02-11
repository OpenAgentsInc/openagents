# Welcome to your Convex + TanStack Start + WorkOS AuthKit app

This is a [Convex](https://convex.dev/) project using WorkOS AuthKit for authentication.

After the initial setup (<2 minutes) you'll have a working full-stack app using:

- Convex as your backend (database, server logic)
- [React](https://react.dev/) as your frontend (web page interactivity)
- [TanStack Start](https://tanstack.com/start) for modern full-stack React with file-based routing
- [Tailwind](https://tailwindcss.com/) for building great looking accessible UI
- [WorkOS AuthKit](https://authkit.com/) for authentication
- [Effuse](https://github.com/OpenAgentsInc/openagents/tree/main/packages/effuse) for Effect-native UI: most route UIs are rendered by Effuse (marketing header and home/login, modules/signatures/tools catalogs, autopilot chat column); React provides the shell (backgrounds, sidebar, blueprint panel) and data/effects

## Get started

1. Clone this repository and install dependencies:

   ```bash
   npm install
   ```

2. Set up your environment variables:

   ```bash
   cp .env.local.example .env.local
   ```

   `.env.local.example` lists WorkOS, optional OpenAI, and other non-Convex vars. Convex URL is added by `npx convex dev`. For production, copy `.env.production.example` to `.env.production` and set prod WorkOS redirect URI (e.g. `https://openagents.com/callback`).

3. Configure WorkOS AuthKit:
   - Create a [WorkOS account](https://workos.com/)
   - Get your Client ID and API Key from the WorkOS dashboard
   - In the WorkOS dashboard, add `http://localhost:3000/callback` as a redirect URI
   - Generate a secure password for cookie encryption (minimum 32 characters)
   - Update your `.env.local` file with these values

4. Configure Convex:

   ```bash
   npx convex dev
   ```

   This will:
   - Set up your Convex deployment
   - Add your Convex URL to `.env.local`
   - Open the Convex dashboard

   Then set your WorkOS Client ID in Convex:

   ```bash
   npx convex env set WORKOS_CLIENT_ID <your_client_id>
   ```

   This allows Convex to validate JWT tokens from WorkOS

5. Run the development server:

   ```bash
   npm run dev
   ```

   This starts both the Vite dev server (TanStack Start frontend) and Convex backend in parallel

6. Open [http://localhost:3000](http://localhost:3000) to see your app

## WorkOS AuthKit Setup

This app uses WorkOS AuthKit for authentication. Key features:

- **Redirect-based authentication**: Users are redirected to WorkOS for sign-in/sign-up
- **Session management**: Automatic token refresh and session handling
- **Route loader protection**: Protected routes use loaders to check authentication
- **Client and server functions**: `useAuth()` for client components, `getAuth()` for server loaders

## Deploy

Full deploy (Convex + Vite build + Cloudflare Worker):

```bash
npm run deploy
```

**If it seems to time out after the Vite build:** the next step is `wrangler deploy`, which bundles the Worker (TypeScript + local packages) and uploads assets. That step can take **1–3+ minutes** with little output. Run it in a normal terminal (not an IDE runner with a short timeout). If the full deploy was interrupted after the build, you can finish with:

```bash
npm run deploy:worker
```

## L402 paywall control-plane API

Hosted paywall lifecycle endpoints are exposed from the Worker under `/api/lightning/paywalls`:

- `POST /api/lightning/paywalls` (create)
- `GET /api/lightning/paywalls` (list, supports `status` and `limit`)
- `GET /api/lightning/paywalls/:paywallId` (get)
- `PATCH /api/lightning/paywalls/:paywallId` (update policy/routes/metadata)
- `POST /api/lightning/paywalls/:paywallId/pause`
- `POST /api/lightning/paywalls/:paywallId/resume`

Error taxonomy is deterministic for programmatic callers:

- `401` `unauthorized`
- `400` `invalid_input`
- `403` `forbidden`
- `404` `paywall_not_found`
- `409` `invalid_transition` / `route_conflict`
- `422` `policy_violation`

Run the non-interactive API smoke check:

```bash
npm run smoke:l402-paywall-api
```

The smoke command outputs machine-readable JSON so agents can parse pass/fail.

## Learn more

To learn more about developing your project with Convex, check out:

- The [Tour of Convex](https://docs.convex.dev/get-started) for a thorough introduction to Convex principles.
- The rest of [Convex docs](https://docs.convex.dev/) to learn about all Convex features.
- [Stack](https://stack.convex.dev/) for in-depth articles on advanced topics.

## Join the community

Join thousands of developers building full-stack apps with Convex:

- Join the [Convex Discord community](https://convex.dev/community) to get help in real-time.
- Follow [Convex on GitHub](https://github.com/get-convex/), star and contribute to the open-source implementation of Convex.
