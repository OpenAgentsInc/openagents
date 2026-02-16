# PostHog backend usage (PHP / Laravel)

This app can use [PostHog](https://posthog.com) for server-side analytics via the official PHP SDK. The SDK batches events and flushes at the end of the request; it can run asynchronously.

## Installation

Add to `composer.json`:

```json
{
    "require": {
        "posthog/posthog-php": "3.0.*"
    }
}
```

Then:

```bash
composer install
```

## Initialization

Set your project API key **before** any PostHog calls. Prefer an environment variable (do not commit keys).

```php
use PostHog\PostHog;

PostHog::init(
    env('POSTHOG_API_KEY'), // e.g. from .env / production secrets
    [
        'host' => 'https://us.i.posthog.com',
    ]
);
```

Use the same US Cloud host (`us.i.posthog.com`) as the frontend (e.g. `apps/web`) so all data stays in one project. Get the project API key and host from [PostHog project settings](https://app.posthog.com/project/settings).

## Capturing events

Send custom events with `capture`:

```php
PostHog::capture([
    'distinctId' => 'distinct_id_of_the_user',
    'event' => 'user_signed_up',
]);
```

**Tip:** Use `[object] [verb]` for event names (e.g. `project created`, `user signed up`, `invite sent`).

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

Identified events create person profiles. Set [person properties](https://posthog.com/docs/data/user-properties) via `$set` and `$set_once` in `properties`:

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

Use [feature flags](https://posthog.com/docs/feature-flags) for rollouts and targeting.

**Boolean:**

```php
$enabled = PostHog::isFeatureEnabled('flag-key', $distinctId);
if ($enabled) {
    // new behavior
}
```

**Multivariate:**

```php
$variant = PostHog::getFeatureFlag('flag-key', $distinctId);
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

## Config options

Pass options into `PostHog::init()` as the second argument:

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

## Environment

- Add `POSTHOG_API_KEY` to `.env` (and to production via Secret Manager or your deploy process). Do not commit the key.
- For production, follow `docs/PRODUCTION_ENV_AND_SECRETS.md` and allowlist `POSTHOG_API_KEY` in the deploy script if it is stored in the env file used by `apply-production-env.sh`.

## References

- [PostHog PHP library docs](https://posthog.com/docs/libraries/php)
- [PostHog project settings](https://app.posthog.com/project/settings) (API key and host)
- [Events and properties](https://posthog.com/docs/data/events)
- [Feature flags](https://posthog.com/docs/feature-flags)
