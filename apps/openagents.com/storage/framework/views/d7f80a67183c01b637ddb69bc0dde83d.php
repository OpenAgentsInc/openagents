---
name: wayfinder-development
description: "Activates whenever referencing backend routes in frontend components. Use when importing from @/actions or @/routes, calling Laravel routes from TypeScript, or working with Wayfinder route functions."
license: MIT
metadata:
  author: laravel
---
<?php
/** @var \Laravel\Boost\Install\GuidelineAssist $assist */
?>
# Wayfinder Development

## When to Apply

Activate whenever referencing backend routes in frontend components:
- Importing from ___SINGLE_BACKTICK___@/actions/___SINGLE_BACKTICK___ or ___SINGLE_BACKTICK___@/routes/___SINGLE_BACKTICK___
- Calling Laravel routes from TypeScript/JavaScript
- Creating links or navigation to backend endpoints

## Documentation

Use ___SINGLE_BACKTICK___search-docs___SINGLE_BACKTICK___ for detailed Wayfinder patterns and documentation.

## Quick Reference

### Generate Routes

Run after route changes if Vite plugin isn't installed:
___SINGLE_BACKTICK______SINGLE_BACKTICK______SINGLE_BACKTICK___bash
<?php echo e($assist->artisanCommand('wayfinder:generate --no-interaction')); ?>

___SINGLE_BACKTICK______SINGLE_BACKTICK______SINGLE_BACKTICK___
For form helpers, use ___SINGLE_BACKTICK___--with-form___SINGLE_BACKTICK___ flag:
___SINGLE_BACKTICK______SINGLE_BACKTICK______SINGLE_BACKTICK___bash
<?php echo e($assist->artisanCommand('wayfinder:generate --with-form --no-interaction')); ?>

___SINGLE_BACKTICK______SINGLE_BACKTICK______SINGLE_BACKTICK___
### Import Patterns

___BOOST_SNIPPET_0___

### Common Methods

___BOOST_SNIPPET_1___

<?php if($assist->roster->uses(\Laravel\Roster\Enums\Packages::INERTIA_LARAVEL) || $assist->roster->uses(\Laravel\Roster\Enums\Packages::INERTIA_REACT) || $assist->roster->uses(\Laravel\Roster\Enums\Packages::INERTIA_VUE) || $assist->roster->uses(\Laravel\Roster\Enums\Packages::INERTIA_SVELTE)): ?>
## Wayfinder + Inertia

<?php if($assist->inertia()->hasFormComponent()): ?>
Use Wayfinder with the ___SINGLE_BACKTICK___<Form>___SINGLE_BACKTICK___ component:
<?php if($assist->roster->uses(\Laravel\Roster\Enums\Packages::INERTIA_REACT)): ?>
___BOOST_SNIPPET_2___
<?php endif; ?>
<?php if($assist->roster->uses(\Laravel\Roster\Enums\Packages::INERTIA_VUE)): ?>
___BOOST_SNIPPET_3___
<?php endif; ?>
<?php if($assist->roster->uses(\Laravel\Roster\Enums\Packages::INERTIA_SVELTE)): ?>
___BOOST_SNIPPET_4___
<?php endif; ?>
<?php else: ?>
Use Wayfinder with ___SINGLE_BACKTICK___useForm___SINGLE_BACKTICK___:

___BOOST_SNIPPET_5___
<?php endif; ?>
<?php endif; ?>

## Verification

1. Run ___SINGLE_BACKTICK___<?php echo e($assist->artisanCommand('wayfinder:generate')); ?>___SINGLE_BACKTICK___ to regenerate routes if Vite plugin isn't installed
2. Check TypeScript imports resolve correctly
3. Verify route URLs match expected paths

## Common Pitfalls

- Using default imports instead of named imports (breaks tree-shaking)
- Forgetting to regenerate after route changes
- Not using type-safe parameter objects for route model binding
<?php /**PATH /Users/christopherdavid/code/openagents/apps/openagents.com/storage/framework/views/dec43f67b495665ddb8f57c78fef8eb7.blade.php ENDPATH**/ ?>