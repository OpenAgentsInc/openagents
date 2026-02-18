# PostHog React (frontend)

PostHog is integrated in this app’s **React (Inertia.js + Vite)** frontend for analytics, custom events, session recordings, feature flags, and more. The backend also sends events (see [POSTHOG_BACKEND.md](./POSTHOG_BACKEND.md)).

## In this app

- **Init:** `resources/js/lib/posthog.ts` — initializes PostHog when `VITE_POSTHOG_KEY` is set in non-development builds; uses `VITE_POSTHOG_HOST` (default `https://us.i.posthog.com`).
- **Provider:** `resources/js/app.tsx` — root is wrapped in `<PostHogProvider client={posthog}>`.
- **Identify:** `resources/js/components/posthog-identify.tsx` — runs on every page; calls `posthog.identify(user.email, { email, name })` when `auth.user` is present (Inertia shared props).
- **Env:** Add to `.env` (and to your hosting provider). Use the **same project API key and host** as the backend so data stays in one project.

### PostHog env checklist (backend + frontend)

| Var                 | Where                         | Purpose                                                  |
| ------------------- | ----------------------------- | -------------------------------------------------------- |
| `POSTHOG_API_KEY`   | Backend (`.env`)              | PHP/PostHogService; do not commit.                       |
| `POSTHOG_HOST`      | Backend (`.env`)              | Optional; default `https://us.i.posthog.com`.            |
| `POSTHOG_DISABLED`  | Backend (`.env`)              | Optional; set `true` to disable backend capture.         |
| `VITE_POSTHOG_KEY`  | Frontend (`.env`, build-time) | Same project key as backend; baked into assets at build. |
| `VITE_POSTHOG_HOST` | Frontend (`.env`, build-time) | Optional; default `https://us.i.posthog.com`.            |

Example (add to `.env`):

```bash
VITE_POSTHOG_KEY=<phc_...>
VITE_POSTHOG_HOST=https://us.i.posthog.com
```

## Installation (already done)

This app already has `posthog-js` and `@posthog/react` installed. To add them in another project:

```bash
npm install --save posthog-js @posthog/react
# or: pnpm add posthog-js @posthog/react  /  yarn add posthog-js @posthog/react  /  bun add posthog-js @posthog/react
```

Vite exposes any `VITE_*` env var to the client (see [Vite env](https://vitejs.dev/guide/env-and-mode.html)). This app uses `VITE_POSTHOG_KEY` and `VITE_POSTHOG_HOST`.

## Usage

### PostHog provider

The app root is wrapped with `PostHogProvider` in `app.tsx`. Do **not** import `posthog` directly in components; use the `usePostHog` hook so the client is always initialized.

### Calling PostHog methods

Use the `usePostHog` hook to capture events, identify users, or use feature flags. **Always use optional chaining** — PostHog can be undefined before init or if the key is missing.

```tsx
import { usePostHog } from '@posthog/react';

function MyComponent() {
    const posthog = usePostHog();

    const handleClick = () => {
        posthog?.capture('button_clicked', { section: 'pricing' });
    };

    return <button onClick={handleClick}>Click me</button>;
}
```

**Identify:** The app identifies logged-in users automatically in `PostHogIdentify` (using `auth.user` from Inertia). To set more properties or identify elsewhere:

```tsx
useEffect(() => {
    if (user) {
        posthog?.identify(user.id, { email: user.email, name: user.name });
    }
}, [posthog, user]);
```

**Don’t directly import PostHog** in components — use `usePostHog()` so the library is guaranteed to be initialized when used.

### TypeError: Cannot read properties of undefined

If you see this, you’re calling a PostHog method when `posthog` is still undefined (e.g. on first render). Fix by using optional chaining or a guard:

```tsx
useEffect(() => {
    posthog?.capture('test');
    // or
    if (posthog) posthog.capture('test');
}, [posthog]);
```

### Tracking element visibility

Use `PostHogCaptureOnViewed` to fire an event when an element scrolls into view (once per instance):

```tsx
import { PostHogCaptureOnViewed } from '@posthog/react';

<PostHogCaptureOnViewed name="hero-banner">
    <div>Your content</div>
</PostHogCaptureOnViewed>

<PostHogCaptureOnViewed name="product-card" properties={{ product_id: '123', category: 'electronics' }}>
    <ProductCard />
</PostHogCaptureOnViewed>
```

Use `trackAllChildren` to track each child separately (e.g. list items). You can pass `observerOptions` (e.g. `threshold`, `rootMargin`) for the IntersectionObserver.

## Feature flags

Use hooks or the `PostHogFeature` component. See [PostHog: Feature flags](https://posthog.com/docs/feature-flags).

| Hook                       | Description                                                     |
| -------------------------- | --------------------------------------------------------------- |
| `useFeatureFlagEnabled`    | Boolean; sends `$feature_flag_called`                           |
| `useFeatureFlagVariantKey` | Variant key; sends `$feature_flag_called`                       |
| `useFeatureFlagPayload`    | Payload; does **not** send exposure — use with one of the above |
| `useActiveFeatureFlags`    | List of active flags; no exposure event                         |

**Boolean example:**

```tsx
import { useFeatureFlagEnabled, useFeatureFlagPayload } from '@posthog/react';

function App() {
    const showWelcome = useFeatureFlagEnabled('show-welcome');
    const payload = useFeatureFlagPayload('show-welcome');
    return showWelcome ? <Welcome title={payload?.title} /> : null;
}
```

**Multivariate example:**

```tsx
import { useFeatureFlagVariantKey } from '@posthog/react';

const variant = useFeatureFlagVariantKey('experiment-key');
if (variant === 'variant-a') {
    /* ... */
}
```

**Component-based:**

```tsx
import { PostHogFeature } from '@posthog/react';

<PostHogFeature flag="show-welcome" match={true}>
    <Welcome />
</PostHogFeature>

<PostHogFeature flag="show-welcome" match="variant-b" fallback={<Default />}>
    {(payload) => <div>{payload?.message}</div>}
</PostHogFeature>
```

**Request timeout:** You can set `feature_flag_request_timeout_ms` in `posthog.init()` in `lib/posthog.ts` (default 3000 ms).

**Bootstrapping:** For redirects or critical UI that must know the flag before the first request, see [PostHog: Bootstrapping feature flags](https://posthog.com/docs/feature-flags/bootstrapping).

## Experiments (A/B tests)

Experiments use feature flags. Use `useFeatureFlagVariantKey` or `<PostHogFeature flag="experiment-key" match="variant-name">` as above. See [PostHog: Experiments](https://posthog.com/docs/experiments/manual) and [running experiments without feature flags](https://posthog.com/docs/experiments/running-experiments-without-feature-flags).

## Autocapture

By default, posthog-js autocaptures pageviews, clicks, and inputs. Configure or disable autocapture in `posthog.init()` in `lib/posthog.ts`; see [PostHog: Configuring autocapture](https://posthog.com/docs/product-analytics/autocapture#configuring-autocapture). Add `className="ph-no-capture"` to elements to exclude them.

## References

- [PostHog React library](https://posthog.com/docs/libraries/react)
- [posthog-js docs](https://posthog.com/docs/libraries/js)
- [PostHog PHP / backend](./POSTHOG_BACKEND.md) (this app)

## Explicit UI Events Tracked

This app now emits explicit high-signal events through `usePostHogEvent` (`resources/js/hooks/use-posthog-event.ts`) in addition to standard PostHog autocapture.

Chat surface events:

- `chat.page_opened`
- `chat.message_submitted`
- `chat.response_completed`
- `chat.response_empty`
- `chat.error_shown`
- `chat.error_dismissed`
- `chat.l402_approval_clicked`
- Guest onboarding: `chat.guest_email_invalid`, `chat.guest_code_sent`, `chat.guest_code_send_failed`, `chat.guest_code_invalid`, `chat.guest_code_verify_failed`, `chat.guest_code_verified`, `chat.guest_email_reset`

Homepage chat events:

- `home_chat.page_opened`
- `home_chat.suggestion_clicked`
- `home_chat.message_submitted`
- `home_chat.response_completed`
- `home_chat.response_empty`
- `home_chat.error_shown`
- `home_chat.error_dismissed`

Click coverage:

- PostHog autocapture remains enabled in production builds and captures general click interactions (including nav/link clicks) unless suppressed with `ph-no-capture`.
- The explicit events above capture intent-level actions in chat that are critical for funnel and failure analysis.

Each explicit event includes a `namespace` property plus contextual metadata such as `conversationId`, `characterCount`, `taskId`, `guestStep`, and status codes where applicable.
