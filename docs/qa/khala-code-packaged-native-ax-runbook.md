# Khala Code Packaged Native AX Runbook

This is the Q3.1 headed smoke for issue #8023: `qa-runner` launches the built
Electrobun `Khala Code.app`, drives the real macOS window through the native AX
backend, submits a fixture prompt, and archives screenshots. This is not the
Vite preview path.

## Build

From the repo root:

```sh
bun install
bun run --cwd clients/khala-code-desktop build
```

The runner auto-discovers `Khala Code.app` or Electrobun's dev bundle
`Khala Code-dev.app` under
`clients/khala-code-desktop/build`, `clients/khala-code-desktop/.electrobun/build`,
or `clients/khala-code-desktop/out`. If the bundle lives somewhere else, set:

```sh
export QA_KHALA_CODE_APP_PATH="/absolute/path/to/Khala Code.app"
```

## Arm And Invoke

The native desktop backend is inert unless explicitly armed:

```sh
QA_NATIVE_DESKTOP=1 \
bun run --cwd apps/qa-runner khala:packaged-native-smoke -- \
  --out ../../var/qa-8023/packaged-native
```

Optional selector overrides are accepted as AX selectors (`AXRole:Name`) or
coordinate fallbacks (`point:x,y`) when labels drift:

```sh
QA_NATIVE_DESKTOP=1 \
bun run --cwd apps/qa-runner khala:packaged-native-smoke -- \
  --out ../../var/qa-8023/packaged-native \
  --hotbar-selector "AXButton:Fleet" \
  --composer-selector "AXTextArea:Message Khala Code" \
  --send-selector "AXButton:Send message"
```

The smoke can also compare or bless headed screenshots against the shared Khala
visual baseline store:

```sh
QA_NATIVE_DESKTOP=1 \
bun run --cwd apps/qa-runner khala:packaged-native-smoke -- \
  --out ../../var/qa-8023/packaged-native \
  --bless-baselines
```

Use `--require-baselines` to fail on missing entries and `--baseline-dir <dir>`
for a scratch store. See
[`khala-code-visual-baselines.md`](./khala-code-visual-baselines.md) for the
manifest format and diff behavior.

The July 2, 2026 owner-Mac proof needed coordinate fallbacks because the current
packaged WebKit AX tree exposed unlabeled buttons. This exact command completed
with three archived screenshots:

```sh
QA_NATIVE_DESKTOP=1 \
bun run --cwd apps/qa-runner khala:packaged-native-smoke -- \
  --out ../../var/qa-8023/packaged-native-point \
  --hotbar-selector point:47,196 \
  --composer-selector point:780,1025 \
  --send-selector point:1618,1036
```

The smoke launches the packaged app with fixture/no-spend defaults:

- `KHALA_CODE_DESKTOP_OPEN_WINDOW=1`
- `KHALA_CODE_CODEX_APP_SERVER_FIXTURE=1`
- isolated `CODEX_HOME` and workspace under the artifact directory
- token-usage sync disabled

## Artifacts

The artifact directory contains:

- `result.json` - shared public-safe qa-runner verdict
- `native-desktop-axtree.json` - latest AX tree snapshot
- `native-desktop-timeline.json` - screenshot frame index
- `native-desktop-*.png` - boot, hotbar, and submitted-turn screenshots
- `khala-packaged-native-smoke.json` - packaged smoke summary with stable artifact names
- `khala-desktop-stdout.jsonl` / `khala-desktop-stderr.jsonl` - packaged app logs

When baseline comparison is enabled, `khala-packaged-native-smoke.json` also
records public-safe `visualBaselines` entries with relative baseline and delta
paths.

Treat `status: "pass"` in `result.json` as the pass/fail source of truth.

## Failure Modes

- `NativeDesktopNotArmedError`: `QA_NATIVE_DESKTOP=1` was not set.
- `NativeDesktopUnavailableError`: macOS Accessibility permission is missing
  for the controlling app. Grant it in System Settings -> Privacy & Security ->
  Accessibility for Terminal, iTerm, Codex, or the app running Bun.
- `Packaged Khala Code app was not found`: run the build command above or set
  `QA_KHALA_CODE_APP_PATH`.
- `No executable was found`: the `.app` bundle is incomplete; rebuild it.
- `click failed: AX element not found`: inspect `native-desktop-axtree.json`.
  If the AX labels changed, rerun with selector overrides or a `point:x,y`
  coordinate fallback.
- `no-process` or focus failures after launch: the bundle name and AX process
  name differ. Rerun with `--app-process-name` or
  `QA_KHALA_CODE_APP_PROCESS_NAME`.
- Packaged app exits before or during the smoke: inspect the stdout/stderr JSONL
  artifacts first; the child process is always shut down before the runner exits.
