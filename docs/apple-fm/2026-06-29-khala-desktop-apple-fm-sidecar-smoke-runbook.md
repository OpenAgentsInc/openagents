# Khala Desktop Apple FM Sidecar Smoke Runbook

Date: 2026-06-29

Status: issue #6973 implementation runbook. This is an admitted-Mac smoke path,
not public product-claim copy.

## Scope

Khala Desktop's Electrobun Apple build bundles the Swift
`foundation-bridge` helper as a local sidecar at:

```text
Contents/Resources/app/apple-fm-bridge/foundation-bridge
```

The desktop Bun host may launch the packaged helper on macOS arm64, but Pylon
remains the Apple FM runtime authority. The webview receives only the
public-safe `appleFmReadiness` projection: state, blocker refs, provider/model
labels, usage-truth class, and redacted Pylon supervisor status.

## CI-Safe Checks

Run from the repository root:

```sh
bun run --cwd clients/khala-desktop verify
bun run --cwd apps/openagents.com check:deploy
```

After an Electrobun Apple build and before signing/notarization, run:

```sh
bun run --cwd clients/khala-desktop verify:apple-fm-bridge
```

That verifier passes only when one built Khala `.app` contains a non-empty
owner-executable helper at the packaged resource path.

## Admitted-Mac Smoke

Use an Apple Silicon macOS machine admitted for Apple Foundation Models.

1. Build the app locally:

   ```sh
   bun run --cwd clients/khala-desktop build
   ```

2. Run the packaged-helper verifier before signing/notarization:

   ```sh
   bun run --cwd clients/khala-desktop verify:apple-fm-bridge
   ```

3. Install the signed/notarized app and launch it from `/Applications`.

4. Start or adopt the local Pylon control node with a local control token. Do
   not publish the token.

5. Open the Khala Desktop operator view and confirm the Apple FM panel reports:

   - `state: ready`
   - provider `pylon-apple-fm-own-capacity`
   - model `apple-foundation-model`
   - demand source `khala_apple_fm_delegation`
   - usage truth `estimated`

6. Complete one local read-only Apple FM tool session through Pylon. The Pylon
   status must advertise both `probe.backend.apple_fm_bridge` and
   `probe.blueprint.tool_menu`; hardware detection alone is not enough.

## Public-Safe Evidence

Public closeout evidence may include:

- app build channel and version
- packaged-helper verifier pass/fail summary
- redacted `appleFmReadiness` state, blocker refs, provider/model labels, and
  supervisor health
- local Apple FM token row totals marked `usage_truth='estimated'`

Never include helper paths, loopback URLs, callback URLs, callback tokens,
Pylon control tokens, prompts, tool arguments, local file bodies, raw bridge
events, wallet material, or local auth paths.
