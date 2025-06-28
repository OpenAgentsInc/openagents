# OpenAgents.com - Chat Your Apps Into Existence

This is the main OpenAgents web application powered by [Convex](https://convex.dev/), [Next.js](https://nextjs.org/), and GitHub OAuth authentication.

## Tech Stack

- **Backend**: Convex (database, server logic, real-time sync)
- **Frontend**: React 19 + Next.js 15 (App Router)
- **UI**: Arwes cyberpunk theme + Tailwind CSS
- **Authentication**: GitHub OAuth via [Convex Auth](https://labs.convex.dev/auth)
- **AI Integration**: AI SDK with OpenRouter provider

## Get started

If you just cloned this codebase and didn't use `npm create convex`, run:

```
npm install
npm run dev
```

If you're reading this README on GitHub and want to use this template, run:

```
npm create convex@latest -- -t nextjs-convexauth
```

## Learn more

To learn more about developing your project with Convex, check out:

- The [Tour of Convex](https://docs.convex.dev/get-started) for a thorough introduction to Convex principles.
- The rest of [Convex docs](https://docs.convex.dev/) to learn about all Convex features.
- [Stack](https://stack.convex.dev/) for in-depth articles on advanced topics.
- [Convex Auth docs](https://labs.convex.dev/auth) for documentation on the Convex Auth library.

## GitHub OAuth Setup

The app uses GitHub OAuth for authentication. To set it up:

### 1. Create GitHub OAuth Apps

You'll need separate OAuth apps for development and production:

#### Development OAuth App
1. Go to [GitHub Developer Settings](https://github.com/settings/developers)
2. Click "New OAuth App"
3. Fill in:
   - **Application name**: `OpenAgents Dev`
   - **Homepage URL**: `http://localhost:3000`
   - **Authorization callback URL**: `https://[your-dev-convex-url].convex.cloud/api/auth/callback/github`
4. Save the Client ID and Client Secret

#### Production OAuth App
1. Create another OAuth app with:
   - **Application name**: `OpenAgents`
   - **Homepage URL**: `https://openagents.com` (or your production URL)
   - **Authorization callback URL**: `https://[your-prod-convex-url].convex.cloud/api/auth/callback/github`

### 2. Set Environment Variables

```bash
# Development
npx convex env set AUTH_GITHUB_ID <dev-client-id>
npx convex env set AUTH_GITHUB_SECRET <dev-client-secret>

# Production
npx convex env set AUTH_GITHUB_ID <prod-client-id> --prod
npx convex env set AUTH_GITHUB_SECRET <prod-client-secret> --prod
```

### 3. Find Your Convex URL

To get your Convex deployment URL for the callback:
```bash
npx convex dashboard
```

The URL will be in the format: `https://[deployment-name].convex.cloud`

## Configuring other authentication methods

To add additional authentication methods, see [Configuration](https://labs.convex.dev/auth/config) in the Convex Auth docs.

## Join the community

Join thousands of developers building full-stack apps with Convex:

- Join the [Convex Discord community](https://convex.dev/community) to get help in real-time.
- Follow [Convex on GitHub](https://github.com/get-convex/), star and contribute to the open-source implementation of Convex.
