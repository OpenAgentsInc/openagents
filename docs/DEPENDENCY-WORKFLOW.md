# Dependency Management Workflow

This guide covers the automated dependency audit and safe update workflow for OpenAgents.

## Overview

OpenAgents uses Bun for dependency management with automated tooling for:
- **Weekly vulnerability audits** (CI workflow)
- **Safe dependency updates** with automatic backups
- **One-command rollback** if updates cause issues

## Weekly Audit (Automated)

The CI workflow runs every Monday at 5am UTC and on-demand via workflow_dispatch.

**What it checks:**
- Production vulnerabilities via `npm audit`
- Available package updates via `npm-check-updates`

**Artifacts:**
- `.openagents/deps/audit-report.json` (uploaded to GitHub artifacts)

**Manual run:**
```bash
bun run deps:audit
```

## Safe Update Workflow

### Step 1: Review Current State

Check for vulnerabilities and available updates:
```bash
bun run deps:audit
```

This outputs:
- Vulnerability counts by severity (info, low, moderate, high, critical)
- List of upgradeable packages with version changes
- Saved report at `.openagents/deps/audit-report.json`

### Step 2: Preview Updates

See what would change without applying updates:
```bash
bun run deps:update --dry-run
```

For specific packages:
```bash
bun run deps:update --dry-run effect @effect/platform
```

### Step 3: Apply Updates

Update all dependencies with automatic backup:
```bash
bun run deps:update
```

**What happens:**
1. Lockfile backed up to `.openagents/deps/backups/bun.lockb.YYYY-MM-DD.backup`
2. Dependencies updated via `bun update`
3. On failure, automatic rollback to backup
4. Success message with backup location

Update specific packages:
```bash
bun run deps:update effect @effect/platform
```

### Step 4: Verify Changes

Run the test suite to ensure nothing broke:
```bash
bun test
```

Check type safety:
```bash
bun run typecheck
```

### Step 5: Commit or Rollback

**If tests pass:**
```bash
git add bun.lockb
git commit -m "deps: update dependencies $(date +%Y-%m-%d)"
git push
```

**If tests fail:**
```bash
# Rollback to the backup (path shown in update output)
bun run deps:update --rollback .openagents/deps/backups/bun.lockb.2025-12-05.backup

# Or find the latest backup
ls -lt .openagents/deps/backups/
bun run deps:update --rollback .openagents/deps/backups/bun.lockb.2025-12-05.backup
```

## CI Integration

### GitHub Actions Workflow

The weekly audit runs via `.github/workflows/deps-audit.yml`:
- Runs every Monday at 5am UTC
- Can be triggered manually via workflow_dispatch
- Uploads audit report as artifact

### Adding Update to CI (Optional)

To automate updates in CI, create `.github/workflows/deps-update.yml`:

```yaml
name: Dependency Update

on:
  schedule:
    - cron: "0 6 * * 1"  # Monday 6am UTC
  workflow_dispatch: {}

jobs:
  update:
    runs-on: ubuntu-latest
    steps:
      - name: Checkout
        uses: actions/checkout@v4

      - name: Setup Bun
        uses: oven-sh/setup-bun@v2
        with:
          bun-version: "1.3.0"

      - name: Install deps
        run: bun install --frozen-lockfile

      - name: Update dependencies
        run: bun run deps:update

      - name: Run tests
        run: bun test

      - name: Create PR
        if: success()
        uses: peter-evans/create-pull-request@v5
        with:
          commit-message: "deps: automated dependency update"
          branch: deps/automated-update
          title: "deps: Automated dependency update"
          body: |
            Automated dependency update from CI.

            Backup available in commit artifacts.
```

## CLI Reference

### deps:audit

Check for vulnerabilities and available updates.

```bash
bun run deps:audit [OPTIONS]

OPTIONS:
  --output, -o <file>   Output path (default: .openagents/deps/audit-report.json)
  --json                Output JSON to stdout
  --help, -h            Show help
```

### deps:update

Safely update dependencies with automatic backup.

```bash
bun run deps:update [OPTIONS] [PACKAGES...]

OPTIONS:
  --dry-run, -n         Preview updates without applying
  --backup, -b <path>   Custom backup path
  --rollback, -r <path> Rollback to a previous backup
  --help, -h            Show help

EXAMPLES:
  # Update all dependencies
  bun run deps:update

  # Preview updates
  bun run deps:update --dry-run

  # Update specific packages
  bun run deps:update effect @effect/platform

  # Rollback to backup
  bun run deps:update --rollback .openagents/deps/backups/bun.lockb.2025-12-05.backup
```

## Backup Management

Backups are stored in `.openagents/deps/backups/` with format:
```
bun.lockb.YYYY-MM-DD.backup
```

**List backups:**
```bash
ls -lt .openagents/deps/backups/
```

**Clean old backups** (keep last 10):
```bash
cd .openagents/deps/backups
ls -t | tail -n +11 | xargs rm -f
```

## Troubleshooting

### Update fails with "Command failed"

**Cause:** Dependency conflict or network issue

**Solution:**
1. Check the error message for specific package conflicts
2. Try updating problematic packages individually
3. Use `--dry-run` to preview without applying

### Rollback fails

**Cause:** Backup file not found or corrupted

**Solution:**
1. Check backup exists: `ls .openagents/deps/backups/`
2. Try git to restore: `git checkout bun.lockb`
3. Reinstall from package.json: `rm bun.lockb && bun install`

### Tests fail after update

**Cause:** Breaking changes in updated packages

**Solution:**
1. Rollback immediately: `bun run deps:update --rollback <backup-path>`
2. Check changelog for breaking changes
3. Update code to handle breaking changes
4. Apply updates incrementally by package

## Best Practices

1. **Always run tests** after updating dependencies
2. **Update incrementally** for major version bumps
3. **Keep backups** for at least 30 days
4. **Review audit reports** weekly
5. **Pin critical dependencies** if stability is more important than updates

## Security Considerations

- Production dependencies only in audit (via `--production` flag)
- Automated backups prevent data loss
- CI workflow uses frozen lockfile for reproducibility
- Audit reports stored in `.openagents/` for tracking
