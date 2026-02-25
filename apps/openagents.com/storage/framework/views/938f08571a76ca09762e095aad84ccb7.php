---
name: inertia-react-development
description: "Develops Inertia.js v2 React client-side applications. Activates when creating React pages, forms, or navigation; using <Link>, <Form>, useForm, or router; working with deferred props, prefetching, or polling; or when user mentions React with Inertia, React pages, React forms, or React navigation."
license: MIT
metadata:
  author: laravel
---
<?php
/** @var \Laravel\Boost\Install\GuidelineAssist $assist */
?>
# Inertia React Development

## When to Apply

Activate this skill when:

- Creating or modifying React page components for Inertia
- Working with forms in React (using ___SINGLE_BACKTICK___<Form>___SINGLE_BACKTICK___ or ___SINGLE_BACKTICK___useForm___SINGLE_BACKTICK___)
- Implementing client-side navigation with ___SINGLE_BACKTICK___<Link>___SINGLE_BACKTICK___ or ___SINGLE_BACKTICK___router___SINGLE_BACKTICK___
- Using v2 features: deferred props, prefetching, or polling
- Building React-specific features with the Inertia protocol

## Documentation

Use ___SINGLE_BACKTICK___search-docs___SINGLE_BACKTICK___ for detailed Inertia v2 React patterns and documentation.

## Basic Usage

### Page Components Location

React page components should be placed in the ___SINGLE_BACKTICK___<?php echo e($assist->inertia()->pagesDirectory()); ?>___SINGLE_BACKTICK___ directory.

### Page Component Structure

___BOOST_SNIPPET_0___

## Client-Side Navigation

### Basic Link Component

Use ___SINGLE_BACKTICK___<Link>___SINGLE_BACKTICK___ for client-side navigation instead of traditional ___SINGLE_BACKTICK___<a>___SINGLE_BACKTICK___ tags:

___BOOST_SNIPPET_1___

### Link with Method

___BOOST_SNIPPET_2___

### Prefetching

Prefetch pages to improve perceived performance:

___BOOST_SNIPPET_3___

### Programmatic Navigation

___BOOST_SNIPPET_4___

## Form Handling

<?php if($assist->inertia()->hasFormComponent()): ?>
### Form Component (Recommended)

The recommended way to build forms is with the ___SINGLE_BACKTICK___<Form>___SINGLE_BACKTICK___ component:

___BOOST_SNIPPET_5___

### Form Component With All Props

___BOOST_SNIPPET_6___

<?php if($assist->inertia()->hasFormComponentResets()): ?>
### Form Component Reset Props

The ___SINGLE_BACKTICK___<Form>___SINGLE_BACKTICK___ component supports automatic resetting:

- ___SINGLE_BACKTICK___resetOnError___SINGLE_BACKTICK___ - Reset form data when the request fails
- ___SINGLE_BACKTICK___resetOnSuccess___SINGLE_BACKTICK___ - Reset form data when the request succeeds
- ___SINGLE_BACKTICK___setDefaultsOnSuccess___SINGLE_BACKTICK___ - Update default values on success

Use the ___SINGLE_BACKTICK___search-docs___SINGLE_BACKTICK___ tool with a query of ___SINGLE_BACKTICK___form component resetting___SINGLE_BACKTICK___ for detailed guidance.

___BOOST_SNIPPET_7___
<?php else: ?>
Note: This version of Inertia does not support ___SINGLE_BACKTICK___resetOnError___SINGLE_BACKTICK___, ___SINGLE_BACKTICK___resetOnSuccess___SINGLE_BACKTICK___, or ___SINGLE_BACKTICK___setDefaultsOnSuccess___SINGLE_BACKTICK___ on the ___SINGLE_BACKTICK___<Form>___SINGLE_BACKTICK___ component. Using these props will cause errors. Upgrade to Inertia v2.2.0+ to use these features.
<?php endif; ?>

Forms can also be built using the ___SINGLE_BACKTICK___useForm___SINGLE_BACKTICK___ helper for more programmatic control. Use the ___SINGLE_BACKTICK___search-docs___SINGLE_BACKTICK___ tool with a query of ___SINGLE_BACKTICK___useForm helper___SINGLE_BACKTICK___ for guidance.

<?php endif; ?>

### ___SINGLE_BACKTICK___useForm___SINGLE_BACKTICK___ Hook

<?php if($assist->inertia()->hasFormComponent() === false): ?>
For Inertia v2.0.x: Build forms using the ___SINGLE_BACKTICK___useForm___SINGLE_BACKTICK___ helper as the ___SINGLE_BACKTICK___<Form>___SINGLE_BACKTICK___ component is not available until v2.1.0+.
<?php else: ?>
For more programmatic control or to follow existing conventions, use the ___SINGLE_BACKTICK___useForm___SINGLE_BACKTICK___ hook:
<?php endif; ?>

___BOOST_SNIPPET_8___

## Inertia v2 Features

### Deferred Props

Use deferred props to load data after initial page render:

___BOOST_SNIPPET_9___

### Polling

Use the ___SINGLE_BACKTICK___usePoll___SINGLE_BACKTICK___ hook to automatically refresh data at intervals. It handles cleanup on unmount and throttles polling when the tab is inactive.

___BOOST_SNIPPET_10___

___BOOST_SNIPPET_11___

- ___SINGLE_BACKTICK___autoStart___SINGLE_BACKTICK___ (default ___SINGLE_BACKTICK___true___SINGLE_BACKTICK___) — set to ___SINGLE_BACKTICK___false___SINGLE_BACKTICK___ to start polling manually via the returned ___SINGLE_BACKTICK___start()___SINGLE_BACKTICK___ function
- ___SINGLE_BACKTICK___keepAlive___SINGLE_BACKTICK___ (default ___SINGLE_BACKTICK___false___SINGLE_BACKTICK___) — set to ___SINGLE_BACKTICK___true___SINGLE_BACKTICK___ to prevent throttling when the browser tab is inactive

### WhenVisible (Infinite Scroll)

Load more data when user scrolls to a specific element:

___BOOST_SNIPPET_12___

## Common Pitfalls

- Using traditional ___SINGLE_BACKTICK___<a>___SINGLE_BACKTICK___ links instead of Inertia's ___SINGLE_BACKTICK___<Link>___SINGLE_BACKTICK___ component (breaks SPA behavior)
- Forgetting to add loading states (skeleton screens) when using deferred props
- Not handling the ___SINGLE_BACKTICK___undefined___SINGLE_BACKTICK___ state of deferred props before data loads
- Using ___SINGLE_BACKTICK___<form>___SINGLE_BACKTICK___ without preventing default submission (use ___SINGLE_BACKTICK___<Form>___SINGLE_BACKTICK___ component or ___SINGLE_BACKTICK___e.preventDefault()___SINGLE_BACKTICK___)
- Forgetting to check if ___SINGLE_BACKTICK___<Form>___SINGLE_BACKTICK___ component is available in your Inertia version
<?php /**PATH /Users/christopherdavid/code/openagents/apps/openagents.com/storage/framework/views/c960ddfe97735fb98905108b3d52fbc3.blade.php ENDPATH**/ ?>