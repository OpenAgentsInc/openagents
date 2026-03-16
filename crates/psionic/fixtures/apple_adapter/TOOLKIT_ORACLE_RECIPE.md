# Toolkit Oracle Recipe For Apple Export Parity

This file is the deterministic local recipe for the `#3664` Apple export
parity gate.

The point of the oracle is not "a directory exists."
The point is:

- one package was exported by Apple's own toolkit path
- the live Swift bridge accepted it
- the repo can compare a candidate package against that oracle and ask the
  bridge whether the candidate also loads

## What Counts As The Oracle

Use a package produced by Apple's exporter:

- toolkit module: `export.export_fmadapter`
- output shape: `<name>.fmadapter`
- required live proof: the package loads through `POST /v1/adapters/load` on
  the local Swift bridge

The canonical gate for recording that proof is:

- `scripts/release/check-psionic-apple-export-parity.sh`

## Minimal Local Workflow

1. Build and run the Swift bridge.

```bash
cd swift/foundation-bridge && ./build.sh
./bin/foundation-bridge
curl -s http://127.0.0.1:11435/health
```

2. Produce or locate a toolkit-exported package.

This can come from:

- the toolkit's own export flow
- a toolkit-backed operator run that wrote
  `toolkit/export/<name>.fmadapter`

3. Choose the candidate package to compare.

For the pre-fix failure case, use the repo-native staged package that should be
rejected.

For the post-fix success case, use the repo-native export that should now load.

4. Run the parity gate.

Failure-expected candidate example:

```bash
scripts/release/check-psionic-apple-export-parity.sh \
  --oracle-path /path/to/toolkit/export/weather_helper.fmadapter \
  --candidate-path /path/to/candidate/weather_helper.fmadapter \
  --candidate-expected failure \
  --output-dir /tmp/openagents-apple-export-parity
```

Success-expected candidate example:

```bash
scripts/release/check-psionic-apple-export-parity.sh \
  --oracle-path /path/to/toolkit/export/weather_helper.fmadapter \
  --candidate-path /path/to/repo/native/weather_helper.fmadapter \
  --candidate-expected success \
  --output-dir /tmp/openagents-apple-export-parity
```

## Required Artifacts To Preserve

Keep the following outputs from the parity gate:

- `parity-report.json`
- `oracle-load.json`
- `candidate-load.json`
- `summary.json`

These are the machine-readable receipts that separate:

- package inventory or metadata parity
- from the only acceptance signal that really matters
- live bridge load success
