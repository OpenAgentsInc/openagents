---
name: integration-laravel
description: PostHog integration for Laravel applications
metadata:
  author: PostHog
  version: 1.5.2
---

# PostHog integration for Laravel

This skill helps you add PostHog analytics to Laravel applications.

## Workflow

Follow these steps in order to complete the integration:

1. `basic-integration-1.0-begin.md` - PostHog Setup - Begin ← **Start here**
2. `basic-integration-1.1-edit.md` - PostHog Setup - Edit
3. `basic-integration-1.2-revise.md` - PostHog Setup - Revise
4. `basic-integration-1.3-conclude.md` - PostHog Setup - Conclusion

## Reference files

- `EXAMPLE.md` - Laravel example project code
- `laravel.md` - Laravel - docs
- `identify-users.md` - Identify users - docs
- `basic-integration-1.0-begin.md` - PostHog setup - begin
- `basic-integration-1.1-edit.md` - PostHog setup - edit
- `basic-integration-1.2-revise.md` - PostHog setup - revise
- `basic-integration-1.3-conclude.md` - PostHog setup - conclusion

The example project shows the target implementation pattern. Consult the documentation for API details.

## Key principles

- **Environment variables**: Always use environment variables for PostHog keys. Never hardcode them.
- **Minimal changes**: Add PostHog code alongside existing integrations. Don't replace or restructure existing code.
- **Match the example**: Your implementation should follow the example project's patterns as closely as possible.

## Framework guidelines

- Create a dedicated PostHogService class in app/Services/ - do NOT scatter PostHog::capture calls throughout controllers
- Register PostHog configuration in config/posthog.php using env() for all settings (api_key, host, disabled)
- Do NOT use Laravel's event system or observers for analytics - call capture explicitly where actions occur
- Remember that source code is available in the vendor directory after composer install
- posthog/posthog-php is the PHP SDK package name
- Check composer.json for existing dependencies and autoload configuration before adding new files
- The PHP SDK uses static methods (PostHog::capture, PostHog::identify) - initialize once with PostHog::init()
- PHP SDK methods take associative arrays with 'distinctId', 'event', 'properties' keys - not positional arguments

## Identifying users

Identify users during login and signup events. Refer to the example code and documentation for the correct identify pattern for this framework. If both frontend and backend code exist, pass the client-side session and distinct ID using `X-POSTHOG-DISTINCT-ID` and `X-POSTHOG-SESSION-ID` headers to maintain correlation.

## Error tracking

Add PostHog error tracking to relevant files, particularly around critical user flows and API boundaries.
