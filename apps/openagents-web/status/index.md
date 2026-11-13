# OpenAgents status

How a change reaches production, and where things stand. The page at https://openagents.com/status draws this on the Grid; its commit list updates from GitHub when the page opens.

## Now

- Production runs `30361db7380eb3f194b7682af1183a53c41b8fd5`.
- main-green: `47f14799fdc225f078d522bb3ae20cf0bd71b09a`.
- 0 commits on main in the 24 hours before this release, 0 in 7 days. 0 of the day's commits name their agent.
- Queue: 0 waiting, 2 sent back. Red list: 0.
- Build hosts: 6 with 3 job slots each; 0 jobs in 24 hours.

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

- `30361db` remote.sh snapshot
- `124b14a` Specify Coax Native shared application architecture
- `cdbc5d6` Splice Bots: the battles' own stills (20 the desert battle, 21 the boss's phase two) and their HUD fixed from screenshots (#13505 part C)
- `19e6eb4` wasm_isolation.py drops COAX from its env, so its fixture compiler runs
- `146579a` The boat exit plan (#13474, stage 6)
- `0bc6709` /chat devbox conversations, and dependable dev boxes (#13474, stages 4 and 5)
- `27e37ca` Dev server: one account named by its email, and Log out sticks (#13493)
- `778ed18` The landing page draws again: verse.wgsl's entry points are bg_vs and fx_vs once more
- `09b306f` check-visuals: the play page's bar is an empty mount, like the editor's and a pane's
- `34d87d0` Microcoder on a real issue: the four runs on #13435, and where each part of #13445 stands (#13444, #13445)
- `cde308e` WIP doc2
- `93c3efa` Home page: someone logged in gets a new chat at /; no aborted view transition; serve drops stale module lists
- `1d40528` Splice Bots: bot battles, the drill mech, the raider mech, the two-phase boss and the ending (#13505 part C)
- `2a8e01b` Engine: bot battles (cards as moves, battery, health, core swaps) and the dinosaur machine kit (#13505)
- `d7258e7` Page checks: a fresh headless Chrome's first page on Linux gets a GPU (#13451)
- `65b2953` Splice Bots: the map fits between the HUD panels; the crosshair shows on light floors (#13505)
- `e3a3d20` Microcoder: a long read with no edit is told to edit; edits in one action may name the hash read before it (#13444, #13445)
- `c046166` Microcoder: restore the compiler's answers and the working memory, reverted by 0f2d65714d (#13444, #13445)
- `5765bd5` Splice Bots: the Director's controls and systems from the slice-1 key map (#13505, part A)
- `3b13184` Metaverse Engine: stance (sneak, sprint keys), carry (a slot grid) and vitals (health, power down, charge) (#13505)
- `4502712` Frame plan: every page without a plan of its own draws in the engine's order (#13451)
- `ab2b51a` API tests: script_cli_checks counts only Splice Bots' scripts
- `e5bc456` Editor bar: boxed buttons everywhere, one-row strip with More, Fullscreen, clean world edits
- `28f3520` Goldens: a page that became the app shell starts its engine in the held frame (#13487)
- `0f2d657` P1.2, P1.3: apps/openagents-web's sources move into a folder for each page; the old set goes under old/ (#13268, #13269)
- `6d88d1b` Microcoder: the compiler's answers and the working memory in the JSON-action loop (#13444, #13445)
- `f502e66` Realm edit docs: the layer's panes go with #13506
- `16ac833` Realm pages follow their realm's world and open the editor shell on it (#13478)
- `461218c` Splice Bots: Space jumps, on the engine's jump (metaverse-engine/move/jump.coax)
- `5587662` API tests: use the Postgres on the PATH when /usr/lib/postgresql has none
