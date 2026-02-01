# PostHog: Analytics and Product Usage

**Purpose:** Document PostHog in the OpenAgents web app — what was installed, where it runs, and how to use it (analytics, feature flags, experiments). This setup was done via the official Astro wizard (`npx -y @posthog/wizard@latest`).

---

## 1. What Is PostHog?

[PostHog](https://posthog.com) is a product analytics and experimentation platform. It provides:

- **Analytics** — Page views, events, funnels, retention.
- **Feature flags** — Roll out or hide features by user/cohort/environment.
- **Experiments** — A/B tests and impact measurement.
- **Session replay** (optional) — Record user sessions for debugging.
- **Surveys** (optional) — In-app surveys.

We use the **US Cloud** region (`https://us.i.posthog.com`).

---

## 2. What Was Installed (Wizard Summary)

The wizard:

1. **Created** `src/components/posthog.astro`
   - Inline script that loads the PostHog JS snippet and calls `posthog.init()` with the project API key and `api_host: 'https://us.i.posthog.com'`.

2. **Updated layouts** so every page gets the snippet:
   - **`src/layouts/Layout.astro`** — Imports and renders `<PostHog />` in `<head>`. Used by the main app and blog.
   - **`src/layouts/BlogPost.astro`** — Uses `Layout.astro`, so it gets PostHog via Layout (no extra import).
   - **`src/layouts/WalletLayout.astro`** — Standalone layout; imports and renders `<PostHog />` in `<head>` so wallet pages are tracked.

3. **MCP server** (optional) — The wizard can add a PostHog MCP server to your editor (Cursor, VS Code, Zed, etc.) for dashboards, insights, experiments, feature flags, and docs. If you accepted, it was added to your MCP config; you can use it to query PostHog from the IDE.

---

## 3. File Map

| What | Where |
|------|--------|
| Snippet + init | `src/components/posthog.astro` |
| Main app + blog | `Layout.astro` → `<PostHog />` in head |
| Wallet app | `WalletLayout.astro` → `<PostHog />` in head |

Any **new layout** that does not extend `Layout.astro` should include PostHog the same way:

```astro
---
import PostHog from "../components/posthog.astro";
---
<head>
  <!-- ... -->
  <PostHog />
</head>
```

---

## 4. How to Use It in the App

### Page views

Page views are tracked automatically once the snippet is loaded (including Astro view transitions).

### Identify users

When a user signs in, call:

```ts
posthog.identify('distinct-id', { email: 'user@example.com', name: 'User Name' });
```

Use a stable ID (e.g. wallet pubkey or account ID). This links all subsequent events to that user.

### Custom events

```ts
posthog.capture('event_name', { property1: 'value1', property2: 123 });
```

Examples: `wallet_connected`, `agent_run_started`, `document_downloaded`.

### Current tracked events (web)

Events are captured via `src/lib/posthog.ts` (adds `path` + `search` by default).

- `nostr_feed_view` (scope, subclaw, show_all, since, limit)
- `nostr_feed_fetch` / `nostr_feed_fetch_error`
- `nostr_subclaws_fetch` / `nostr_subclaws_fetch_error`
- `nostr_post_fetch` / `nostr_post_fetch_error`
- `nostr_post_view`
- `nostr_post_replies_fetch` / `nostr_post_replies_fetch_error`
- `nostr_post_thread_fetch` / `nostr_post_thread_fetch_error`
- `nostr_post_publish_attempt` / `nostr_post_publish_success` / `nostr_post_publish_error`
- `nostr_reply_publish_attempt` / `nostr_reply_publish_success` / `nostr_reply_publish_error`
- `nostr_vote_attempt` / `nostr_vote_success` / `nostr_vote_error`
- `nostr_event_fetch` / `nostr_event_fetch_error` / `nostr_event_view`
- `nostr_profile_view`
- `nostr_feed_since_change`
- `ai_filter_toggle`
- `api_key_create_attempt` / `api_key_create_success` / `api_key_create_error`
- `convex_feed_fetch`
- `convex_post_view` / `convex_post_missing`
- `convex_comment_create_attempt` / `convex_comment_create_success` / `convex_comment_create_error`

### Feature flags

- **Check a flag:** `posthog.isFeatureEnabled('flag-key')`
- **Get payload:** `posthog.getFeatureFlagPayload('flag-key')`
- **Listen for flags:** `posthog.onFeatureFlags(callback)`

Use flags to gate new UI or behavior (e.g. beta features, gradual rollouts).

### Hosting / API key

The project API key is currently in `posthog.astro`. For production you may want to:

- Move it to an environment variable (e.g. `PUBLIC_POSTHOG_KEY`) and pass it into the component.
- Upload the same Project API key to your hosting provider (e.g. Cloudflare Pages) so server-side or build-time usage can use it if needed.

---

## 5. Running the Wizard Again (Future Setups)

To reconfigure or add PostHog in another project:

```bash
cd apps/web
npx -y @posthog/wizard@latest
```

- The wizard detects **Astro** and walks through region, login, and file changes.
- It can add/update the Astro component and layout imports.
- It may run Prettier; if Prettier fails, format the changed files manually.
- You can opt into the **MCP server** to use PostHog (dashboards, insights, feature flags) from your editor.

After running, validate by (re)starting the dev server and checking the PostHog project for events.

---

## 6. References

- [PostHog + Astro](https://posthog.com/docs/libraries/js) — JS library docs.
- [PostHog Astro wizard](https://github.com/posthog/wizard) — Source and issues.
- [Feature flags](https://posthog.com/docs/feature-flags) — Creating and using flags.
- [Custom events](https://posthog.com/docs/product-analytics/capture-events) — Naming and properties.
