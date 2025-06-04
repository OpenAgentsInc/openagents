# Disable NPM Publishing - December 4, 2025, 10:20 AM

## Problem

After successfully fixing all CI issues and merging PR #901 to main, the release workflow triggered and attempted to publish all packages to npm. This failed because:

1. No NPM_TOKEN was configured (authentication failure)
2. Packages aren't ready for public release yet
3. Version 0.0.0 indicates development/unreleased state

## Error Details

```
🦋  error an error occurred while publishing @openagentsinc/server: ENEEDAUTH This command requires you to be logged in to https://registry.npmjs.org/ 
🦋  error You need to authorize this machine using `npm adduser`
🦋  error packages failed to publish:
🦋  @openagentsinc/cli@0.0.0
🦋  @openagentsinc/domain@0.0.0
🦋  @openagentsinc/server@0.0.0
```

## Root Cause

The release workflow (`/.github/workflows/release.yml`) was configured to automatically publish packages when code is merged to main branch. This is standard for mature packages but premature for development repositories.

## Solution Applied

Modified the release workflow to only handle versioning without publishing:

### Before (Problematic)
```yaml
- name: Create Release Pull Request or Publish
  uses: changesets/action@v1
  with:
    version: pnpm changeset-version
    publish: pnpm changeset-publish
  env:
    GITHUB_TOKEN: ${{ secrets.GITHUB_TOKEN }}
    NPM_TOKEN: ${{ secrets.NPM_TOKEN }}
```

### After (Safe)
```yaml
- name: Create Release Pull Request
  uses: changesets/action@v1
  with:
    version: pnpm changeset-version
  env:
    GITHUB_TOKEN: ${{ secrets.GITHUB_TOKEN }}
```

## Changes Made

1. **Removed `publish` parameter** - No longer attempts to publish to npm
2. **Removed `NPM_TOKEN` requirement** - No authentication needed for version-only workflow
3. **Updated workflow name** - Clarifies it only creates release PRs, doesn't publish

## Current Behavior

- ✅ Merging to main will create changeset version PRs when changesets exist
- ✅ No automatic npm publishing 
- ✅ Version bumps tracked but not released
- ✅ Safe for development work

## When Ready to Enable Publishing

See `docs/changeset.md` for full instructions on re-enabling publishing when packages are ready for release.

## Files Modified

- `/.github/workflows/release.yml` - Disabled publish step

## Time Investment

**Duration**: 5 minutes
**Complexity**: Simple configuration change
**Impact**: Prevents accidental publishing

## Verification

The workflow will now safely handle version management without attempting npm publication until explicitly re-enabled.