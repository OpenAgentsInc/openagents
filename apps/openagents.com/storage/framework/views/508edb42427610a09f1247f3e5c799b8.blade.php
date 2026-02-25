@php
/** @var \Laravel\Boost\Install\GuidelineAssist $assist */
@endphp
## Pest

- This project uses Pest for testing. Create tests: ___SINGLE_BACKTICK___{{ $assist->artisanCommand('make:test --pest {name}') }}___SINGLE_BACKTICK___.
- Run tests: ___SINGLE_BACKTICK___{{ $assist->artisanCommand('test --compact') }}___SINGLE_BACKTICK___ or filter: ___SINGLE_BACKTICK___{{ $assist->artisanCommand('test --compact --filter=testName') }}___SINGLE_BACKTICK___.
- Do NOT delete tests without approval.
- CRITICAL: ALWAYS use ___SINGLE_BACKTICK___search-docs___SINGLE_BACKTICK___ tool for version-specific Pest documentation and updated code examples.
- IMPORTANT: Activate ___SINGLE_BACKTICK___pest-testing___SINGLE_BACKTICK___ every time you're working with a Pest or testing-related task.
