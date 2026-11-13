# OpenAgents status

How a change reaches production, and where things stand. The page at https://openagents.com/status draws this on the Grid; its commit list updates from GitHub when the page opens.

## Now

- Production runs `63e092cf684ef2778be89a337732d40fa27a402b`.
- main-green: not set. No commit has passed the full check yet.
- 222 commits on main in the 24 hours before this release, 1822 in 7 days. 9 of the day's commits name their agent.
- Queue: 0 waiting, 0 sent back. Red list: 0.
- Build hosts: 6 with 3 job slots each; 411 jobs in 24 hours.

## Live now

1. Builders work in a worktree from main.
2. They check the units they touched on the build hosts.
3. They rebase and push main.
4. `coax/web/release.sh` builds main once.
5. Staging gets the build: smoke checks, and every page starts.
6. Production gets the same files and is checked again. A release stops only for a check that fails on staging and passes on production.

## Built, not switched on

The green-main pipeline ([design](https://github.com/OpenAgentsInc/openagents/blob/main/docs/metaverse/green-main-pipeline.md)): submit, queue, an integrator that checks branches in parallel and lands the passing ones in order, a full check after landing, main-green, and releases from main-green. A failure is traced to its commit and filed for that commit's agent.

## Recent commits

- `63e092c` Everglade Experimental: the red dragon as a flying mount, and riding stages (#13113)
- `683ab0c` Aethyr: the camera orbit check splits in two, each well under its step budget (#13115)
- `575281d` wasm.sh: a unit past its compile headroom line stops a release build (#13115)
- `a8c3a05` Experimental terrain: the building sites are a constant table, not a chain of tests (#13115)
- `739fe14` Agent workflow audit: where agent time and tokens go, and the green-main cutover (#13102)
- `272d885` Experimental: a builder at each build site, and its landings' sounds (#13108)
- `c4c2d27` Housewright stages: a check that the camera never enters a house (#13107)
- `7697b0f` Everglade's side bar: an open menu and its tip push the frame-rate plaque aside (#13103)
- `e5c48a8` after.sh: a full run at most once an hour
- `eec10a9` check.sh: report module sizes against a start stamp
- `e023e90` Everglade Experimental: ?stage=house-* time-lapses own the camera, the sun and the clock (#13107)
- `f0e14a9` Everglade parity: spell effects through one shared module (#13086)
- `d1526b6` docs(293): owner decisions, the production format and what to collect for each beat
- `ebdb247` AGENTS.md: the Agent: trailer and the red list
- `22dac5e` release.sh: release origin/main-green by default
- `e1e0cc4` Green-main pipeline: after.sh runs the full check on main and bisects a failure to its commit
- `25b11c2` Plots and many build sites in Aethyr, Everglade's 16-plot town and its stages (#13109)
- `e359372` Experimental: a build site sounds its busiest material in a quick rhythm (#13108)
- `35a6455` Plans: seven implementation plans from a full audit, with an index
- `ae48886` Build sites: the piles' meshes, bottom first, and the carried item (#13108)
- `42b9c97` Build sites: material piles, a builder's trips and material sounds (#13108)
- `30374a9` Green-main pipeline: the integrator checks queued branches and lands the green prefix
- `c9e4770` remote builds: daily cap to $50 (owner, 2026-11-09)
- `6785c3d` Episode 293: retime transcript and show notes to the recut video
- `e50c65e` Everglade: the housewright time-lapse setups (#13107)
- `661a7e2` Aethyr: the glade's day profile, so any realm can sweep a day (#13107)
- `95362ab` Aethyr: a camera track any zone can load from a stage (#13107)
- `909813c` Aethyr build sites: one progress value, the arriving part and the materials used (#13106)
- `c54fb11` Aethyr HUD widgets: text fits its box, and side-bar rows are touch-sized on a tablet (#13103)
- `b204699` Everglade: the red dragon model, its clips and its place in the glade pack
