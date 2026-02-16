# PostHog backend usage (PHP / Laravel)

[PostHog](https://posthog.com) can provide analytics, custom event capture, feature flags, and more for this Laravel app. This guide uses the [PostHog PHP SDK](https://posthog.com/docs/libraries/php).

## Installation

With [Composer](https://getcomposer.org/) installed, add the PHP SDK:

```bash
composer require posthog/posthog-php
```

Or add to `composer.json` and run `composer install`:

```json
{
    "require": {
        "posthog/posthog-php": "3.0.*"
    }
}
```

## Initialization (Laravel)

Initialize PostHog in the `boot` method of `app/Providers/AppServiceProvider.php` so it is ready before any requests. Use an environment variable for the API key (do not commit keys).

```php
<?php
namespace App\Providers;

use Illuminate\Support\ServiceProvider;
use PostHog\PostHog;

class AppServiceProvider extends ServiceProvider
{
    public function boot(): void
    {
        PostHog::init(
            config('services.posthog.key') ?? env('POSTHOG_API_KEY'),
            [
                'host' => 'https://us.i.posthog.com',
            ]
        );
    }
}
```

Use the same US Cloud host (`us.i.posthog.com`) as the frontend (e.g. `apps/web`) so all data stays in one project. You can find your project API key and instance address in [your PostHog project settings](https://us.posthog.com/project/settings). Optional: define `services.posthog.key` in `config/services.php` and read it here.

## Usage

Use the PostHog client anywhere by importing `use PostHog\PostHog;` and calling `PostHog::method_name`. Example: capture an event in a route.

```php
<?php
use Illuminate\Support\Facades\Route;
use PostHog\PostHog;

Route::get('/', function () {
    PostHog::capture([
        'distinctId' => 'distinct_id_of_your_user',
        'event' => 'route_called',
    ]);
    return view('welcome');
});
```

In real usage, pass the authenticated user’s ID (or session ID) as `distinctId`.

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
