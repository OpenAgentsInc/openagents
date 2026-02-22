# Legacy PHP/TypeScript Implementation Archive

Date: 2026-02-22  
Status: archived legacy reference (non-serving)

Legacy PHP/TypeScript implementation lanes for `apps/openagents.com` are retained only for historical reference and migration auditability.

## Archived Legacy Directories

- `apps/openagents.com/app/`
- `apps/openagents.com/bootstrap/`
- `apps/openagents.com/config/`
- `apps/openagents.com/database/`
- `apps/openagents.com/resources/`
- `apps/openagents.com/routes/`
- `apps/openagents.com/tests/`
- `apps/openagents.com/scripts/archived-laravel/`
- `apps/openagents.com/public/index.php`
- `apps/openagents.com/vite.config.ts`

## Active Product Paths (Rust-only)

- `apps/openagents.com/service/`
- `apps/openagents.com/web-shell/`
- `apps/openagents.com/scripts/` (excluding `archived-laravel/`)

## Enforcement

Rust-only terminal gate command:

```bash
./apps/openagents.com/scripts/verify-rust-only-terminal-gate.sh
```

Harness + artifact command:

```bash
./apps/openagents.com/scripts/run-rust-only-terminal-gate-harness.sh
```

The terminal gate fails if:
1. Any `.php`/`.ts`/`.tsx` appears inside active product paths.
2. Active scripts reference legacy runtime commands (`php artisan`, composer/npm dev lanes).
3. PHP/TypeScript files exist outside the archived legacy directories listed above.
4. Laravel serving retirement verification regresses.
