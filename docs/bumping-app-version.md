---
title: Bumping the App Version
---

# Bumping the App Version

This project uses the Expo app `version` as the runtime version and maps EAS Update channels to a `v<version>` string. Follow these steps to bump versions safely without blanket find/replace.

## 1) Pick the new version

- Use semantic versioning: `X.Y.Z` (no `v` prefix).
- App runtime: `X.Y.Z`
- EAS channel: `vX.Y.Z` (note the `v`).

## 2) Update the app runtime version

- Edit `expo/app.json` and set the Expo app version to the new value:

  - File: `expo/app.json`
  - Field: `expo.version`

Example:

```json
{
  "expo": {
    "version": "0.2.0"
  }
}
```

## 3) Update the EAS Update channel

- Set the production build channel to the new `vX.Y.Z`:
  - File: `expo/eas.json`
  - Field: `build.production.channel`

- Update OTA scripts to point at the new channel:
  - File: `expo/package.json`
  - Fields: `scripts.update:ios`, `scripts.update:android`, `scripts.update:both`

Example changes:

```json
// expo/eas.json
{
  "build": {
    "production": {
      "channel": "v0.2.0"
    }
  }
}

// expo/package.json
{
  "scripts": {
    "update:ios": "eas update --channel v0.2.0 --environment production --platform ios --message",
    "update:android": "eas update --channel v0.2.0 --environment production --platform android --message",
    "update:both": "eas update --channel v0.2.0 --environment production --message"
  }
}
```

## 4) Update docs that reference the channel/version

- Keep internal docs accurate when bumping:
  - `AGENTS.md` — references to channel and example `update:ios` command.
  - `docs/architecture-and-performance.md` — the OTA runtime version note.

## 5) Verify targeted changes only (no blanket replace)

Use ripgrep to confirm you updated only the intended places and avoided unrelated projects, lockfiles, or VCS data.

Search for the new version string (with optional `v`):

```bash
rg -n --hidden \
  --glob '!**/node_modules/**' \
  --glob '!**/.git/**' \
  --glob '!**/bun.lock' \
  --glob '!**/bun.lockb' \
  --glob '!Cargo.lock' \
  '\\bv?0\\.2\\.0\\b'
```

If you are bumping from a prior version, also search for the old strings to clean up lingering mentions in docs only (do not touch lockfiles):

```bash
rg -n --hidden \
  --glob '!**/node_modules/**' \
  --glob '!**/.git/**' \
  --glob '!**/bun.lock' \
  --glob '!**/bun.lockb' \
  --glob '!Cargo.lock' \
  '\\bv?0\\.1\\.2\\b'
```

Notes:
- Do not mass replace across the repo. Edit only:
  - `expo/app.json`
  - `expo/eas.json`
  - `expo/package.json`
  - Docs: `AGENTS.md`, `docs/architecture-and-performance.md`
- Avoid editing lockfiles (`Cargo.lock`, `expo/bun.lock`, `bun.lockb`) and third-party package versions.

## 6) Commit (and optionally push)

Stage the specific files you changed and create a focused commit:

```bash
git add expo/app.json expo/eas.json expo/package.json AGENTS.md docs/architecture-and-performance.md
git commit -m "Bump app version to v0.2.0"
# Optional: git push
```

## 7) Publish an OTA (iOS default)

Unless Android is explicitly requested, prefer iOS-only OTA updates:

```bash
cd expo
bun run update:ios -- "<concise change summary>"
```

The EAS channel must match `vX.Y.Z` exactly, and runtime version derives from `expo/app.json`.
