# PostHog backend usage (PHP / Laravel)

[PostHog](https://posthog.com) provides analytics, custom event capture, feature flags, and more for this Laravel + Inertia.js app. This doc describes the **backend** (PHP/Laravel) integration. For the **React (Inertia + Vite)** frontend — provider, identify, capture, feature flags — see [POSTHOG_REACT.md](POSTHOG_REACT.md). This doc covers the backend integration added by the [PostHog Laravel wizard](https://posthog.com/docs/libraries/php) and how to use it.

## Installation (wizard)

The recommended way to install and wire PostHog is the official wizard (Laravel + Inertia.js):

```bash
cd apps/openagents.com
npx -y @posthog/wizard@latest --region us
```

The wizard will:

- Detect Laravel with Inertia.js and read the project (`.env` is not sent off-machine).
- Install the PostHog PHP package via Composer.
- Create `config/posthog.php` and add PostHog env vars to `.env`.
- Create `app/Services/PostHogService.php` (wrapper for identify, capture, feature flags).
- Initialize PostHog in `app/Providers/AppServiceProvider.php` (`configurePostHog()`).
- Add `getPostHogProperties()` to `app/Models/User.php` for person properties.
- Insert capture/identify calls in auth, chat, runs, and settings (see [Events integrated](#events-integrated)).
- Optionally install the PostHog MCP server for Cursor, VS Code, Zed, or Codex CLI.

After running it, review the diff and ensure `.env` has `POSTHOG_API_KEY` (and optionally `POSTHOG_HOST`, `POSTHOG_DISABLED`) for production.

### Manual installation

To add only the PHP SDK without the wizard:

```bash
composer require posthog/posthog-php
```

Then add config, a service wrapper, and initialization as in the files the wizard creates (see [Configuration](#configuration) and [Usage](#usage)).

## Configuration

- **Config file:** `config/posthog.php`  
  - `api_key` from `POSTHOG_API_KEY`  
  - `host` from `POSTHOG_HOST` (default `https://us.i.posthog.com`)  
  - `disabled` from `POSTHOG_DISABLED` (default `true` for `local/dev/testing/staging`, `false` for `production`)  
  - `debug` from `APP_DEBUG`

- **Initialization:** `AppServiceProvider::boot()` calls `configurePostHog()`, which runs `PostHog::init()` when `posthog.disabled` is false and `posthog.api_key` is set. Use the same US host as the frontend so all data stays in one project.

- **Environment:** Set `POSTHOG_API_KEY` in `.env` (and in production via your deploy process). Do not commit the key. For production, see `docs/PRODUCTION_ENV_AND_SECRETS.md` and use `apply-production-env.sh`, which syncs `POSTHOG_API_KEY` into Secret Manager and binds it to Cloud Run automatically when present in `.env.production`.

## Events integrated

The app emits backend events for auth, chat/runs, settings, and API mutations:

| Event | Location | Properties (typical) |
|-------|----------|----------------------|
| `login code sent` | `EmailCodeAuthController::sendCode` and `ChatLoginTool` | `method` |
| `user signed up` | `EmailCodeAuthController::verifyCode` / `ChatLoginTool` | `signup_method` |
| `user logged in` | `EmailCodeAuthController::verifyCode` / `ChatLoginTool` | `login_method` |
| `user logged out` | `routes/auth.php` logout route | — |
| `chat started` | `Api\\ChatController::store` | `conversation_id` |
| `chat message sent` | `ChatApiController::stream` | `conversation_id`, `message_length` |
| `chat run completed` | `RunOrchestrator` | `run_id`, `thread_id`, latency/usage |
| `chat run failed` | `RunOrchestrator` | `run_id`, `thread_id`, error |
| `l402 payment made` | `RunOrchestrator` (paid receipts) | `run_id`, `thread_id`, amount/proof |
| `profile updated` | `ProfileController::update` and `Api\\ProfileController::update` | changed fields |
| `account deleted` | `ProfileController::destroy` and `Api\\ProfileController::destroy` | — |
| `agent_payments.*` | `Api\\AgentPaymentsController` | wallet lifecycle, invoice/payment/send-spark success + failures |
| `l402.paywall_*` | `Api\\L402PaywallController` | create/update/delete and reconcile failures |
| `shouts.created` | `Api\\ShoutsController::store` | `shoutId`, `zone`, `bodyLength` |
| `whispers.*` | `Api\\WhispersController` | list viewed, sent, marked-read |
| `autopilot.*` | `Api\\AutopilotController` | list/view/create/update/thread actions |

On login/verify, the app also calls `PostHogService::identify($user->email, $user->getPostHogProperties())` so person profiles get email, name, and `date_joined`.


Prefer the **PostHogService** wrapper (inject or `resolve(PostHogService::class)`): it respects `posthog.disabled` and uses `config/posthog.*`.

```php
use App\Services\PostHogService;

// In a controller or route
$posthog = resolve(PostHogService::class);
$posthog->capture($user->email, 'thing happened', ['key' => 'value']);
$posthog->identify($user->email, $user->getPostHogProperties());
```

For one-off or non-service usage you can still use the client directly:

```php
use PostHog\PostHog;

PostHog::capture([
    'distinctId' => $distinctId,
    'event' => 'event_name',
    'properties' => [],
]);
```

Use the authenticated user’s ID (e.g. `$user->email`) or a stable anonymous ID as `distinctId`.

## Capturing events

Use **PostHogService** in app code:

```php
$posthog->capture($distinctId, 'user_signed_up', ['login_type' => 'email']);
```

Or the client directly:

```php
PostHog::capture([
    'distinctId' => $distinctId,
    'event' => 'user_signed_up',
    'properties' => [],
]);
```

**Tip:** Use `[object] [verb]` for event names (e.g. `project created`, `user signed up`, `invite sent`).

**Exception tracking:** `PostHogService::captureException($exception, $distinctId)` sends a `$exception` event with type, message, file, line, and stack trace. Omit `$distinctId` to use the current user’s email or `'anonymous'`.

### Event properties

Add a `properties` object for extra context:

```php
PostHog::capture([
    'distinctId' => $userId,
    'event' => 'user_signed_up',
    'properties' => [
        'login_type' => 'email',
        'is_free_trial' => true,
    ],
]);
```

### Backend-only pageviews

If you are not sending pageviews from the frontend, you can send them from the backend:

```php
PostHog::capture([
    'distinctId' => $userId,
    'event' => '$pageview',
    'properties' => [
        '$current_url' => 'https://example.com/path',
    ],
]);
```

## Person profiles and properties

The app identifies users on login with `$user->getPostHogProperties()` (see `User::getPostHogProperties()`: email, name, `date_joined`). Identified events create person profiles. To set [person properties](https://posthog.com/docs/data/user-properties) manually, use `$set` and `$set_once` in `properties`:

```php
PostHog::capture([
    'distinctId' => $distinctId,
    'event' => 'event_name',
    'properties' => [
        '$set' => [
            'name' => 'Max Hedgehog',
        ],
        '$set_once' => [
            'initial_url' => '/blog',
        ],
    ],
]);
```

To send anonymous events **without** creating/updating a person profile:

```php
PostHog::capture([
    'distinctId' => $distinctId,
    'event' => 'event_name',
    'properties' => [
        '$process_person_profile' => false,
    ],
]);
```

## Alias

Link another distinct ID to the same user (e.g. when the frontend distinct ID is not available on the backend):

```php
PostHog::alias([
    'distinctId' => 'distinct_id',
    'alias' => 'alias_id',
]);
```

See [PostHog: Alias](https://posthog.com/docs/data/identify#alias-assigning-multiple-distinct-ids-to-the-same-user) for when and how to use this.

## Feature flags

Use [feature flags](https://posthog.com/docs/feature-flags) for rollouts and targeting. **PostHogService** exposes `isFeatureEnabled()` and `getFeatureFlagPayload()` (they no-op when `posthog.disabled` is true).

**Boolean:**

```php
$enabled = $posthog->isFeatureEnabled('flag-key', $distinctId);
// or: PostHog::isFeatureEnabled('flag-key', $distinctId);
if ($enabled) {
    // new behavior
}
```

**Multivariate / payload:**

```php
$variant = PostHog::getFeatureFlag('flag-key', $distinctId);
$payload = $posthog->getFeatureFlagPayload('flag-key', $distinctId);
if ($variant === 'variant-key') {
    // variant behavior
}
```

**Include flag in events (for insights):** Add the variant to event properties so breakdowns work:

```php
PostHog::capture([
    'distinctId' => $distinctId,
    'event' => 'event_name',
    'properties' => [
        '$feature/feature-flag-key' => $variant, // or from getFeatureFlag()
    ],
]);
```

Optional: set `send_feature_flags => true` on `capture()` to auto-attach flags (adds an extra request). For timeouts and local evaluation, see the [PHP SDK docs](https://posthog.com/docs/libraries/php) and [local evaluation](https://posthog.com/docs/feature-flags/local-evaluation).

## Group analytics

Associate events with a group (e.g. company):

```php
PostHog::capture([
    'distinctId' => $userId,
    'event' => 'some_event',
    '$groups' => ['company' => $companyId],
]);
```

Update group properties:

```php
PostHog::groupIdentify([
    'groupType' => 'company',
    'groupKey' => $companyId,
    'properties' => ['name' => 'Awesome Inc.', 'employees' => 11],
]);
```

Group analytics is a paid feature; see [PostHog pricing](https://posthog.com/pricing).

## Config options (PostHog::init)

Pass options into `PostHog::init()` as the second argument (AppServiceProvider uses `config/posthog.php`):

| Option | Description |
|--------|-------------|
| `host` | PostHog instance URL (e.g. `https://us.i.posthog.com`) |
| `debug` | `true` to log debug output |
| `timeout` | Request timeout in ms (default 10000) |
| `feature_flag_request_timeout_ms` | Timeout for feature-flag requests (default 3000) |
| `consumer` | Transport: `socket`, `file`, `lib_curl`, `fork_curl` |
| `verify_batch_events_request` | `true` = synchronous delivery check; `false` = fire-and-forget |

Example with debug and timeout:

```php
PostHog::init(env('POSTHOG_API_KEY'), [
    'host' => 'https://us.i.posthog.com',
    'debug' => config('app.debug'),
    'feature_flag_request_timeout_ms' => 3000,
]);
```

## Inertia.js and React frontend

- **Frontend integration:** This app uses PostHog in React (Inertia + Vite): `PostHogProvider` and `usePostHog` in the frontend, with automatic identify when `auth.user` is present. See **[POSTHOG_REACT.md](POSTHOG_REACT.md)** for setup, env vars (`VITE_POSTHOG_KEY`, `VITE_POSTHOG_HOST`), capture, feature flags, and visibility tracking.
- **MCP:** The wizard can optionally install the PostHog MCP server for Cursor, VS Code, Zed, or Codex CLI (Dashboards, Insights, Experiments, etc.). Re-run the wizard and choose the MCP option, or install it separately.

## Next steps

For more detail on specific PostHog features (analytics, feature flags, A/B tests, etc.) in Laravel, see the [PHP SDK docs](https://posthog.com/docs/libraries/php) and these tutorials:

- [Set up analytics in Laravel](https://posthog.com/tutorials/laravel-analytics)
- [Set up feature flags in Laravel](https://posthog.com/tutorials/laravel-feature-flags)
- [Set up A/B tests in Laravel](https://posthog.com/tutorials/laravel-ab-tests)

## References

- [PostHog PHP library docs](https://posthog.com/docs/libraries/php)
- [PostHog project settings](https://us.posthog.com/project/settings) (API key and host)
- [Events and properties](https://posthog.com/docs/data/events)
- [Feature flags](https://posthog.com/docs/feature-flags)
