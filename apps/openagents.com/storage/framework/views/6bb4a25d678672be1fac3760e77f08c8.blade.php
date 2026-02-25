@php
/** @var \Laravel\Boost\Install\GuidelineAssist $assist */
@endphp
# Laravel Wayfinder

Wayfinder generates TypeScript functions for Laravel routes. Import from ___SINGLE_BACKTICK___@/actions/___SINGLE_BACKTICK___ (controllers) or ___SINGLE_BACKTICK___@/routes/___SINGLE_BACKTICK___ (named routes).

- IMPORTANT: Activate ___SINGLE_BACKTICK___wayfinder-development___SINGLE_BACKTICK___ skill whenever referencing backend routes in frontend components.
- Invokable Controllers: ___SINGLE_BACKTICK___import StorePost from '@/actions/.../StorePostController'; StorePost()___SINGLE_BACKTICK___.
- Parameter Binding: Detects route keys (___SINGLE_BACKTICK___{post:slug}___SINGLE_BACKTICK___) — ___SINGLE_BACKTICK___show({ slug: "my-post" })___SINGLE_BACKTICK___.
- Query Merging: ___SINGLE_BACKTICK___show(1, { mergeQuery: { page: 2, sort: null } })___SINGLE_BACKTICK___ merges with current URL, ___SINGLE_BACKTICK___null___SINGLE_BACKTICK___ removes params.
@if($assist->roster->uses(\Laravel\Roster\Enums\Packages::INERTIA_LARAVEL) || $assist->roster->uses(\Laravel\Roster\Enums\Packages::INERTIA_REACT) || $assist->roster->uses(\Laravel\Roster\Enums\Packages::INERTIA_VUE) || $assist->roster->uses(\Laravel\Roster\Enums\Packages::INERTIA_SVELTE))
- Inertia: Use ___SINGLE_BACKTICK___.form()___SINGLE_BACKTICK___ with ___SINGLE_BACKTICK___<Form>___SINGLE_BACKTICK___ component or ___SINGLE_BACKTICK___form.submit(store())___SINGLE_BACKTICK___ with useForm.
@endif
