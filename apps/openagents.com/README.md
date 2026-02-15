# openagents.com (Laravel)

**Incoming core web app** for OpenAgents: Laravel 12 + Inertia + React (TypeScript), WorkOS auth, Laravel Boost. This app is the target replacement for the current Effuse/Cloudflare/Convex web stack.

- **Plan:** `docs/plans/active/laravel-rebuild.md` (from repo root: `../../docs/plans/active/laravel-rebuild.md`)
- **Verification:** `composer test`, `composer lint`, `npm run build`

## Local development

```bash
composer run dev
```

Or: `php artisan serve` in one terminal and `npm run dev` in another.

## Stack

- Laravel 12, Inertia, React (TS), Vite
- WorkOS auth
- Laravel Wayfinder (typed routes/actions)
- Pest for PHP tests
