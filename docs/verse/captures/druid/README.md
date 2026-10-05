# Druid demo captures

Offline renders of the Grove ([druid demo](../../druid-demo.md)) from
`grove_capture` at 1280 × 800, and 844 × 390 for the phone. Regenerate them
with:

```sh
cargo run --release -p verse --example grove_capture -- out.png VIEW [ARGS]
```

The spider captures were taken before the four-row bar, so they show the
single row it replaced.

| File | View | Issue |
| --- | --- | --- |
| `spider.jpg` | `spider`: Wild Shape as the Giant Spider, mid-bite on a straw dummy, with Bite and Web on the bar | [#10610](https://github.com/OpenAgentsInc/openagents/issues/10610) |
| `spider-walk.jpg` | `spider-walk`: the Giant Spider walking across the meadow | [#10610](https://github.com/OpenAgentsInc/openagents/issues/10610) |
| `bar.jpg` | `field`: the four-row bar and the combat log | [#10609](https://github.com/OpenAgentsInc/openagents/issues/10609) |
| `tooltip.jpg` | `tooltip`: Fireball's card with its key, range, dice range, and save | [#10609](https://github.com/OpenAgentsInc/openagents/issues/10609) |
| `phone.jpg` | `phone`: one row and the row switcher | [#10609](https://github.com/OpenAgentsInc/openagents/issues/10609) |
| `moonbeam.jpg` | `spell 22 1.6`: Moonbeam's column burning a straw dummy | [#10609](https://github.com/OpenAgentsInc/openagents/issues/10609) |
| `call-lightning.jpg` | `spell 25 0.12`: a bolt from Call Lightning's storm | [#10609](https://github.com/OpenAgentsInc/openagents/issues/10609) |
| `wall-of-fire.jpg` | `spell 29 1.2`: Wall of Fire across the field | [#10609](https://github.com/OpenAgentsInc/openagents/issues/10609) |
| `sunburst.jpg` | `spell 36 0.25`: Sunburst over every dummy, with the blinded tags and the log | [#10609](https://github.com/OpenAgentsInc/openagents/issues/10609) |
| `conditions.jpg` | `spell 19 1.0 6 18 15`: Faerie Fire's outline and tag, Entangle's vines, and Poison Spray | [#10609](https://github.com/OpenAgentsInc/openagents/issues/10609) |
| `bear.jpg` | `spell 0 0.6 3 12`: Wild Shape as the Brown Bear, biting | [#10609](https://github.com/OpenAgentsInc/openagents/issues/10609) |
| `live-druid.jpg` | openagents.com/druid on the `new` tag in headless Chrome: Wild Shape: Giant Spider, then Burning Hands (`Alt+6`) | [#10611](https://github.com/OpenAgentsInc/openagents/issues/10611) |
