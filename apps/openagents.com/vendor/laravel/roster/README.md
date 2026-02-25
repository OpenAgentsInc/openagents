# Laravel Roster

<p align="center">
<a href="https://github.com/laravel/roster/actions"><img src="https://github.com/laravel/roster/workflows/tests/badge.svg" alt="Build Status"></a>
<a href="https://packagist.org/packages/laravel/roster"><img src="https://img.shields.io/packagist/dt/laravel/roster" alt="Total Downloads"></a>
<a href="https://packagist.org/packages/laravel/roster"><img src="https://img.shields.io/packagist/v/laravel/roster" alt="Latest Stable Version"></a>
<a href="https://packagist.org/packages/laravel/roster"><img src="https://img.shields.io/packagist/l/laravel/roster" alt="License"></a>
</p>

## Introduction

Laravel Roster detects which Laravel ecosystem packages are in use within a project, and gives you an easy to use API to work with that data.


## Installation
To get started, install Roster via Composer:

```bash
composer require laravel/roster --dev
```

## Usage

**Scan a directory**

Get a roster of installed packages by scanning a directory:

```php
use Laravel\Roster\Roster;

$roster = Roster::scan($directory);
```

**Query the roster**
```php
use Laravel\Roster\Packages;

// Get all packages
$roster->packages();

// Get only packages that will be used in production
$roster->packages()->production();

// Packages that are only used for dev
$roster->packages()->dev();

// Check if a package is in use
$roster->uses(Packages::INERTIA);

// Check if a particular version of a package is in use
$roster->usesVersion(Packages::INERTIA, '2.0.0', '>=');

// Detect which JavaScript package manager is in use
$packageManager = $roster->nodePackageManager();
```

## Contributing

Thank you for considering contributing to Roster! The contribution guide can be found in
the [Laravel documentation](https://laravel.com/docs/contributions).

## Code of Conduct

In order to ensure that the Laravel community is welcoming to all, please review and abide by
the [Code of Conduct](https://laravel.com/docs/contributions#code-of-conduct).

## Security Vulnerabilities

Please review [our security policy](https://github.com/laravel/roster/security/policy) on how to report security
vulnerabilities.

## License

Laravel Roster is open-sourced software licensed under the [MIT license](LICENSE.md).
