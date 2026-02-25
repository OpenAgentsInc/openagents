---
name: pest-testing
description: "Tests applications using the Pest 4 PHP framework. Activates when writing tests, creating unit or feature tests, adding assertions, testing Livewire components, browser testing, debugging test failures, working with datasets or mocking; or when the user mentions test, spec, TDD, expects, assertion, coverage, or needs to verify functionality works."
license: MIT
metadata:
  author: laravel
---
@php
/** @var \Laravel\Boost\Install\GuidelineAssist $assist */
@endphp
# Pest Testing 4

## When to Apply

Activate this skill when:

- Creating new tests (unit, feature, or browser)
- Modifying existing tests
- Debugging test failures
- Working with browser testing or smoke testing
- Writing architecture tests or visual regression tests

## Documentation

Use ___SINGLE_BACKTICK___search-docs___SINGLE_BACKTICK___ for detailed Pest 4 patterns and documentation.

## Basic Usage

### Creating Tests

All tests must be written using Pest. Use ___SINGLE_BACKTICK___{{ $assist->artisanCommand('make:test --pest {name}') }}___SINGLE_BACKTICK___.

### Test Organization

- Unit/Feature tests: ___SINGLE_BACKTICK___tests/Feature___SINGLE_BACKTICK___ and ___SINGLE_BACKTICK___tests/Unit___SINGLE_BACKTICK___ directories.
- Browser tests: ___SINGLE_BACKTICK___tests/Browser/___SINGLE_BACKTICK___ directory.
- Do NOT remove tests without approval - these are core application code.

### Basic Test Structure

___BOOST_SNIPPET_0___

### Running Tests

- Run minimal tests with filter before finalizing: ___SINGLE_BACKTICK___{{ $assist->artisanCommand('test --compact --filter=testName') }}___SINGLE_BACKTICK___.
- Run all tests: ___SINGLE_BACKTICK___{{ $assist->artisanCommand('test --compact') }}___SINGLE_BACKTICK___.
- Run file: ___SINGLE_BACKTICK___{{ $assist->artisanCommand('test --compact tests/Feature/ExampleTest.php') }}___SINGLE_BACKTICK___.

## Assertions

Use specific assertions (___SINGLE_BACKTICK___assertSuccessful()___SINGLE_BACKTICK___, ___SINGLE_BACKTICK___assertNotFound()___SINGLE_BACKTICK___) instead of ___SINGLE_BACKTICK___assertStatus()___SINGLE_BACKTICK___:

___BOOST_SNIPPET_1___

| Use | Instead of |
|-----|------------|
| ___SINGLE_BACKTICK___assertSuccessful()___SINGLE_BACKTICK___ | ___SINGLE_BACKTICK___assertStatus(200)___SINGLE_BACKTICK___ |
| ___SINGLE_BACKTICK___assertNotFound()___SINGLE_BACKTICK___ | ___SINGLE_BACKTICK___assertStatus(404)___SINGLE_BACKTICK___ |
| ___SINGLE_BACKTICK___assertForbidden()___SINGLE_BACKTICK___ | ___SINGLE_BACKTICK___assertStatus(403)___SINGLE_BACKTICK___ |

## Mocking

Import mock function before use: ___SINGLE_BACKTICK___use function Pest\Laravel\mock;___SINGLE_BACKTICK___

## Datasets

Use datasets for repetitive tests (validation rules, etc.):

___BOOST_SNIPPET_2___

## Pest 4 Features

| Feature | Purpose |
|---------|---------|
| Browser Testing | Full integration tests in real browsers |
| Smoke Testing | Validate multiple pages quickly |
| Visual Regression | Compare screenshots for visual changes |
| Test Sharding | Parallel CI runs |
| Architecture Testing | Enforce code conventions |

### Browser Test Example

Browser tests run in real browsers for full integration testing:

- Browser tests live in ___SINGLE_BACKTICK___tests/Browser/___SINGLE_BACKTICK___.
- Use Laravel features like ___SINGLE_BACKTICK___Event::fake()___SINGLE_BACKTICK___, ___SINGLE_BACKTICK___assertAuthenticated()___SINGLE_BACKTICK___, and model factories.
- Use ___SINGLE_BACKTICK___RefreshDatabase___SINGLE_BACKTICK___ for clean state per test.
- Interact with page: click, type, scroll, select, submit, drag-and-drop, touch gestures.
- Test on multiple browsers (Chrome, Firefox, Safari) if requested.
- Test on different devices/viewports (iPhone 14 Pro, tablets) if requested.
- Switch color schemes (light/dark mode) when appropriate.
- Take screenshots or pause tests for debugging.

___BOOST_SNIPPET_3___

### Smoke Testing

Quickly validate multiple pages have no JavaScript errors:

___BOOST_SNIPPET_4___

### Visual Regression Testing

Capture and compare screenshots to detect visual changes.

### Test Sharding

Split tests across parallel processes for faster CI runs.

### Architecture Testing

Pest 4 includes architecture testing (from Pest 3):

___BOOST_SNIPPET_5___

## Common Pitfalls

- Not importing ___SINGLE_BACKTICK___use function Pest\Laravel\mock;___SINGLE_BACKTICK___ before using mock
- Using ___SINGLE_BACKTICK___assertStatus(200)___SINGLE_BACKTICK___ instead of ___SINGLE_BACKTICK___assertSuccessful()___SINGLE_BACKTICK___
- Forgetting datasets for repetitive validation tests
- Deleting tests without approval
- Forgetting ___SINGLE_BACKTICK___assertNoJavaScriptErrors()___SINGLE_BACKTICK___ in browser tests
