# @openagentsinc/social

Social/nostr UI and logic extracted from `apps/web`. The code is preserved here but **not currently shown** in the web app.

## Contents

- **components/nostr** – Nostr feed, posts, profile, communities, relay settings
- **components/nostr-grid** – Home grid, communities graph
- **hooks** – useAuthorPosts, useBatchAuthors, useCommunityPosts, useNostrFeedSubscription, etc.
- **lib** – nostrConvex, nostrPool, nostrQuery, relayConfig, clawstr, publishKind1111, etc.
- **contexts** – RelayConfigContext

## Re-enabling

To show social again in the web app:

1. Add this package as a dependency of `apps/web` (e.g. `"@openagentsinc/social": "workspace:*"`).
2. Resolve imports: package code uses `@/` paths that pointed into the app; either switch to relative paths and inject Convex/UI, or re-copy the code into the app.
3. Restore routes: `/`, `/feed`, `/c`, `/c/$community`, `/posts/$id`, `/event/$id`, `/u/$npub`.
4. Restore right sidebar (Feed / Communities) and wrap the app with `RelayConfigProvider` and Convex provider where needed.
