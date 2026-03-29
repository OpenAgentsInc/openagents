# WGPUI Headless Capture

`wgpui` now ships headless example capture binaries that render PNG review artifacts
without opening a window or relying on OS screenshot tools.

## Entry points

- gallery capture:
  - `cargo run -p wgpui --example gallery_capture -- --list-targets`
  - `scripts/wgpui/capture-gallery.sh --list-targets`
- storybook capture:
  - `cargo run -p wgpui --example storybook_capture --features storybook -- --list-sections`
  - `scripts/wgpui/capture-storybook.sh --list-sections`

## Common arguments

- `--width <px>`
- `--height <px>`
- `--scale <factor>`
- `--time-seconds <seconds>`
- `--frame <index>`
- `--fps <frames-per-second>`
- `--output <png-path>`
- `--output-dir <directory>`
- `--allow-fallback-adapter`

`--time-seconds` and `--frame` are mutually exclusive. If neither is provided,
the capture commands use a fixed `1.0s` snapshot time so animated demos remain
deterministic.

## Gallery examples

Single target:

```bash
scripts/wgpui/capture-gallery.sh \
  --target viz-primitives \
  --output target/wgpui-captures/review/viz-primitives.png
```

Batch capture:

```bash
scripts/wgpui/capture-gallery.sh
```

Supported gallery targets:

- `all`
- `viz-primitives`
- `component-showcase`

## Storybook examples

Single named section:

```bash
scripts/wgpui/capture-storybook.sh \
  --section Autopilot \
  --output target/wgpui-captures/review/storybook-autopilot.png
```

List sections:

```bash
scripts/wgpui/capture-storybook.sh --list-sections
```

Batch capture all sections:

```bash
scripts/wgpui/capture-storybook.sh
```

`--section` accepts the display name from `--list-sections` or a slug-like form
such as `autopilot`, `wallet-flows`, or `nostr-protocol`.

## Output contract

Default output roots:

- gallery: `target/wgpui-captures/gallery/`
- storybook: `target/wgpui-captures/storybook/`

Each capture writes:

- `<slug>.png`
- `<slug>.json`

Batch commands also write:

- `manifest.json`

The per-capture JSON files come from `wgpui::capture`. The batch `manifest.json`
lists every artifact emitted by the command so agents can inspect a directory
without guessing filenames.
