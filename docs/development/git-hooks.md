# Git Hooks

This repository uses Git hooks to enforce code quality and policy compliance.

## Pre-Commit Hook

Location: `.git/hooks/pre-commit`

### Purpose

Enforces directive d-012 (No Stubs - Production-Ready Code Only) by preventing stub code from being committed.

### What It Checks

The pre-commit hook scans all staged Rust files for forbidden stub patterns:

- ❌ `todo!()` macros
- ❌ `unimplemented!()` macros
- ❌ `panic!("not implemented")` patterns

### When It Runs

Automatically runs before every commit. If violations are found, the commit is blocked.

### Example Output

```
🔍 Checking for stub patterns (d-012)...
Scanning staged Rust files...
❌ Found todo!() macro in crates/example/src/lib.rs
+ // todo!()

━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
COMMIT BLOCKED: Stub code detected (directive d-012)
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

Directive d-012: No Stubs - Production-Ready Code Only

All code must be production-ready or REMOVED. Per d-012:

  ❌ FORBIDDEN:
     • todo!() macros
     • unimplemented!() macros
     • panic!("not implemented") patterns

  ✅ ALTERNATIVES:
     • Implement the functionality NOW
     • Return an explicit error with clear message
     • Feature-gate with #[cfg(feature = "experimental")]
     • Comment out the code until it's ready
```

### Bypassing (Not Recommended)

In rare cases where you need to bypass the hook:

```bash
git commit --no-verify
```

**WARNING**: Only use `--no-verify` if you have a very good reason and understand you're violating d-012.

## Pre-Push Hook

Location: `.git/hooks/pre-push`

### Purpose

Prevents pushing code with build errors, warnings, or border radius styling.

### What It Checks

1. **Build Errors**: Runs `cargo build` and blocks push if compilation fails
2. **Build Warnings**: Blocks push if any compiler warnings exist
3. **Border Radius**: Scans all files for `border-radius` CSS property (design system violation)

### When It Runs

Automatically runs before every push to remote.

## Installation

Git hooks are stored in `.git/hooks/` and are **not** tracked by Git (they're in `.gitignore`).

### For New Contributors

When you clone this repo, you need to make the hooks executable:

```bash
chmod +x .git/hooks/pre-commit
chmod +x .git/hooks/pre-push
```

### Keeping Hooks Updated

Since hooks aren't tracked by Git, they must be manually updated when changed:

1. Pull latest changes
2. Check if hooks were modified (announced in PR/commit messages)
3. Manually copy updated hooks from documentation or reference source

## See Also

- [Directive d-012: No Stubs - Production-Ready Code Only](../.openagents/directives/d-012.md)
- [Pre-push hook source](.git/hooks/pre-push)
- [Pre-commit hook source](.git/hooks/pre-commit)
