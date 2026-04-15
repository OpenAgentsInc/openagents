## Episode 223 live CS336 A1 dual-host proof

Retained on April 14, 2026 around `21:41-21:42 CDT` (`2026-04-15 02:41-02:42 UTC`)
while closing the final Episode 223 proof issues.

### Honest result

- The live-network dual-host homework bar is now met on the real proof pair:
  - `macbook-pro-m5`
    - provider pubkey `112667a1b22433dfe785bdafbd3c8a1f46be72979467fd2dad306428bdb0fc3a`
    - assignment `assign.run.cs336.a1.demo.window.cs336.a1.demo.0004.worker.2.attempt57`
    - contribution `18aedc0295b914638dd186fd2b3cd5c99a6bb03ac92c9ab13ecf06d21e104378`
  - `archlinux`
    - provider pubkey `5d80f7098245a5db2ba11e0f03ae0d934325fcfa9220453e279b7c559c0ba2c8`
    - assignment `assign.run.cs336.a1.demo.window.cs336.a1.demo.0004.worker.1.attempt64`
    - contribution `6e0ba5d7a9a754d8d99f3fbbf50967d463267ba952cb463b224118c0974284fd`
- Public Nexus now shows `window.cs336.a1.demo.0004` as `sealed` with
  `total_contributions=2` and `admitted_contributions=2`.
- Public Nexus `training/contributions` now exposes both the Mac and Linux
  contribution records on the same live window.
- Both machine-side `contribution_receipt.json` files say `outcome="succeeded"`
  with `exit_code=0`.
- GCS now contains both assignment contribution bundles plus
  `sealed_window_bundle.json` for `window.cs336.a1.demo.0004`.

This is enough to honestly close both:

- `#4338`, because one Mac Pylon and one Linux Pylon both materially did the
  homework path on the same live named CS336 A1 Demo window.
- `#4343`, because one fresh retained public + machine-side proof bundle now
  exists for that live dual-host run.

### Caveats

- Treasury remains degraded on the public board. That is still true in the
  retained `api-stats.json` capture.
- `window.cs336.a1.demo.0004` is currently `sealed`, not reconciled. The public
  summary still shows `pending_validation_windows=1`,
  `validator_challenges_open=3`, and `accepted_contributors=0`.
- Both proof hosts later fell back stale/offline on the control-plane loop
  after contributing. That does not erase the retained contribution receipts,
  public contribution records, sealed window record, or stored artifact bundle
  objects listed below.

The honest recording-day call for the Episode 223 dual-host homework proof is:

- record the live-network version, while stating plainly that treasury remains
  degraded and post-seal validation is still pending.

### Retained artifacts

Artifact directory:

- `docs/reports/pylon/artifacts/2026-04-14-episode-223-live-cs336-a1-dual-host-proof/`

Public captures:

- `api-stats.json`
- `training-summary.json`
- `training-nodes.json`
- `training-windows.json`
- `training-contributions.json`
- `gcs-window-0004.txt`
- `seal-0004-request.json`

Machine-side captures:

- `macbook-pro-m5-pylon-status.json`
- `macbook-pro-m5-training-status.json`
- `macbook-pro-m5-contribution-receipt.json`
- `macbook-pro-m5-artifact-manifest.json`
- `archlinux-pylon-status.json`
- `archlinux-training-status.json`
- `archlinux-contribution-receipt.json`
- `archlinux-artifact-manifest.json`
- `captured-at-cdt.txt`
- `captured-at-utc.txt`
