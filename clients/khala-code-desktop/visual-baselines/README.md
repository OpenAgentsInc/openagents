# Khala Code Visual Baselines

This directory is the default committed screenshot-baseline store for the Khala
Code fixture visual smokes and the packaged native AX smoke.

The store is intentionally manifest-first:

- `manifest.json` records the schema, stable screenshot ids, viewport,
  color-scheme, reduced-motion mode, PNG dimensions, SHA-256, and the
  redaction-check timestamp.
- `screenshots/*.png` holds blessed candidates after an owner runs a smoke with
  `--bless-baselines`.
- `deltas/*.delta.png` is ignored local output from compare failures.

Baseline entries must use relative paths only. The QA harness rejects manifest
metadata that contains local filesystem paths, credential-looking field values,
or unsafe ids.
