# PostHog PHP

Please see the main [PostHog docs](https://posthog.com/docs).

Specifically, the [PHP integration](https://posthog.com/docs/integrations/php-integration) details.

## Features

- ✅ Event capture and user identification
- ✅ Feature flag local evaluation
- ✅ **Feature flag dependencies** (new!) - Create conditional flags based on other flags
- ✅ Multivariate flags and payloads
- ✅ Group analytics
- ✅ Comprehensive test coverage

## Quick Start

1. Copy `.env.example` to `.env` and add your PostHog credentials
2. Run `php example.php` to see interactive examples of all features

## Questions?

### [Join our Slack community.](https://join.slack.com/t/posthogusers/shared_invite/enQtOTY0MzU5NjAwMDY3LTc2MWQ0OTZlNjhkODk3ZDI3NDVjMDE1YjgxY2I4ZjI4MzJhZmVmNjJkN2NmMGJmMzc2N2U3Yjc3ZjI5NGFlZDQ)

## Contributing

1. [Download PHP](https://www.php.net/manual/en/install.php) and [Composer](https://getcomposer.org/download/)
2. `php composer.phar update` to install dependencies
3. `bin/test` to run tests (this script calls `./vendor/bin/phpunit --verbose test`)
