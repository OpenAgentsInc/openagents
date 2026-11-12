# OpenAgents status

How a change reaches production, and where things stand. The page at https://openagents.com/status draws this on the Grid; its commit list updates from GitHub when the page opens.

## Now

- Production runs `fa8065472257d3ae8232b0625b65a4d114914475`.
- main-green: `0c1a9e8eba04d41a827f753c2982e454cb724973`.
- 0 commits on main in the 24 hours before this release, 0 in 7 days. 0 of the day's commits name their agent.
- Queue: 0 waiting, 2 sent back. Red list: 0.
- Build hosts: 16 with 3 job slots each; 0 jobs in 24 hours.

## Live now

1. Builders work in a worktree from main.
2. They check the units they touched on the build hosts.
3. They rebase and push main.
4. `coax/web/release.sh` builds main once.
5. Staging gets the build: smoke checks, and every page starts.
6. Production gets the same files and is checked again. A release stops only for a check that fails on staging and passes on production.

## Built, not switched on

The green-main pipeline ([design](https://github.com/OpenAgentsInc/openagents/blob/main/docs/verse/green-main-pipeline.md)): submit, queue, an integrator that checks branches in parallel and lands the passing ones in order, a full check after landing, main-green, and releases from main-green. A failure is traced to its commit and filed for that commit's agent.

## Recent commits

- `fa80654` remote.sh snapshot
- `0f46553` Nostr relay: 256 connections and TCP keepalive, for production (#13361 step 4)
- `4854618` Nostr book: example loads name nostr/relay/ as the listings do; chapter 99's removed test
- `7be9850` Nostr book: chapter 99's weblogin.coax back, chapter 86 names the moved microcoder
- `9fa1b09` Nostr relay: NIP-11 lists 17 and 59, under the relay's own name (#13361 step 1)
- `65a5c95` Nostr relay: move the relay program and its server-only files to nostr/relay/ (#13361 step 0)
- `9c4fe97` Protocol E1: check the familiar and Healer's Kit profiles (#13244)
- `1154830` Realm edits: server slot names for board rows, undo restores exact bytes (#13354)
- `e0c2d69` CLI journey fake answers in the server's shapes (#13354)
- `cebda14` /chat's Claude turns read the repository through oa-repo (#13326)
- `a8b72f1` live-check.py chat: log in with the site session, as /chat does now (#13242)
- `f3fe9a3` P1.1b: the room loads no Everglade v1 file; layer-check gates it (#13346, #13267 step 7)
- `2d1d908` Protocol C5: the hosted runner's wire in one codec (#13242)
- `df89020` Indexed repos: the service oa-repo (#13326)
- `3fde47f` P1.1b: trim v1's town and cast queue from the room record (#13346)
- `7c1f551` Indexed repos: the schema openagents.repo.v1alpha1, its one codec and goldens (#13326)
- `d2241b7` P1.1b: delete v1's room rules, raid and forge files (#13346)
- `01a1f57` P1.1b: everglade/server/battle.coax replaces v1's room rules (#13346)
- `ef51b45` Microcoder: a failed ending lists the run's commits and files; a commit's result says how the last check ended (#13351)
- `ca90f67` Protocol E6: realm document and edits on one schema and one fixture (#13249)
- `5f894e0` Microcoder next (#13325): the Carbon field runs read offline; every wrong commit came from a run that ended on a limit
- `86cac5e` Seat ticket HMAC writer: mutation-clean (#13248)
- `d706f56` Seat ticket: player.c builds no ticket; v2-v4 come from seat_ticket_hmac_write (#13248)
- `6ea58b1` Seat ticket: v2-v4 HMAC writer exported to C, checked byte for byte against player.c's (#13248)
- `5882381` Protocol E4: the NPC ask's schema and one codec (#13247)
- `78312c7` DDD.7: the item ledger has one rules owner, ledger.coax; SQL stores (#13318)
- `6eec674` Protocol C3: stream records match their protos, profile coder-stream (#13240)
- `ae59216` verse/voice_smoke.c: define POSIX/default feature macros and M_PI fallback (#13348)
- `66d705a` seat-ticket: check HMAC writer against room tk_check profile (#13248)
- `15a4948` seat-ticket: add tk_write for HMAC v2/v3/v4 seat tickets (#13248)
