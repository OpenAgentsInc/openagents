# Upgrade Guide
## General Notes
## Upgrading from 3.x to 4.x

### Minimum PHP Version
PHP 8.3 is now the minimum required version.

### Minimum PestPHP Version
Pest v4 is now the minimum required version.

### Removed `toHaveProfanity` Expectation
The `toHaveProfanity` Expectation has been removed in `v4`. This will cause your test suite to fail if you do not remove
this Expectation. To use this package, you must now run `./vendor/bin/pest --profanity`. You may wish to set this up 
as a Composer script:

```json
"scripts": {
    "profanity": "pest --profanity"
},
```

You can then execute it by running `composer profanity`. You may also wish to set it up in your CI/CD scripts:

```bash
./vendor/bin/pest --profanity
```
