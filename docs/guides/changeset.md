# Changeset Workflow Guide

## Overview

This repository uses [Changesets](https://github.com/changesets/changesets) for version management and release automation. Changesets provide a workflow for managing versions, changelogs, and publishing in monorepos.

## Current Configuration

### Development Mode (Current State)
- **Versioning**: Enabled - Creates release PRs with version bumps
- **Publishing**: Disabled - No automatic npm publishing
- **Reason**: Packages are in development (version 0.0.0)

### Files Involved
```
/.changeset/config.json          # Changeset configuration
/.github/workflows/release.yml   # Release automation workflow
/packages/*/package.json         # Package versions and metadata
```

## How Changesets Work

### 1. Creating a Changeset

When you make changes that should trigger a version bump:

```bash
# Create a new changeset
pnpm changeset

# Follow prompts:
# - Select packages that changed
# - Choose bump type (patch/minor/major)
# - Write change summary
```

This creates a markdown file in `/.changeset/` describing the change.

### 2. Changeset File Format

Example changeset file (`/.changeset/blue-lions-sing.md`):
```markdown
---
"@openagentsinc/domain": minor
"@openagentsinc/server": minor  
"@openagentsinc/cli": patch
---

Add new Todo completion endpoint with improved validation
```

### 3. Version Release Process

When changesets exist and code is merged to main:

1. **Automatic PR Creation**: GitHub Action creates a "Version Packages" PR
2. **Consolidated Changes**: PR includes all pending changesets
3. **Version Bumps**: Updates package.json versions according to changesets
4. **Changelog Generation**: Creates/updates CHANGELOG.md files
5. **Changeset Cleanup**: Removes consumed changeset files

### 4. Manual Version Commands

```bash
# Preview version changes
pnpm changeset status

# Apply version changes locally (for testing)
pnpm changeset-version

# Check for packages that need publishing
pnpm changeset status --verbose
```

## Current Workflow (Publishing Disabled)

### What Happens Now
1. Create changesets for your changes
2. Merge to main triggers release workflow
3. GitHub Action creates "Version Packages" PR
4. PR contains version bumps and changelog updates
5. **No publishing occurs** - versions tracked but not released

### Benefits of This Approach
- Track version changes during development
- Maintain changelog discipline
- Prepare for eventual publishing
- Review version bumps before release

## Re-enabling Publishing (Future)

When ready to publish packages to npm, follow these steps:

### 1. Configure NPM Authentication

Add NPM_TOKEN secret to GitHub repository:
```bash
# Generate npm token (classic token with publish scope)
npm login
npm token create --access=public

# Add to GitHub Secrets:
# Settings → Secrets → Actions → New repository secret
# Name: NPM_TOKEN
# Value: npm_token_here
```

### 2. Update Release Workflow

Modify `/.github/workflows/release.yml`:

```yaml
- name: Create Release Pull Request or Publish
  uses: changesets/action@v1
  with:
    version: pnpm changeset-version
    publish: pnpm changeset-publish  # Re-add this line
  env:
    GITHUB_TOKEN: ${{ secrets.GITHUB_TOKEN }}
    NPM_TOKEN: ${{ secrets.NPM_TOKEN }}  # Re-add this line
```

### 3. Verify Package Configuration

Ensure packages are properly configured for publishing:

```json
{
  "name": "@openagentsinc/domain",
  "version": "1.0.0",  // Change from 0.0.0
  "license": "CC0-1.0",
  "repository": {
    "type": "git", 
    "url": "https://github.com/OpenAgentsInc/openagents"
  },
  "publishConfig": {
    "access": "public"  // Add if publishing public packages
  }
}
```

### 4. Test Publishing Process

Before enabling automatic publishing:

```bash
# Build packages locally
pnpm build

# Dry run publish (doesn't actually publish)
pnpm changeset publish --dry-run

# Manual publish of single package (for testing)
cd packages/domain
npm publish --dry-run
```

## Changeset Configuration

Current configuration (`/.changeset/config.json`):

```json
{
  "$schema": "https://unpkg.com/@changesets/config@3.0.3/schema.json",
  "changelog": "@changesets/cli/changelog",
  "commit": false,
  "fixed": [],
  "linked": [],
  "access": "restricted",
  "baseBranch": "main",
  "updateInternalDependencies": "patch",
  "ignore": []
}
```

### Key Configuration Options

- **`access`**: "restricted" (private) vs "public" for npm publishing
- **`updateInternalDependencies`**: How to bump internal package dependencies
- **`linked`**: Groups packages to version together
- **`fixed`**: Forces packages to same version
- **`ignore`**: Packages to exclude from changeset process

## Best Practices

### When to Create Changesets

Create changesets for:
- ✅ New features (minor bump)
- ✅ Bug fixes (patch bump)  
- ✅ Breaking changes (major bump)
- ✅ Documentation that affects usage
- ❌ Internal refactoring without behavior change
- ❌ Test-only changes
- ❌ Build/CI configuration updates

### Changeset Writing Guidelines

```markdown
---
"@openagentsinc/domain": minor
---

Add TodoFilters API for advanced todo querying

This adds new filtering capabilities to the TodosApi including:
- Filter by completion status  
- Date range filtering
- Text search functionality
```

**Good changeset descriptions:**
- Explain user-facing impact
- Mention breaking changes clearly
- Reference related issues/PRs
- Use consistent tense (present/past)

### Version Bump Guidelines

- **Patch (0.0.X)**: Bug fixes, small improvements
- **Minor (0.X.0)**: New features, non-breaking additions  
- **Major (X.0.0)**: Breaking changes, API changes

## Troubleshooting

### Common Issues

**"No changesets found" in workflow**
- Normal when no changes need versioning
- Workflow still validates packages aren't already published

**Changeset doesn't include all packages**
- Re-run `pnpm changeset` to add missing packages
- Use `pnpm changeset status` to see current state

**Version conflicts**
- Delete `.changeset/*.md` files causing conflicts
- Re-create changesets with correct configuration

### Recovery Commands

```bash
# Reset changeset state
rm .changeset/*.md

# Force specific version bump
pnpm changeset add --empty  # Creates empty changeset for editing

# Skip changeset for commit
git commit -m "chore: internal refactor [skip changeset]"
```

## Migration Notes

This repository was configured with:
- All packages starting at version 0.0.0
- Development-focused workflow
- Publishing disabled for safety
- Standard changelog generation enabled

When transitioning to published packages, coordinate the first release carefully to establish version baselines.