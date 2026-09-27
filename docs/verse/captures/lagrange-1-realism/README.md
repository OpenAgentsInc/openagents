# Lagrange 1 realism captures

Offline renders from `lagrange_capture` (1280 × 800, reduced to 960 × 600)
recording the realism roadmap
([audit](../../../research/unreal/2026-09-27-lagrange-realism-audit.md),
[#9803](https://github.com/OpenAgentsInc/openagents/issues/9803)). Regenerate
them with:

```sh
cargo run --release -p verse --example lagrange_capture -- out.png VIEW
```

| File | View |
| --- | --- |
| `before-spawn.jpg`, `before-jig.jpg` | The amber-era renderer before the roadmap (commit `2ef0572339`) |
| `after-spawn.jpg` | The spawn view: the habitat half lit under the station's 30° pitch |
| `after-jig.jpg` | Flying toward the keel jig |
| `after-carry.jpg` | At the depot: crinkled insulation bands, the tank in the rack, the tether |
| `after-sunside.jpg` | The station from the sunward side |
| `after-wide.jpg` | A three-quarter view of the whole station |
| `after-earthzoom.jpg` | The Earth at a 1.7° field of view |
| `earth-vs-epic.jpg` | That render beside DSCOVR EPIC, 2026-09-25 11:10 UTC (NASA, public domain) |
| `after-moonzoom.jpg` | The Moon at a 0.7° field of view |
| `after-sunzoom.jpg` | The Sun through a solar filter (EV 31): limb darkening |
| `after-stars.jpg` | The art preset toward the galactic center: catalogue stars |
