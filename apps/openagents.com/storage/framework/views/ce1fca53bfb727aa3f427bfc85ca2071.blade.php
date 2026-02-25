# PHP

@php
/** @var \Laravel\Boost\Install\GuidelineAssist $assist */
@endphp
@if($assist->shouldEnforceStrictTypes())
- Always use strict typing at the head of a ___SINGLE_BACKTICK___.php___SINGLE_BACKTICK___ file: ___SINGLE_BACKTICK___declare(strict_types=1);___SINGLE_BACKTICK___.
@endif
- Always use curly braces for control structures, even for single-line bodies.

## Constructors
- Use PHP 8 constructor property promotion in ___SINGLE_BACKTICK_____construct()___SINGLE_BACKTICK___.
    - ___SINGLE_BACKTICK___public function __construct(public GitHub $github) { }___SINGLE_BACKTICK___
- Do not allow empty ___SINGLE_BACKTICK_____construct()___SINGLE_BACKTICK___ methods with zero parameters unless the constructor is private.

## Type Declarations
- Always use explicit return type declarations for methods and functions.
- Use appropriate PHP type hints for method parameters.

<!-- Explicit Return Types and Method Params -->
___SINGLE_BACKTICK______SINGLE_BACKTICK______SINGLE_BACKTICK___php
protected function isAccessible(User $user, ?string $path = null): bool
{
    ...
}
___SINGLE_BACKTICK______SINGLE_BACKTICK______SINGLE_BACKTICK___

## Enums
@if(empty($assist->enums()) || preg_match('/[A-Z]{3,8}/', $assist->enumContents()))
- Typically, keys in an Enum should be TitleCase. For example: ___SINGLE_BACKTICK___FavoritePerson___SINGLE_BACKTICK___, ___SINGLE_BACKTICK___BestLake___SINGLE_BACKTICK___, ___SINGLE_BACKTICK___Monthly___SINGLE_BACKTICK___.
@else
- That being said, keys in an Enum should follow existing application Enum conventions.
@endif

## Comments
- Prefer PHPDoc blocks over inline comments. Never use comments within the code itself unless the logic is exceptionally complex.

## PHPDoc Blocks
- Add useful array shape type definitions when appropriate.
