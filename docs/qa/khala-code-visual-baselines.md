# Khala Code Visual Baselines

Issue #8025 adds a PNG-aware baseline oracle for Khala Code screenshots. The
oracle lives in `@openagentsinc/khala-qa-harness/visual-baseline` and is wired
into the fixture visual smokes plus the packaged native AX smoke.

## Store

The default store is:

```sh
clients/khala-code-desktop/visual-baselines
```

`manifest.json` uses schema `openagents.khala_visual_baselines.v1`. Each entry
stores a public-safe id, harness name, viewport, dark/light mode,
reduced-motion mode, relative screenshot path, PNG dimensions, SHA-256, and
`redactionCheckedAt`. The manifest never stores local absolute paths.

## Bless Workflow

Run a deterministic smoke with `--bless-baselines` to copy screenshots into the
store and update the manifest:

```sh
bun run --cwd clients/khala-code-desktop smoke:cockpit-visual -- --bless-baselines
bun run --cwd clients/khala-code-desktop smoke:composer-visual -- --bless-baselines
bun run --cwd clients/khala-code-desktop smoke:part2-ui -- --bless-baselines
bun run --cwd clients/khala-code-desktop smoke:part2-fleet-gym-visual -- --bless-baselines
```

For the packaged native headed smoke:

```sh
QA_NATIVE_DESKTOP=1 \
bun run --cwd apps/qa-runner khala:packaged-native-smoke -- \
  --out ../../var/qa/packaged-native \
  --bless-baselines
```

Use `--baseline-dir <dir>` or `KHALA_CODE_VISUAL_BASELINE_DIR` for a scratch
store. Use `--require-baselines` or `KHALA_CODE_VISUAL_BASELINE_REQUIRE=1` when
a missing baseline should fail instead of soft-reporting `status: "missing"`.

## Compare Behavior

The oracle decodes PNG pixels rather than comparing raw files. Metadata-only PNG
encoding churn does not fail a run. A real pixel difference fails the oracle and
writes a relative `deltas/<id>.delta.png` image in the baseline store. Smoke
summaries include each screenshot's `visualBaseline` verdict, including
`matched`, `missing`, `blessed`, or the failing `changed` status.

The fixture visual smokes run in dark mode across desktop and mobile viewports;
mobile uses reduced-motion. The packaged native AX smoke records its headed
screenshots as desktop, dark, no-preference baselines.

## Public-Safety Guard

Baseline metadata is redaction-checked on read and write. The harness rejects
unsafe ids, absolute screenshot paths, local home paths, credential-looking
values, bearer/secret wording, and provider/raw-payload markers. The visual
smokes still run their existing DOM/text public-safety tripwires before
capturing screenshots.
