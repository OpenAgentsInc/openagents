# Programmatic UI gap analysis

- Date: 2026-07-19
- Class: contract
- Dispatch: evidence collection only
- Owner: Sol issue #9035
- Status: active
- Schema: `openagents.ui-gap-evidence.v1`

## Purpose

This tool set compares two desktop user interfaces.
It collects source, build, runtime, accessibility, and visual evidence.
It does not use a Codex-only control interface.
An operator can run each tool from a normal terminal.

The tool set does not make product decisions.
A source match does not prove that a feature works.
A screenshot delta does not measure usability.
The final assessment keeps these limits visible.

## Evidence planes

The schema keeps six evidence planes separate.

| Plane | Purpose |
| --- | --- |
| Identity | Bind each result to a Git commit and tree. |
| Source | Find exact code samples for a selected UI axis. |
| Build | Record the command, duration, result, and artifact digest. |
| Runtime | Record a window, its controls, its text, and its screenshot metrics. |
| Comparison | Calculate source, control-role, and visual deltas. |
| Assessment | Record reviewed findings, priorities, confidence, and scope decisions. |

The assessment is a reviewed projection.
It is not a direct output from match counts.

## Files

- `ui-gap-evidence.schema.json` defines the evidence format.
- `ui-gap.mjs` runs the common checks and comparisons.
- `macos-ui-capture.swift` captures a native macOS window and accessibility tree.
- `macos-vision-ocr.swift` reads visible text from a reviewed fixture screenshot.
- `openagents-electron-capture.mjs` captures the isolated OpenAgents Electron DOM.
- `openagents-zed.config.json` pins the two source corpora and their probes.
- `ui-gap.test.mjs` tests the common runner.

## Requirements

Use Node.js 24 or a later compatible release.
Use Git for source identity and source inventory.
The macOS adapter also requires Swift and `screencapture`.
The OpenAgents adapter requires the repository `pnpm` install.

macOS can refuse a screen or accessibility capture.
Give the terminal process Screen Recording permission for screenshots.
Give it Accessibility permission only when a native tree is necessary.
Do not enable a permission for a source-only scan.

## Common run

Set the two roots in the operator shell.
The examples do not store the local root paths in evidence.

```sh
export OA_UI_ROOT="/path/to/openagents"
export ZED_UI_ROOT="/path/to/zed"
export UI_GAP_OUT="$(mktemp -d)"

node docs/sol/ui-gap-analysis/ui-gap.mjs doctor \
  --config docs/sol/ui-gap-analysis/openagents-zed.config.json \
  --root "openagents=${OA_UI_ROOT}" \
  --root "zed=${ZED_UI_ROOT}" \
  --out "${UI_GAP_OUT}/doctor.json"

node docs/sol/ui-gap-analysis/ui-gap.mjs scan-source \
  --config docs/sol/ui-gap-analysis/openagents-zed.config.json \
  --root "openagents=${OA_UI_ROOT}" \
  --root "zed=${ZED_UI_ROOT}" \
  --out-dir "${UI_GAP_OUT}"
```

Run an explicit build through the receipt wrapper.
The configuration file cannot execute a command.

```sh
node docs/sol/ui-gap-analysis/ui-gap.mjs record-command \
  --label zed-source-build \
  --cwd "${ZED_UI_ROOT}" \
  --public-cwd '<ZED_ROOT>' \
  --out "${UI_GAP_OUT}/zed-build.json" \
  --artifact target/debug/zed \
  --public-artifact '<ZED_BUILD>/zed' \
  -- cargo build -p zed --bin zed
```

Launch a native application before a native capture.
Use its process name or process ID.

```sh
node docs/sol/ui-gap-analysis/ui-gap.mjs capture-macos \
  --config docs/sol/ui-gap-analysis/openagents-zed.config.json \
  --target zed \
  --process Zed \
  --output-dir "${UI_GAP_OUT}/zed-window" \
  --out "${UI_GAP_OUT}/zed-runtime.json"

node docs/sol/ui-gap-analysis/ui-gap.mjs capture-ocr \
  --config docs/sol/ui-gap-analysis/openagents-zed.config.json \
  --target zed \
  --image "${UI_GAP_OUT}/zed-window/window.png" \
  --public-image window.png \
  --out "${UI_GAP_OUT}/zed-ocr.json"
```

Use the isolated adapter for OpenAgents.
The workspace must be a disposable directory.
The app profile always stays below the operating-system temporary directory.

```sh
mkdir -p "${UI_GAP_OUT}/workspace" "${UI_GAP_OUT}/openagents-window"

node --import tsx docs/sol/ui-gap-analysis/openagents-electron-capture.mjs \
  --repo-root "${OA_UI_ROOT}" \
  --workspace "${UI_GAP_OUT}/workspace" \
  --output-dir "${UI_GAP_OUT}/openagents-window" \
  --out "${UI_GAP_OUT}/openagents-runtime.json" \
  --analysis-id openagents-ide-vs-zed-ui-2026-07-19 \
  --target-id openagents
```

Compare the common evidence.

```sh
node docs/sol/ui-gap-analysis/ui-gap.mjs compare \
  --config docs/sol/ui-gap-analysis/openagents-zed.config.json \
  --left-source "${UI_GAP_OUT}/openagents-source.json" \
  --right-source "${UI_GAP_OUT}/zed-source.json" \
  --left-runtime "${UI_GAP_OUT}/openagents-runtime.json" \
  --right-runtime "${UI_GAP_OUT}/zed-runtime.json" \
  --out "${UI_GAP_OUT}/comparison.json"
```

## Add a target

Add one target record to a new configuration file.
Pin its commit and tree.
Select a small UI source corpus.
Add one regular-expression probe for each shared axis.
Do not add a build or launch command to the configuration.

Run `doctor` before a source scan.
Refuse a revision mismatch unless the assessment records it.
Run the common test after each runner or schema change.

```sh
node --test docs/sol/ui-gap-analysis/ui-gap.test.mjs
swiftc -typecheck docs/sol/ui-gap-analysis/macos-ui-capture.swift
swiftc -typecheck docs/sol/ui-gap-analysis/macos-vision-ocr.swift
```

## Safety and privacy

Use a disposable project for runtime tests.
Do not use a customer repository or a private prompt.
Do not capture a signed-in account page.
Do not include terminal output or editor text in the accessibility value field.
The native adapter records only numeric and Boolean values.

Keep raw captures in a temporary directory by default.
Commit only a bounded and reviewed evidence set.
Replace local roots with the public labels from the configuration.

## Interpretation rules

- A build result proves only that the named source built on the named host.
- A visible window proves only the selected launch state.
- An accessibility tree can omit custom-rendered controls.
- OCR can confirm visible fixture text, but it cannot prove control semantics.
- A DOM tree does not prove VoiceOver behavior.
- A pixel comparison needs a current fixture contract.
- A probe count measures evidence density, not feature completeness.
- A gap needs exact source or runtime evidence from both targets.
- An upstream feature is evidence, not OpenAgents product authority.
