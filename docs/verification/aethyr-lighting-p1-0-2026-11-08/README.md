# Aethyr lighting P1.0 identity check (2026-11-08)

Issue #13060 (umbrella #13068) moved Character Create's glade lighting out of the app into shared
Aethyr owners and made it the painterly style's default. This receipt is the identity check that
proves Character Create, Everglade II and Experimental render the same afterwards, against the P1.1
baseline (`docs/verification/aethyr-lighting-p1-baseline-2026-11-08/`, #13061). The bounds are that
baseline's; none were changed.

## Command

From the repo root, after `COAX_MUTATION=0 coax/build.sh`:

```sh
coax/web/tool.sh coax/web/lighting-baseline.coax OUT \
  --compare docs/verification/aethyr-lighting-p1-baseline-2026-11-08 --build
```

`--build` rebuilt the host modules, the everglade2 page pack and the everglade2 and fivelab wasm
first. Files here: `check.log` (the tool's output, OUT for the scratch directory), `compare.tsv`,
`run.tsv`, `probes.tsv` and `contact-sheet.jpg`. The 16 capture JPEGs are byte-identical to the
baseline's, so they are not copied again.

## Source and identities

| Item | Identity |
| --- | --- |
| Source | `cfca6e47a6` (origin/main) plus this change |
| Compiler | coax0, checked build `aa8be04a6d4d` (the baseline's) |
| everglade2.wgsl (Character Create, Everglade II) | `3e18d54100bc10b7…` (baseline `73d2db4d1ba0c146…`) |
| charactercreator.wgsl (Experimental) | `64ef4678044b2ca5…` (baseline `b4153a17309e55ee…`) |
| everglade2.wasm / fivelab.wasm | `90b43df978390c98…` / `33816148a24c24c6…`, the baseline's (no CPU page code changed) |

The shader hashes in `run.tsv` are those of the modules the pages actually compiled, and they match
the regenerated committed WGSL, so the captures exercise the new shared functions (`paint_*`,
`transfer_*`, `shade_*` behind the `cg_*` delegates). Host and device as the baseline: Mac17,6,
Apple M5 Max, Chrome 157 headless and muted, WebGPU metal-3.

## Result

PASS. Every capture and probe is inside the declared bounds, and in fact identical:

| Scene group | Captures | Blocks moved | Mean shift | Frame SHA-256 |
| --- | --- | --- | --- | --- |
| Character Create (glade, close-up, turn 0–3, phone) | 7 | 0 of 32400 each (phone 0 of 5194) | 0.0 | identical |
| Hand-over to Everglade II (0.5 s to 23.5 s) | 5 | 0 of 32400 each | 0.0 | identical |
| Everglade II (arrival, sunlit path, deep canopy) | 3 | 0 of 32400 each | 0.0 | identical |
| Experimental (Fire Bolt after an orbit) | 1 | 0 of 32400 | 0.0 | identical |

`probes.tsv` is byte-identical to the baseline's: 192 probes (144 pixel probes and the 48 GPU
lighting probes on Character Create's and Experimental's shaders), 0 outside their bounds. Because
nothing moved, no hand-over or Experimental gameplay drift needed separating from lighting.

## Rebased run

Before pushing, the change was rebased onto `3286289edf`, which adds the 0 A.D. hamlet to
Experimental's glade pack and changes Experimental and Everglade II page code (other agents' work).
The same check on the rebased tree (wasm rebuilt: everglade2 `4e69e9496084d33f…`, fivelab
`1d07149f1d8a9fa7…`; shaders unchanged, `3e18d541…` and `64ef4678…`) passed again with every capture
at 0 blocks moved, all 16 frame SHA-256s and `probes.tsv` identical to the baseline
(`check-rebased.log`, `run-rebased.tsv`, `compare-rebased.tsv`). Run it with `--serve PORT` when
another agent's server holds 8977: a first attempt here met another worktree's static server on
8977 and measured that tree (old shaders), so it was discarded and rerun on 8987.

## Unit checks

| Command | Result |
| --- | --- |
| `coax/bin/coax test aethyr/render/painterly.coax` | 102 passed (with the glade's moved checks and "the default's evaluation reads the glade profile's resolved values") |
| `coax/bin/coax test aethyr/render/transfer.coax` | 14 passed |
| `coax/bin/coax test aethyr/render/contact-shade.coax` | 15 passed |
| `coax/bin/coax test aethyr/world/environment.coax` | 59 passed |
| `coax/bin/coax mutate aethyr/world/environment.coax --root-only` | 652 mutants, 652 killed, 0 survived |
| `coax/bin/coax test verse/style-preset.coax` | 139 passed (with "the painterly preset's default environment resolves to the glade profile") |
| `coax/bin/coax test apps/openagents-web/gpu/charactercreator.coax`, `everglade2.coax`, `fivelab.coax` | 171, 200 and 170 passed |
| `coax/bin/coax test aethyr/world/day.coax`, `weather.coax`, `environment-region.coax`, `aethyr/render/environment.coax`, `verse/style.coax` | 146, 169, 78, 63 and 91 passed |
| `coax/web/tool.sh coax/web/wgsl-module.coax --site --check` | 56 modules checked, 0 stale, 0 failed |
