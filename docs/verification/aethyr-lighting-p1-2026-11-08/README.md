# Lighting Phase 1 exit (GitHub #13067)

Date: 2026-11-08. Plan: `docs/verse/lighting/README.md`, Phase 1 step 7 and the Phase 1 exit.
Baseline: `docs/verification/aethyr-lighting-p1-baseline-2026-11-08/` (P1.1). Earlier receipts:
`aethyr-lighting-p1-0-2026-11-08/` (identity), `aethyr-lighting-p1-5-2026-11-08/` (Everglade II's
set), `aethyr-lighting-p1-6-2026-11-08/` (the realms read the resolved record).

## Statuses

| Part | Status |
| --- | --- |
| Primitives (records, regions, day, weather, the frame record) | done |
| Everglade adoption (Character Create, Everglade II, Experimental) | done |
| Browser qualification | done (this receipt) |
| Native qualification | **not qualified**. No native host reads the resolved record yet, and no native capture or readback was run. |

## What changed in #13067

- **The ordinary default.** The resolved record is the only enabled light path in Character Create,
  Everglade II, Experimental (fivelab) and a made painterly realm. fivelab's duplicate word count
  now names `ENV_FRAME_FLOATS` (`U_ENV_WORDS`). No comparison switch is left.
- **Everglade II's sky plate follows the record.** The plate is still Everglade II's own texture.
  It is multiplied by `paint_env_sky_tint` (`aethyr/render/painterly.coax`, through `cg_sky_tint`),
  the ratio of the record's decoded sky colour to the glade's in the same direction. Under the glade
  the tint is exactly 1, so the day plate is unchanged. At night the plate darkens with the record
  (`e2-night.jpg`: the sky patch at top right is dark).
- **The light views.** `?light=ambient|direct|pregrade|postgrade|regions` on Everglade II and
  Character Create (developer-only, off without the query). `aethyr/render/environment-debug.coax`
  names the views and writes a six-line readout: region weights by profile id, ambient, key colour
  and direction, and an 18% grey and the zenith before and after the grade. The shaders read uniform
  word 51 (`E2_U_LIGHT_VIEW`): 1 ambient without albedo, 2 direct without albedo, 3 pregrade
  (display-encoded, no film). See `e2-view-*.jpg`.
- **The made realm in a browser.** `myrealm.coax` reports through its probe whether the frame used
  the glade (`realm_probe(5998)`) and its style block (`realm_probe(6000 + word)`, millionths).
- **The legacy probe.** `verse/tools/assets/glade_light_check.coax` binds a uniform block with the
  glade's record at word 208 and checks the current functions (`transfer_linear`, `transfer_srgb`,
  `cg_output`, `cg_air_depth`, `cg_irradiance`). The retired `cg_shoulder` check is replaced by the
  grade: the GPU's 18% grey must match the CPU's `env_debug_grade` of the glade (0.5105).
- **The harness.** `coax/web/lighting-baseline.coax` gained seven loads: `made-glade` (a made
  painterly realm, stubbed spec), `e2-night` (Everglade II with a night record at day 0), and the
  five light views.

## Phase 1 exit, item by item

| Exit item | Result | Evidence |
| --- | --- | --- |
| Identity: Character Create's glade unchanged within its bound | **pass** for the scene | `check.log`, `compare.tsv`. The cc captures move only in the left panel: the SRD agent's new LEVEL row (#13040) landed on main after P1.6. P1.6's compare of the same loads was 0 blocks moved. Light words: `env.tsv` shows 0 words off the glade's noon for cc, cc-phone, e2-arrival and exp-combat. |
| No jump at the hand-over | **pass** | `env.tsv` handover: 786 frames, 335 records, largest step 0.0101 (bound 0.02). handover-1 to handover-3 pass the image compare. handover-4, e2-sunlit-path and e2-deep-canopy move by exactly P1.6's numbers: the reviewed glade deltas of #13065. |
| A made painterly realm renders under the glade | **pass** | `made-glade.jpg`. `check.log`: "glade yes, 0 of 25 light words off the glade's noon (worst 0.0 millionths)", read back from the running page through `realm_probe`. |
| Check-first tests | **pass** | Already run in #13062 to #13066, not rerun here: midnight wrap (`environment.coax` "a fraction that rounds to one is midnight", `day.coax` "midnight wraps through the adapter"), irregular knots (`environment.coax` "irregular knots interpolate on each segment and across midnight"), invalid and nonfinite fields (`environment.coax` "duplicate and decreasing knot times are refused", "... refuse nonfinite ..."), overlaps (`environment-region.coax` "overlapping regions normalize by priority", "partial overlaps compose"), condition changes (`weather.coax` "condition changes: when the weather turns, the resolved light follows it"), bounded refusal (`environment-region.coax` "a bucket over capacity or a malformed region refuses the whole replacement", "a tile ... refuses rather than truncates"). Mutation: #13062 652/652 and 162/162, #13063 403, #13064 117/117 and 175/175, #13066 102/102. New here: `environment-debug.coax` (4 tests), the sky tint test in `painterly.coax`, the `?light=` test in `everglade2.coax`, the probe test in `myrealm.coax`. |
| Replay and late join agree | **pass** | `day.coax` "fixed-state replay ..." and `weather.coax` "fixed-state replay and late-join evaluation agree" (#13064). Everglade II's day comes from the room's W header through `day_from_room`, so a late joiner resolves the same record. |
| A malformed replacement never partly applies | **pass** | `environment.coax` "a set replaces the active set whole or not at all", `environment-frame.coax` "a refused set leaves the active set ..." and "a frame that cannot resolve writes the record before it, whole". |
| Browser readback of resolved values | **pass** | `probes.tsv` (GPU probes that bind the live uniform block) and `glade-light-check.log` (`ok: true`, all six checks, Apple metal-3). `e2-night` probes read the night sky's zenith near 0.001 and a key of about 0.2. |
| 60 FPS | **pass** | `realm-perf.log`: Everglade II at 60.0 FPS, p95 16.7 ms, 0 frames over 20 ms, at 1440x900 dpr 1 and 2 and phone. |

Not covered by this exit: the plan's walk along a pinned route through forest, town, a doorway
and a river. Everglade II's set has only the glade, the sunlit path and the deep canopy, so town,
doorway and river regions do not exist yet. Region continuity is covered by the region tests and
by the two region stations (`e2-view-regions.jpg` shows `REGIONS 12:1.000*` in the deep canopy;
`e2-view-direct.jpg` shows `1:0.088 11:0.912*` on the sunlit path).

## Notes on the run

- Run: `coax/web/tool.sh coax/web/lighting-baseline.coax docs/verification/aethyr-lighting-p1-2026-11-08
  --compare docs/verification/aethyr-lighting-p1-baseline-2026-11-08 --serve 8995`. It exits 1
  because the compare fails on the known moves above: the panel row and the reviewed deltas. The
  seven new loads have no baseline image and are recorded as new.
- fivelab was over its wasm budget on main before this change (2,604,270 of 2,602,000 bytes), so it
  was built with `COAXIAL_MODE=dev` and the run used no `--build`. everglade2.wasm is 691,588 of
  720,000 bytes. myrealm.wasm is 634,422 of 660,000 bytes.
- `glade-light-check.log` and `realm-perf.log` ran against a static server of
  `apps/openagents-web` on 127.0.0.1:8996.
