# AGENTS.md — Coax Web Application Guide

This application follows the canonical **Coax Web** skeleton (`coax/web/SKELETON.md`).
Prefer machine-readable introspection from `coax/web/cli.sh` over grepping files.

## One-Command App Introspection

```bash
coax/web/cli.sh doctor             # Verify toolchain, policies, and environment
coax/web/cli.sh routes --json      # Declarative API/web routes & auth rules
coax/web/cli.sh schema --json      # Parsed Postgres tables & columns from schema.sql
coax/web/cli.sh policies --json    # Every #authority policy + #test/#forall counts
coax/web/cli.sh config --schema    # Typed config keys, defaults & prod guards (zero secrets)
```

## Day-to-Day Workflows

```bash
coax/web/cli.sh setup              # Check prerequisites & prepare local .env
coax/web/cli.sh dev                # Local dev server with Wasm/WGSL hot-swap
coax/web/cli.sh test               # Run #test + #forall checks
coax/web/cli.sh migrate            # Apply Postgres migrations (Prohibitable in prod)
coax/web/cli.sh work               # Run Postgres queue worker (oa-api --work)
```

## Scaffolding New Components

```bash
coax/web/cli.sh make:policy <name>      # New #authority *-policy.coax (mutation-checked)
coax/web/cli.sh make:wire <name>        # New #authority *-wire.coax codec
coax/web/cli.sh make:migration <name>   # New immutable NNNN_<name>.sql migration
coax/web/cli.sh make:page <name>        # New Coax Web page folder (entry.coax + index.md)
```

## Non-Negotiable Invariants

1. **Zero npm / zero bundler / zero JS framework** on the browser host.
2. **All domain rules live in `#authority` `*-policy.coax`** with `coax test` + `coax mutate` passing at **0 surviving mutants**.
3. **Production safety (`Prohibitable`):** `migrate --fresh`, `db:wipe`, and `seed` are refused whenever `APP_ENV=production` or `OA_ENV=production`.
4. **Never log secrets** (tokens, cookies, emails, or keys) in structured `LogContext` output.
