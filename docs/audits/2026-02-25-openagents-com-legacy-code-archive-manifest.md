# Openagents.com Legacy Code Archive Manifest

Status: archived
Date: 2026-02-25
Issue: `#2212` (`OA-AUDIT Phase 0: Delete all legacy PHP/TS/non-retained code surfaces`)

## Summary

Tracked legacy code under `apps/openagents.com/` (outside `service/`) was archived to backroom and removed from this repository.

Archive destination:

- `/Users/christopherdavid/code/backroom/openagents-code-archive/2026-02-25-oa-audit-issue-2212-openagents-com-legacy/`

Tracked files archived and removed: `600`

## Major lanes removed

1. `apps/openagents.com/app/` (`180` files)
2. `apps/openagents.com/resources/` (`140` files)
3. `apps/openagents.com/tests/` (`72` files)
4. `apps/openagents.com/docs/` (`61` files)
5. `apps/openagents.com/database/` (`28` files)
6. `apps/openagents.com/config/` (`20` files)
7. `apps/openagents.com/scripts/` (`16` files)
8. `apps/openagents.com/deploy/` (`13` files)
9. Additional legacy root/build/config assets (composer/npm/phpunit/vite/artisan/etc).

## File type highlights removed

1. PHP: `311` files
2. TS/TSX: `138` files
3. Shell scripts: `21` files
4. Markdown docs: `66` files

## Retained under `apps/openagents.com/`

1. `apps/openagents.com/service/` (canonical Rust control service surface).
2. `apps/openagents.com/.github/workflows/*` were removed in OA-AUDIT `#2213` to satisfy `INV-12`.
