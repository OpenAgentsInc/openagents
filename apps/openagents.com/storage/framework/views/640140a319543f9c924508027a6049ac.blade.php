# Inertia

- Inertia creates fully client-side rendered SPAs without modern SPA complexity, leveraging existing server-side patterns.
- Components live in ___SINGLE_BACKTICK___{{ $assist->inertia()->pagesDirectory() }}___SINGLE_BACKTICK___ (unless specified in ___SINGLE_BACKTICK___vite.config.js___SINGLE_BACKTICK___). Use ___SINGLE_BACKTICK___Inertia::render()___SINGLE_BACKTICK___ for server-side routing instead of Blade views.
- ALWAYS use ___SINGLE_BACKTICK___search-docs___SINGLE_BACKTICK___ tool for version-specific Inertia documentation and updated code examples.
@if($assist->hasPackage(\Laravel\Roster\Enums\Packages::INERTIA_REACT))
- IMPORTANT: Activate ___SINGLE_BACKTICK___inertia-react-development___SINGLE_BACKTICK___ when working with Inertia client-side patterns.
@elseif($assist->hasPackage(\Laravel\Roster\Enums\Packages::INERTIA_VUE))
- IMPORTANT: Activate ___SINGLE_BACKTICK___inertia-vue-development___SINGLE_BACKTICK___ when working with Inertia Vue client-side patterns.
@elseif($assist->hasPackage(\Laravel\Roster\Enums\Packages::INERTIA_SVELTE))
- IMPORTANT: Activate ___SINGLE_BACKTICK___inertia-svelte-development___SINGLE_BACKTICK___ when working with Inertia Svelte client-side patterns.
@endif
