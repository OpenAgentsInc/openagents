# Web-built Unreal games in the Metaverse

Written 2026-11-11. This doc is a workflow and a plan. Nothing in it has been
built yet. It is based on the UE 5.9 source (`Engine/Platforms/Wasm`, read
only), the DesertBlow project and its build logs, and our own Metaverse code. The
proof-of-concept tasks in section 9 have not been filed as issues.

Unreal games come into the OpenAgents Metaverse as **guest content**. They don't replace
the OpenAgents Metaverse Engine, Coax or our renderer, and they never carry Draconia or any other
MetaHuman. Our own worlds stay Metaverse realms.

## Summary

- **The Wasm target works on this Mac.** DesertBlow (a Third Person template
  project) packaged for the web on 2026-11-11 with
  `BuildCookRun -platform=Wasm -clientconfig=Development`:
  `BUILD SUCCESSFUL`, exit code 0, 1 min 25 s for an incremental run. The
  output is in `~/UnrealProjects/out/Wasm`. Epic's docs say "Windows is the
  documented host", but the macOS source build packaged without errors.
  **Nobody has booted that stage in a browser yet**, so the PoC starts by
  doing that.
- **Size.** The stage is 275 MB on disk, and a player's first visit downloads
  about **70.5 MB** (67 MiB):

  | File | On disk | Over the network |
  |---|---|---|
  | `DesertBlow.wasm` (Development) | 207.2 MB | 42.1 MB (Brotli) |
  | `DesertBlow-Wasm.ucas` (Oodle) | 16.0 MB | 16.0 MB |
  | `DesertBlow-Wasm.pak` | 9.2 MB | 8.3 MB (Brotli) |
  | `global.ucas` | 3.8 MB | 3.8 MB |
  | everything else (JS, `.utoc`, tables) | 0.9 MB | 0.3 MB |

  This is a Development build. Epic's release scan blocks Development builds
  from public hosting, so the public build is Shipping, and its size hasn't
  been measured. Epic's trim skill reports cutting a physics game's first play
  on the web from 50 to 28 MB.
- **What it runs on.** It renders with WebGPU only (no WebGL fallback) on the
  *mobile* renderer. Nanite, virtual shadow maps and hardware ray tracing are
  off. It runs as wasm64 with threads, a heap of 1 GB at start and 4 GB at
  most, and the page must be cross-origin isolated (COOP and COEP). Desktop
  Chrome, Edge and macOS Safari are supported. On iOS, Safari draws the 3D
  scene black, and Development builds get the tab closed.
- **Networking.** The browser can't open raw sockets, and engine HTTP is off.
  UE multiplayer needs a UDP-over-WebSocket relay and a native dedicated
  server.
- **Recommendation.** Host each game as a static stage on a **separate guest
  domain**: a GCS bucket behind Cloud CDN, with isolation headers and a strict
  CSP. Open it at the top level from a world card at
  `openagents.com/worlds/<slug>`. Give it a small `window.verse` bridge with
  game-scoped consent tokens, and a WORLDS button that returns to the Grid.
  Ship it single-player first, unlisted first, and on web first. Use pixel
  streaming only as an opt-in fallback. It costs about $0.71 per concurrent
  player-hour.

## 1. What it means

A **web-built Unreal game in the Metaverse** is a UE 5.9 project, packaged for the
`Wasm` platform, that we host and that players reach the way they reach any
world:

- a **card** on `/worlds` (listed) or only a URL (unlisted);
- a **gate** from the Grid or another world: a portal or a door that leaves
  for the game;
- a **URL**: `openagents.com/worlds/<slug>`, which opens the game.

It runs on the web first. It runs in our apps only where their web view can
run it (section 5).

### How it differs from a Metaverse Engine realm

| | Metaverse Engine realm ([realm-host.md](../verse/realm-host.md)) | Unreal guest game |
|---|---|---|
| Code | Coax compiled to a small wasm module per realm, in our repo | The creator's C++ and Blueprints, compiled by UBT and emscripten 6.0.10, outside our repo |
| Size | The fixture is 55 KB; packs have budgets of a few MB | About 70 MB on the first visit (DesertBlow, Development) |
| Host | `coax/web/host.js` on openagents.com, and the native Metal PageHost | Epic's page (`index.html`, `wasm-loader.js`, `sw.js`) on our guest domain |
| Renderer | Our WebGPU renderer, every frame driven by Coax | UE's mobile renderer on WebGPU |
| Rooms | Our room service (WebSocket or WebTransport, [room-loss-matrix](../audits/2026-11-11-room-loss-matrix.md)) | UE replication through a UDP relay to a UE dedicated server, or no multiplayer |
| Native apps | Everglade iOS and Mac run realms natively in the PageHost | Only in a WKWebView, and iOS isn't working yet in UE 5.9 |
| Saves, place, identity | Save slots, `place.coax`, the player key | Through the bridge (section 4), with consent |
| Release checks | Goldens, `realms.ts --check`, the release walk | Our admission checks (section 2.8) and Epic's `check_stage.py` and `webdoctor.py` |
| Who it's for | Us and makers building in Coax | Studios and creators who already work in Unreal |

### When to choose it

- You already have an Unreal project, or a team that works in Unreal.
- Your game needs UE's tools: Blueprints, Niagara, Sequencer, StateTree, GAS
  or the template gameplay.
- You can accept a download of tens of MB, the mobile renderer, and no phone
  support for now.

Choose a Metaverse Engine realm when the world must open on a phone over a slow line,
live in our rooms with Everglade's players, run natively in our apps, or be
built by our agents in Coax. That's every first-party world. Draconia and every MetaHuman
scene render in the Metaverse, never in Unreal
([metahuman-support.md](../verse/metahuman-support.md)).

## 2. The creator's workflow

### 2.1 Set up UE 5.9 from source

Follow [build-from-source.md](build-from-source.md). Then install
**emscripten 6.0.10**. No other version is accepted: `Config/Wasm_SDK.json`
pins the minimum and the maximum.

```sh
git clone https://github.com/emscripten-core/emsdk.git ~/emsdk
~/emsdk/emsdk install 6.0.10 && ~/emsdk/emsdk activate 6.0.10
export WASM_EMSDK_ROOT=~/emsdk
```

Use the engine's own Python
(`Engine/Binaries/ThirdParty/Python3/Mac/bin/python3`) for the web tools.
Install Playwright into it for the harness
(`Engine/Platforms/Wasm/Build/Tools/web/requirements.txt`).

### 2.2 Target Wasm

| Requirement | Why |
|---|---|
| A **C++ project**. A Blueprint-only project needs one C++ class added. | Preview 1 doesn't package content-only projects for the web |
| `OnlineSubsystemUtils` and `OnlineSubsystemNull` with `"PlatformDenyList": ["Wasm"]` in the `.uproject` | Online subsystems don't run in the browser. DesertBlow already does this. |
| **AI toolset plugins off when you cook**: `AIAssistant`, `AllToolsets`, `ToolsetRegistry` | They're game-feature plugins, and the Wasm cook fails with them on (Epic's GettingStarted, "Known limitations") |
| Project Settings > Platforms > Wasm (`[/Script/WasmTargetPlatform.WasmTargetSettings]`) | Page title, `StartMode=Click` (sound and pointer lock need a click), `MobileDevices` (warn, block or allow), the renderer tiers |
| Content within the renderer limits below | The web has no Nanite, VSM or ray tracing |

DesertBlow's settings are a good start:

```ini
[/Script/WasmTargetPlatform.WasmTargetSettings]
bTargetMobileDeferredTier=False   ; forward tier only: smallest cook, runs on every WebGPU GPU
PageTitle=Desert Blow
StartMode=Click
MobileDevices=Allow
```

**Renderer limits** (`Platforms/Wasm/Docs/SupportMatrix.md`):

| Feature | On the web |
|---|---|
| Renderer | Mobile forward (`SP_WEBGPU_ES3_1`) everywhere, and mobile deferred on desktop-class GPUs if it's cooked. The desktop tier (SM5, software Lumen) is experimental and off. |
| Nanite | Off. Meshes draw their fallback, so author real LODs. |
| Virtual shadow maps | Off. Cascaded shadow maps are used. |
| Hardware ray tracing | Off |
| Textures | BC by default (`texture-compression-bc`). Apple GPUs and phones need ASTC: see Epic's `web-texture-sets` skill. |
| Audio | RAD Audio, Bink Audio, Opus, ADPCM or PCM. There's no Vorbis. Sound starts after the first click or key press. |
| Input | Keyboard, mouse and pointer lock through the page (`Module.ueWebPage`). The page passes touches. Copy and paste in text fields doesn't work yet. |
| Memory | 1 GB heap at start, 4 GB at most. Set a fixed navmesh tile pool. |
| Threads | A pthread pool of Workers (Chrome stops at about 76). Needs COOP and COEP. |
| Known renderer bug | Mobile forward draws metal lit by a Reflection Capture black. Epic is fixing it. |

### 2.3 Build and cook

UBT builds the game target and UAT does everything after that. On this Mac:

```sh
UE=~/UnrealEngine-5.9/src
P=~/UnrealProjects/MyGame/MyGame.uproject
# the code (BuildCookRun -build does this too)
$UE/Engine/Build/BatchFiles/RunUBT.sh MyGame Wasm Shipping -Project=$P -Progress
# cook, stage, pak, package and archive a public build
$UE/Engine/Build/BatchFiles/RunUAT.sh BuildCookRun -project=$P -platform=Wasm \
  -clientconfig=Shipping -build -cook -stage -pak -iostore -package \
  -archive -archivedirectory=$HOME/UnrealProjects/out -unattended -utf8output
```

The command that succeeded for DesertBlow was the same command with
`-clientconfig=Development`. The archived stage, `out/Wasm/`, is the whole
site:

- `index.html` and the page scripts;
- `sw.js`;
- `<Project>/Binaries/Wasm/<Game>.{js,wasm}`;
- `<Project>/Content/Paks/*.{utoc,ucas,pak}`;
- the `.br` siblings;
- `wasm-manifest.json` (every file with its size and sha1) and
  `wasm-cache.json`.

Symbols and UAT manifests go to `out/Wasm-Debug` and never go public.

**Make it small.** Follow Epic's trim skill
(`Engine/Extras/AgentTools/Skills/unreal-trim/SKILL.md`):

- cut plugins and modules you don't use;
- cook only the maps you ship;
- set `bWithServerCode = false` for Wasm in a game with no authority code;
- relay the containers so what the boot reads comes first.

Measure every cut and run the game after it.

### 2.4 Test locally

```sh
PY=$UE/Engine/Binaries/ThirdParty/Python3/Mac/bin/python3
$PY $UE/Engine/Platforms/Wasm/Build/Tools/deploy/check_stage.py ~/UnrealProjects/out/Wasm
$PY $UE/Engine/Platforms/Wasm/Build/Tools/web/serve.py --root ~/UnrealProjects/out/Wasm --port 8900
```

Then run a headless boot. Our rule is **headless browsers only**: never open a
visible window on the owner's screen.

```sh
$PY $UE/Engine/Platforms/Wasm/Build/Tools/web/webrun.py run --stage ~/UnrealProjects/out/Wasm --fixture boot --out /tmp/boot
# or our own capture, with Chrome headless and WebGPU on
bun coax/web/shot.ts http://127.0.0.1:8900/index.html /tmp/boot.png 1280x720 20000
```

A boot passes when the console prints `UE_RUN BOOT ready <ms>` and
`UE_RUN READY`, prints no `UE_RUN ERROR`, and the shot isn't black. Check that
`webrun.py` runs Chrome headless before you use it. If it doesn't, use
`shot.ts`. `file://` never works.

### 2.5 Add a Metaverse manifest

Put `verse.json` next to the `.uproject`. It travels with every submission.

```json
{
  "v": "openagents.verse.guest.v1",
  "engine": "unreal-5.9-wasm",
  "slug": "desert-blow",
  "title": "Desert Blow",
  "line": "A third-person run across the dunes",
  "description": "A short third-person game made in Unreal Engine 5.9 and played in the browser.",
  "card": "card.jpg",
  "gate": { "from": "the-grid", "label": "DESERT BLOW" },
  "listed": false,
  "owner": "n-<pubkey>",
  "licence": { "game": "All rights reserved", "eula_accepted": "2026-11-11T14:00-06:00" },
  "rating": { "system": "verse-1", "value": "everyone", "notes": "cartoon fighting" },
  "multiplayer": "none",
  "bridge": ["identity.name", "character.look", "achievements"],
  "provenance": [
    { "path": "Content/Characters/Mannequins", "source": "epic-template", "licence": "UE EULA, Epic Content" },
    { "path": "Content/LevelPrototyping", "source": "epic-template", "licence": "UE EULA, Epic Content" }
  ],
  "contact": "lightning:creator@example.com"
}
```

| Field | Rule |
|---|---|
| `slug` | The world's address, `/worlds/<slug>`. Lowercase letters, digits and `-`, unique among realms and guest games. |
| `title`, `line`, `description` | Shown on the card and the world page. `line` is at most 60 characters, the same rule as realms. |
| `card` | A 1200x630 JPEG, at most 300 KB. |
| `gate` | Optional. The world whose portal leaves for the game. A gate in one of our worlds needs that world's owner to agree. |
| `listed` | `false` keeps the game off `/worlds` and off the Grid. Its URL still opens. Every game starts unlisted. |
| `owner` | The submitting account (`n-<pubkey>` or `w-<hash>`, [auth](../auth/2026-10-31-realm-invites-and-auth.md)). |
| `licence.eula_accepted` | When the creator accepted the Unreal EULA. Required (section 7). |
| `rating` | `everyone`, `teen` or `mature`. A `mature` game is never listed. |
| `multiplayer` | `none`, `relay` (the creator's own server, at their address) or `hosted` (ours, section 4.8). |
| `bridge` | The scopes the game asks for. The player sees them and agrees to them (section 4.1). |
| `provenance` | Every content folder in the cook, with its source and licence. Allowed sources: `own`, `cc0`, `cc-by` (with attribution), `fab-standard`, `epic-template`, `epic-content`. **`metahuman` is refused in every build.** Megascans or Fab assets need the creator's Fab licence tier. |

### 2.6 Submit

There are two equal ways to submit:

- **Web**: `/worlds/new/unreal` takes the archived stage as a zip, plus
  `verse.json`. It uses the upload protocol the site already has
  (`coax/web/uploads.js`: admit, upload, scan).
- **CLI or agent**: `openagents worlds submit ~/UnrealProjects/out/Wasm --manifest verse.json`
  signs each request with NIP-98.

The upload holds only `Wasm/`, never `Wasm-Debug/`. The creator keeps the
symbol map.

### 2.7 The agent path

An agent can build a game and ship it from start to finish:

1. **Author** in the editor through UE's MCP server ([mcp-tools.md](mcp-tools.md)):
   - levels and actors with `EditorToolset`;
   - Blueprints with `BlueprintToolsets`;
   - checks with `AutomationTestToolset`;
   - shots with `SceneCaptureToolset`;
   - frame times with `PerfToolset`.

   Connect with
   `claude mcp add --transport http unreal http://localhost:8000/mcp`. Keep the
   MCP server on loopback.
2. **Make it web-ready** with Epic's `web-deploy` skill
   (`Engine/Platforms/Wasm/Skills/web-deploy/SKILL.md`). For a new project,
   `new_template_project.py --template TP_ThirdPerson --out <dir> --web-ready`
   makes one from a template.
3. **Turn the toolset plugins off for the cook.** Set `"Enabled": false` for
   `AIAssistant`, `AllToolsets` and `ToolsetRegistry` in a copy of the
   `.uproject` that's used only for packaging, or limit them to the editor
   target. The MCP tools and the web cook can't share one enabled plugin set.
4. **Build, cook and trim** with the commands in 2.3, and follow
   `unreal-trim`'s measured loop.
5. **Boot it headless** (2.4) and keep `result.json` and the shot.
6. **Submit** with `openagents worlds submit`, signed by the agent's own key.
   The game belongs to the account the key is linked to.
7. **Read the admission report** (2.8) and fix what failed.

The agent never publishes on its own authority. Publishing is the account
owner's click, or an admin's (2.9). Epic's `web-deploy` skill already says
"never upload or publish anything", and we keep that rule.

### 2.8 Our checks

The admission checker is a Coax tool, `coax/web/guest/admit.coax`. It runs on
every submission. The two Epic Python scripts it calls are shipped with the
engine and are its firm reason for not being Coax.

| Check | Fails when |
|---|---|
| Manifest | `verse.json` is missing a field, breaks a rule in 2.5, or names a slug that's taken |
| Stage | Epic's `check_stage.py` fails: sha1s don't match, `.br` siblings are missing, or debug files are present |
| Release scan | Epic's release scan finds machine paths, user names, keys or tokens. A Development build always fails here. |
| Size | The bytes over the network are more than the limit for the tier (section 6). The card shows the size whatever it is. |
| Provenance | A cooked package path isn't covered by `provenance`, or a path or asset name matches MetaHuman content (`/MetaHumans/`, `MH_`, MetaHuman DNA or groom assets) |
| Code | The wasm imports something outside emscripten's and the UE page's runtime: no `eval` of fetched text, no imports we don't recognise |
| Boot | A headless Chrome boot on our checker host doesn't reach `UE_RUN READY` within 60 s, prints `UE_RUN ERROR`, or the frame is black |
| Frame time | It fails the listing bar in section 6. An unlisted game only gets a report. |
| Bridge | It calls a bridge scope that the manifest doesn't declare |
| Review | A person (an admin) plays it for two minutes before it's listed. Unlisted games skip this step. |

### 2.9 Publish

Publishing writes the stage to the bucket under a versioned prefix (3.3). It
adds a row to the guest table, `verse.guest_games`, with the slug, version,
owner, manifest, sizes, check report and listed flag.
`openagents.com/worlds/<slug>` then starts to answer. The first publish is
unlisted. Changing `listed` to `true` needs the review in 2.8 and an admin's
click. A listed game appears on `/worlds` and, if it has a gate, in that
world.

### 2.10 Update versions

Each submission is a new version (`v2`, `v3` and so on) and passes all the
checks again. The world page points at one live version. A switch changes
that pointer in one step, so a player gets either the whole old version or
the whole new one, never a mix. Keep the previous version online for a day,
because a tab that's still open reads its own files (Epic's `Deploy.md`, §2).

### 2.11 Roll back

A rollback moves the pointer back to the previous version. It takes one click
for the owner or an admin, or `openagents worlds rollback <slug>`. If a game
must stop for everyone at once, serve its `wasm-cache.json` as
`{"sw": "off"}`, mark the row `pulled`, and have the world page show "This
game is offline".

## 3. Hosting

### 3.1 Where the files live

| Piece | Choice |
|---|---|
| Bucket | `openagentsgemini-verse-guest`: one bucket for every guest game, with each game under `g/<slug>/<version>/` |
| Front | A new backend bucket with Cloud CDN, on the existing load balancer, with a host matcher for the guest domain. `everglade.world` already uses this pattern (`docs/verse/everglade-domain.md`). |
| Domain | A **separate registrable domain** for guest games (decision O1). A subdomain of openagents.com is the wrong choice: it would be same-site with our pages. |
| Streaming | Epic's loader reads the IoStore containers with HTTP Range requests and caches whole files in its service worker. GCS answers Range requests with 206. |

**Compression on GCS.** GCS can't choose between a file and its `.br` sibling
for each request, so we upload with these rules:

- `<Game>.wasm` and the page scripts are always read whole. Upload the Brotli
  bytes with `Content-Encoding: br`. Every browser that runs WebGPU accepts
  Brotli.
- `.ucas`, `.utoc` and `.pak` are read in ranges. Upload them raw, with no
  `Content-Encoding`. They're already Oodle-compressed.
- Never upload the `.br` siblings. Our checker confirms that no Range response
  has a `Content-Encoding` header (Epic's `webdoctor.py` does this check).

### 3.2 Headers

These are set as custom response headers on the backend bucket for the guest
domain, so every file gets them:

| Header | Value | Why |
|---|---|---|
| `Cross-Origin-Opener-Policy` | `same-origin` | Threads need `SharedArrayBuffer`, which needs a cross-origin isolated page |
| `Cross-Origin-Embedder-Policy` | `require-corp` | As above |
| `Cross-Origin-Resource-Policy` | `same-origin` | Under COEP every subresource must opt in |
| `Content-Type` | `application/wasm` for `.wasm`, `text/javascript` for `.js`, `application/json` for `.json`, `application/octet-stream` for `.ucas`, `.utoc` and `.pak` | Streaming compile needs the wasm type. `nosniff` refuses scripts with the wrong type. |
| `X-Content-Type-Options` | `nosniff` | |
| `Content-Security-Policy` | see 3.5 | |
| `Permissions-Policy` | `camera=(), microphone=(), geolocation=(), payment=(), usb=()` | A guest game gets none of these. Voice chat would need a decision. |
| `Cache-Control` | `no-cache` on `index.html`, `sw.js`, `wasm-cache.json` and `wasm-manifest.json`. `public, max-age=31536000, immutable` on everything else under `g/<slug>/<version>/`. | A version's files never change after upload, so the CDN can keep them. The entry files must revalidate. |

`coax serve` (`coax/web/serve.c`) already sends COOP and COEP, so the local
loop matches production. openagents.com itself isn't cross-origin isolated
behind the CDN. This plan doesn't need it to be (3.4).

### 3.3 Caching and versions

- **Paths**: `g/<slug>/<version>/index.html` and the rest of the stage under
  the same prefix. Nothing under a version is ever overwritten.
- **The pointer**: the `live` column of `verse.guest_games`. The world page
  sends the browser to `https://<guest>/g/<slug>/<live>/`. A switch or
  rollback changes the row and nothing else.
- **Cost of the simple path**: the service worker's scope is the version
  folder. An update therefore downloads the whole game again, instead of only
  the files whose sha1 changed. That's fine at first. If games update often,
  a stable `g/<slug>/current/` path can be rewritten to the live version at
  the load balancer, which brings back Epic's per-file delta. It's a later
  task, and not a decision for the owner.
- **Cleanup**: keep the live version and the one before it, and delete older
  versions after seven days.

### 3.4 Sandboxing

The game runs **at the top level on the guest domain**, not in an iframe on
openagents.com. Here's why:

- An isolated game can sit in an iframe only if the page around it is
  isolated too (`Networking.md`, `Deploy.md`). That would put COOP and COEP on
  `/worlds/<slug>`, and so change how every embed, image and script there
  loads.
- A separate site can't read our cookies or storage. `oa_session` and
  `oa_admin` are host-only, `Secure` and `HttpOnly`, and `oa_admin` is also
  `SameSite=Strict`. A different registrable domain also isn't "same-site", so
  `SameSite=Lax` cookies aren't sent on its requests to us.
- The game never gets our session. It gets only a game-scoped token (4.1).

The flow:

1. `/worlds/<slug>` on openagents.com shows the card, the size ("70 MB on the
   first visit"), the scopes the game asks for, and PLAY.
2. PLAY navigates the tab to the guest URL. Everglade's portal into
   `everglade.world` already works this way.
3. WORLDS in the game returns the tab to `openagents.com/worlds`.

### 3.5 CSP

```text
default-src 'self'; script-src 'self' 'wasm-unsafe-eval'; worker-src 'self' blob:;
connect-src 'self' https://openagents.com/api/guest/ wss://relay.<guest>/;
img-src 'self' data: blob:; media-src 'self' blob:; style-src 'self' 'unsafe-inline';
frame-ancestors 'none'; form-action 'none'; base-uri 'none'
```

- Epic's emscripten threads start their Workers from `blob:` URLs. The boot
  check confirms that the page boots under this policy.
- `connect-src` names only our guest API and, for a game with a hosted
  server, our relay. A game can't send data anywhere else.
- `frame-ancestors 'none'` stops other sites from framing a game we host.

### 3.6 Cost against the $1,500 a month cap

The guest bucket and CDN are new. Cloud CDN cache egress in North America
costs $0.08 per GiB for the first 10 TiB
([Cloud CDN pricing](https://cloud.google.com/cdn/pricing)). A first visit at
DesertBlow's 67 MiB costs about $0.005:

| First visits a month | Egress | Cost |
|---|---|---|
| 1,000 | 65 GiB | about $5 |
| 10,000 | 654 GiB | about $52 |
| 100,000 | 6.4 TiB | about $520 |

- Return visits read the service worker's cache and cost almost nothing.
- Storage at 275 MB per version is cents a month.
- The checker's headless boot runs on a host we already have (the Mac or the
  pool), not on a new always-on VM.

**Limits we show.** Each game has a monthly egress budget, $25 by default. The
creator's page shows the budget and the month's spend. When the budget runs
out, the world page says "This game reached its monthly download budget
($25); it opens again on <date>". An admin can raise the budget. The total of
all guest budgets counts against the GCP cap in the weekly spend audit.

### 3.7 Abuse

- Every game starts unlisted, and a person reviews it before it's listed.
- The release scan and the CSP stop data from leaving through
  `connect-src`.
- Each world page has a REPORT button. A pulled game is offline for everyone
  within a minute: we serve `{"sw": "off"}` and mark the row `pulled`.
- Uploads are limited per account: 3 versions a day and 2 GB in total for
  each game, and the upload screen states these limits.
- A `mature` game is never listed. A game that crawls our API or mines with
  the visitor's CPU is pulled, and its account loses the right to submit
  games.

## 4. The Metaverse integration contract

The bridge is one JavaScript file, `verse-bridge.js`, staged into the game
through Epic's template override (`<Project>/Build/Wasm/Template/`). We
supply it, and its firm reason for not being Coax is that it's browser glue
the UE page loads. C++ calls it through a small UE plugin, `VerseBridge`,
built with `EM_JS`. Blueprints get the same calls as Blueprint nodes. Every
call returns a promise, or fires a callback in the game.

```js
window.verse = {
  version: 1,
  who(),                 // {id, name, scopes} or null when the player is a guest
  character(),           // the player's look as a record, if "character.look" was granted
  achieve(id),           // records an achievement declared in verse.json
  presence(state),       // "playing", plus an optional short line
  friends(),             // friends online, if "friends" was granted
  pay(sats, memo),       // opens our pay sheet; resolves {paid, sats} or {paid:false}
  gate(slug),            // leaves for another world (the tab goes there)
  worlds(),              // back to openagents.com/worlds
};
```

### 4.1 Who the player is

1. The first time a player opens a game that asks for a scope, the world page
   on openagents.com shows the scopes in plain words, for example "Desert
   Blow wants your name and your character's look", with ALLOW and PLAY AS
   GUEST.
2. ALLOW mints a **game-scoped token**. It's valid for this game, this
   account and these scopes for 24 hours, and the server keeps only its hash.
   The token goes to the game in the URL fragment (`#vt=...`), so it never
   reaches a server log. The bridge reads it and then clears the fragment.
3. The bridge calls `https://openagents.com/api/guest/*` with
   `Authorization: Bearer <token>`. Those routes accept only guest tokens:
   never cookies, never `oa_session`. They answer CORS for the guest origin.
4. `who()` returns a **per-game pseudonymous id**,
   `HMAC(server key, account || slug)`. Two games therefore can't link the
   same player to each other.

A player who isn't logged in plays as a guest, and `who()` returns `null`.

### 4.2 The player's character

An Everglade character can appear in a UE game:

- **Format.** Characters come from the Quaternius humans and the Fantasy
  Source wardrobe, all CC0 1.0 (`docs/verse/character-creator.md`). We publish
  a **character pack for Unreal**: the base bodies, heads, hair and the 76
  garments as FBX for UE's import pipeline, plus glTF, and a short mapping of
  the appearance record (`aethyr.player.appearance.v1`) to mesh names and
  material slots.
- **Rig.** The Quaternius skeleton isn't the UE Mannequin skeleton. The pack
  ships an IK Retargeter asset that maps it to the Mannequin, so a game can
  play its own animations on our characters.
- **Runtime.** The game includes the pack's meshes in its own cook. `character()`
  returns the player's choices (body, head, hair, colours, garments, class),
  and the game puts them together. No mesh is fetched at run time.
- **Licence.** CC0 lets anyone use it, so a creator needs nothing from us. We
  never put MetaHuman characters such as Draconia in the pack.
  [metahuman-support.md](../verse/metahuman-support.md) and the
  private-pipeline rule apply.

### 4.3 Returning to the Grid

- **Our button.** We stage a WORLDS button into the page through the template
  override. It sits in a corner outside the game's canvas, and on a phone in
  the band beside the notch, as in the Everglade app. It works even while the
  game is paused or has hung.
- **The game's button.** `verse.worlds()` goes to `openagents.com/worlds`. A
  game with its own menu may call it from there. Our button stays either
  way.

### 4.4 Gates between worlds

- **Leaving the game.** `verse.gate("the-grid")` or `gate("<another slug>")`
  navigates the tab. The destination must be a listed world or one that the
  manifest names. Gates out of a game need no approval.
- **Entering the game.** A gate into a game sits in a Metaverse Engine world as a
  portal that opens `/worlds/<slug>`. Its owner must agree. The Grid's gates
  are ours, and we place them when we list the game.

### 4.5 Presence and friends

`presence()` posts the player's state to `/api/guest/presence`, so friends in
the Metaverse see "playing Desert Blow". `friends()` needs the `friends` scope and
returns the pseudonymous ids (4.1) and display names of friends who also
allowed this game. Nothing else is shared.

### 4.6 Achievements

The manifest declares achievements, each with an id, a title, an icon and
whether it's hidden. `achieve(id)` records one for the signed-in player.
They show on the player's profile under the game's name, and the creator's
page shows how many players earned each. The server refuses ids that aren't
declared. These are cosmetic records, never money.

### 4.7 Payments in Bitcoin

- `pay(sats, memo)` never touches a wallet inside the game. The bridge
  navigates to `openagents.com/pay?game=<slug>&sats=...&memo=...`.
- That page is ours. It shows the amount and the creator's name, and the
  player pays from **their own** wallet: Spark in our apps, or any Lightning
  wallet through the creator's Lightning address. We hold no funds, matching
  the custody design in
  [build-everything-audit](../research/bitcoin/2026-11-11-build-everything-audit.md),
  Decision 0.
- After the payment the tab returns to the game with a signed receipt the
  bridge checks. The amount is capped per payment, at 100,000 sats by
  default, and the pay page states the cap.
- These are tips and zaps to the creator. In-game purchases that unlock
  content wait for decision O6. Payments aren't live for our own makers yet
  either, so this ships after the first PoC.

### 4.8 Multiplayer

| Option | How it works | Cost | Verdict |
|---|---|---|---|
| None | Single-player. Presence and achievements come through the bridge. | $0 | **The default** |
| The creator's own server | The game runs UE netcode through Epic's `ws_relay.py` to a server the creator runs. Our CSP must allow their `wss://` relay. | $0 to us | Allowed when listed, after review |
| A UE dedicated server we host | A Linux UE server build plus a WSS relay on an e2-standard-2, like Everglade's room VM | About $55-60 a month per always-on VM. Most games need a fleet with match-making. | Only by decision O5, with a budget per game |
| Our room service | The game speaks our room protocol through the bridge instead of UE replication | Shared with the rooms we already run | Not now. It means writing UE-side code against [openagents-verse-protocol.md](../verse/openagents-verse-protocol.md) and loses UE's replication, prediction and GAS. |

Epic's relay forwards UDP to any address it allows, so we'd harden it before
hosting one: only TLS, an origin allowlist (the guest domain), and a single
upstream per game. In UE 5.9 browsers can't host, so there are no listen
servers.

## 5. Native apps

| App | How it runs worlds | Can it run a UE wasm game? |
|---|---|---|
| Everglade for iOS, Android and Mac (`apps/everglade-mobile`, `apps/everglade-desktop`) | The **Metaverse Engine PageHost** (`metaverse-engine/host/apple/PageHost.m`): Coax realm libraries on Metal. There's no web view. | **No.** PageHost runs Coax realm modules, not emscripten pages. |
| OpenAgents for iOS 26 (`apps/openagents-ios`) | A **WKWebView** that opens the Grid (`WebGridViewController.swift`) | Possibly. The guest page loads in a web view like any other page. |
| OpenAgents for Mac | Would need a WKWebView | Possibly, with the same work |

What blocks iOS today, according to Epic's own docs (`SupportMatrix.md`,
`GettingStarted.md`, "Known limitations"):

- iOS Safari draws the 3D scene **black** with no WebGPU errors. Epic is
  investigating.
- A Development build compiling its code holds about 913 MiB, and Safari
  closes the tab after about 12 s. A Test build of a large map peaks at about
  1.8 GiB.
- The page runs iOS single-threaded by default (`ue-threads-ios` is `single`,
  and the pthread pool is 1).

**What to test** in the OpenAgents iOS app on a real iPhone, and in the
simulator only as a second check:

1. `crossOriginIsolated` is `true` in the WKWebView on the guest page, so
   COOP and COEP are honoured.
2. `navigator.gpu` returns an adapter, and the scene isn't black.
3. Peak memory measured with `footprint` while a Shipping build boots and
   plays for 5 minutes. Note when the process is killed.
4. Touch input, sound after a tap, and WORLDS (the app's own button) working.
5. Frame time on the forward tier with ASTC textures.

The OpenAgents Mac app gets a WKWebView only after iOS passes. Until then,
Mac players use Safari or Chrome.

On App Store rules, guideline 4.7 covers HTML5 games offered inside an app
([App Store Review Guidelines](https://developer.apple.com/app-store/review/guidelines/#4.7)).
Our review (2.8) is the content check that guideline expects of us.

## 6. Performance and quality bars

These bars apply to **listing**. An unlisted game is only measured, and the
creator sees the report. Every measurement is a headless Chrome run on the
reference devices. The card shows the game's measured size and load time,
whatever they are, so players see them up front.

| Bar | Listed (desktop) | "Plays on phones" badge |
|---|---|---|
| First visit, over the network | at most 100 MB | at most 30 MB |
| Time to `UE_RUN READY`, cold, 50 Mbit/s (`webrun.py --net 50mbit`) | at most 30 s | at most 20 s |
| Time to ready, warm (from the service worker) | at most 8 s | at most 8 s |
| Frame rate, 2 minutes of play | p50 at least 60 fps, p5 at least 30 fps | p50 at least 30 fps |
| Peak memory (`--mem-every`) | at most 2.5 GB | at most 1.2 GB |
| Errors | no `UE_RUN ERROR` and no WebGPU validation error | the same |

**Reference devices.**

- Desktop: the owner's Apple-silicon Mac in Chrome, and an NVIDIA L4 Linux
  Chrome host (Epic's own cloud reference, with the Vulkan switches) for
  automated runs.
- Phone: none qualifies until section 5 passes on an iPhone 17 Pro, so no game
  gets the phone badge yet.

## 7. Legal

Epic's own EULA and FAQ pages returned HTTP 403 to our fetcher on 2026-11-11,
as they did for [ue6.md](ue6.md). The points below come from Epic's licence
summaries quoted in the press, a clause index of the EULA, and Fab's
documentation. **Have counsel read the current EULA before we list a game
that earns money** (decision O7).

| Question | What we found | Source |
|---|---|---|
| Royalties | 5% of gross revenue above the first $1M per product, over the product's lifetime. It's 3.5% under "Launch Everywhere with Epic". The royalty attaches to products that include engine code at run time, and a web build does. | [PC Gamer](https://www.pcgamer.com/unreal-engine-games-no-longer-owe-royalties-on-their-first-dollar1m-in-revenue), [MCV/Develop](https://mcvuk.com/business-news/epic-waives-royalties-on-first-1-million-in-unreal-engine-game-revenue/), [ConductAtlas EULA index](https://conductatlas.com/platform/unreal-engine/unreal-engine-eula/) ("Royalty Obligation Threshold at One Million USD", "Standard Royalty Rate Is Five Percent", "Reduced Royalty Rate of 3.5%") |
| Who owes them | The licensee, meaning the creator, for their product. The clause index names no royalty for a host or distributor, and lets publishers and distributors work on the licensee's behalf. **We don't owe royalties for hosting.** We'd become a licensee only if we built or sold the game. | ConductAtlas: "Royalty Obligation for Royalty Products", "Publishers and Distributors May Market on Your Behalf" |
| Engine binaries on the web | Engine code may be distributed only in object code, inside a product. A packaged `.wasm` is object code inside a product. End users may not take the engine out of it for their own products. We never host the engine's source, headers or editor. | ConductAtlas: "Object Code Only Distribution Requirement", "End Users Cannot Incorporate Licensed Technology Into Own Products" |
| AI training | Licensed Technology may not be used as input to train generative AI, and MetaHuman may not be used to build AI databases. **Guest builds and their files stay out of every training, Gym or dataset pipeline**, alongside the MetaHuman rule we already follow. | ConductAtlas: "Prohibition on Using Licensed Technology as Generative AI Training Input", "Prohibition on Using MetaHuman for AI Database Building" |
| Epic template and sample content | Epic Content, such as the Mannequin and LevelPrototyping in DesertBlow, may ship cooked inside a UE product. Not as raw assets, and not in another engine. | The UE EULA's Epic Content terms (not read first-hand); the "Examples May Be Distributed" clause |
| Fab and Megascans | The Fab Standard License allows use in any engine. It covers the finished product, not the source assets. It has a Personal tier (under $100k gross revenue in the last 12 months) and a Professional tier. Megascans claimed from Quixel or Bridge under the old Epic Content License are limited to Unreal, which a UE game is. | [Fab licences and pricing](https://dev.epicgames.com/documentation/fab/licenses-and-pricing-in-fab), [CG Channel](https://www.cgchannel.com/?p=179945), [Epic forum](https://forums.unrealengine.com/t/megascans-update-march-2025/2419561) |
| MetaHuman | The EULA treats MetaHumans as Non-Engine Products. Raw assets can't be redistributed, and MetaHumans can't be used for AI training. **We refuse MetaHuman content in guest builds entirely**, so we never host a MetaHuman inside a downloadable `.ucas`. | Our rule ([metahuman-support.md](../verse/metahuman-support.md)) |

**What the creator accepts at submission:**

1. They've accepted the Unreal Engine EULA and will pay any royalties it
   requires. We record the time they accepted.
2. They hold rights to every asset in `provenance`.
3. The game contains no MetaHuman content.
4. They agree to our guest terms: we may host it, review it, unlist it or
   pull it. The terms are drafted under decision O7.

## 8. Pixel streaming as a fallback

Pixel streaming runs the game natively on a GPU server and streams video to
the browser. Every device that plays video can play it, iPhones included, and
there's no large download.

| | Wasm (the default) | Pixel streaming |
|---|---|---|
| Cost per player | About $0.005 per first visit, then $0 | About **$0.71 per concurrent player-hour**: one NVIDIA L4 (`g2-standard-4`, us-central1, on demand, [Holori](https://calculator.holori.com/gcp/vm/g2-standard-4), [GCP pricing](https://cloud.google.com/products/compute/pricing/accelerator-optimized)) per session at 1080p |
| 100 players, 1 hour a day for 30 days | about $0.50 | about $2,100 |
| Latency | Local frames | Network round trip plus encoding, usually 50-150 ms |
| Phones | Not yet | Yes |
| Our GPU rule | No GPUs | Needs an owner ask, and GPUs are paused |

100 players for an hour a day breaks the $1,500 a month cap on its own.
Pixel streaming stays an **opt-in fallback for a named event**: a creator pays,
or the owner approves a fixed block of hours. It's never the default. Spot L4
prices from $0.17 to $0.33 an hour would lower the cost, but spot instances
can be taken away mid-session.

## 9. Proof of concept

Goal: DesertBlow, unlisted, at `openagents.com/worlds/desert-blow`. The game
opens on the guest domain, and WORLDS returns to the Grid. No identity, no
payments and no multiplayer in this PoC. The tasks below are issue-sized, and
none are filed yet.

| # | Step | Acceptance | Tasks |
|---|---|---|---|
| 1 | Boot the existing Development stage locally | `serve.py` plus a headless boot reach `UE_RUN READY`. The shot shows the Third Person level, not black. `result.json` and the shot are attached to the issue. | U1: headless boot of `~/UnrealProjects/out/Wasm`, recording BOOT phases and memory |
| 2 | A Shipping build | `BuildCookRun ... -clientconfig=Shipping` succeeds on this Mac. `check_stage.py` and the release scan pass. The size table is recorded here. | U2: Shipping build and size table; U3: `unreal-trim` pass, with before and after numbers |
| 3 | `verse.json` for DesertBlow | The manifest passes the checker. Provenance lists `Characters/Mannequins`, `LevelPrototyping`, `ThirdPerson` and `Input` as `epic-template`. | U4: manifest schema and `coax/web/guest/admit.coax` checking manifest, stage and provenance |
| 4 | Guest domain and bucket | The domain resolves. The backend bucket on the LB sends the headers in 3.2. `webdoctor.py` passes against it. `crossOriginIsolated` is `true`. | U5: register the domain (after O1); U6: bucket, backend bucket, host matcher and custom headers (`lb.sh guest-domain`); U7: uploader that applies the Brotli and raw rules from 3.1 |
| 5 | Upload and boot from the CDN | `https://<guest>/g/desert-blow/v1/` boots headless in under 30 s cold at 50 Mbit/s. A Range request on `.ucas` returns 206 with no `Content-Encoding`. | U8: `openagents worlds submit` (CLI, NIP-98) for admins only at first |
| 6 | The world page | `/worlds/desert-blow` shows the card, size and PLAY. It isn't on `/worlds` or the Grid (`listed: false`). PLAY opens the game. | U9: `verse.guest_games` table and the world page generated for a guest game; U10: `realms.ts --check` knows guest slugs, so slugs never clash |
| 7 | WORLDS back to the Grid | Our staged WORLDS button lands on `openagents.com/worlds` from a running game, headless. Leaving and reopening comes from the service worker in under 8 s. | U11: `verse-bridge.js` with `worlds()` and `gate()`, plus the WORLDS overlay in the template override |
| 8 | Rollback | Publishing v2 and rolling back to v1 switches the pointer in one step. The pulled kill switch takes the game offline within a minute. | U12: pointer switch, rollback and `{"sw":"off"}` |
| 9 | Cost guard | The game's egress budget and spend show on its admin row. Spending past the budget shows the stated message. | U13: per-game egress metering from CDN logs, and the budget message |
| 10 | Then | Identity (4.1), the character pack (4.2), the iOS WKWebView test (5) and the creator upload screen | U14-U17, after the PoC |

## 10. Owner decisions

| # | Decision | Recommendation |
|---|---|---|
| O1 | **The guest domain's name.** It must be a separate registrable domain, not under openagents.com. | Register one short name for every guest game, in the style of `everglade.world`, for example `openagents.games` if it's free. We pick the name and register it after your yes. |
| O2 | **Who may submit**: admins only, invited makers, or anyone with an account | Admins only for the PoC. Then the makers we invite through the realm-invite system. Not anyone, until review and abuse tooling have run for a month. |
| O3 | **Listing**: may guest games appear on `/worlds` and the Grid next to our realms? | Yes, after review, with a small "Made in Unreal" tag on the card. Our realms stay first in order. |
| O4 | **A GCP budget line for guest games** | $100 a month in total at first, at most $25 for each game, shown on screen and counted in the weekly audit |
| O5 | **Hosting UE dedicated servers** | Not now. Single-player and the creator's own servers only. Revisit when a listed game has 100 daily players. |
| O6 | **Money**: tips only, or also purchases inside games, and do we take a cut? | Tips and zaps only, with no cut, once maker tips are live. No purchases inside games until counsel reads the EULA and our terms. |
| O7 | **Counsel and terms**: a lawyer reads the current UE EULA and drafts our guest-content terms | Yes, before any game is listed that takes money. An unlisted free PoC doesn't need to wait. |
| O8 | **Pixel streaming** | Keep it off. Allow it only per event, paid by the creator or by a fixed block of hours you approve. GPUs are paused under the current rule anyway. |
| O9 | **iOS** | Test in the OpenAgents iOS app's WKWebView once Epic fixes the black scene. Until then, don't advertise phone play. |

## Sources

- UE 5.9 source, read only, at `/Users/christopherdavid/UnrealEngine-5.9/src`:
  - `Engine/Platforms/Wasm/Docs/README.md`, `GettingStarted.md`,
    `SupportMatrix.md`, `Deploy.md`, `Shipping.md`, `Networking.md`,
    `Performance.md`, `PageContract.md` and `Troubleshooting.md`;
  - `KIT.md`;
  - `Skills/web-deploy/SKILL.md`;
  - `Engine/Extras/AgentTools/Skills/unreal-trim/SKILL.md`.
- DesertBlow, read only: `DesertBlow.uproject`, `Config/DefaultEngine.ini`,
  `Saved/StagedBuilds/Wasm/wasm-cache.json`, and
  `~/Library/Logs/Unreal Engine/LocalBuildLogs/Log.txt` (2026-11-11
  12:34-12:36 CST, `BUILD SUCCESSFUL`).
- Our code:
  - `docs/verse/realm-host.md`;
  - `apps/openagents-web/realms.json`;
  - `coax/web/serve.c`, `coax/web/cdn.sh` and `coax/web/api/lb.sh`;
  - `metaverse-engine/host/apple/PageHost.m` and `apps/everglade-mobile/README.md`;
  - `apps/openagents-ios/OpenAgents/WebGridViewController.swift`;
  - `docs/auth/2026-10-31-realm-invites-and-auth.md`;
  - `docs/audits/2026-11-11-room-loss-matrix.md`;
  - `docs/verse/character-creator.md` and `docs/verse/everglade-domain.md`.
- Web:
  - [Cloud CDN pricing](https://cloud.google.com/cdn/pricing);
  - [Holori: g2-standard-4](https://calculator.holori.com/gcp/vm/g2-standard-4);
  - [GCP accelerator pricing](https://cloud.google.com/products/compute/pricing/accelerator-optimized);
  - [PC Gamer on UE royalties](https://www.pcgamer.com/unreal-engine-games-no-longer-owe-royalties-on-their-first-dollar1m-in-revenue);
  - [MCV/Develop](https://mcvuk.com/business-news/epic-waives-royalties-on-first-1-million-in-unreal-engine-game-revenue/);
  - [ConductAtlas UE EULA index](https://conductatlas.com/platform/unreal-engine/unreal-engine-eula/);
  - [Fab licences and pricing](https://dev.epicgames.com/documentation/fab/licenses-and-pricing-in-fab);
  - [CG Channel on Megascans](https://www.cgchannel.com/?p=179945);
  - [Epic forum, Megascans update March 2025](https://forums.unrealengine.com/t/megascans-update-march-2025/2419561);
  - [App Store Review Guidelines 4.7](https://developer.apple.com/app-store/review/guidelines/#4.7).
  - Epic's own [EULA](https://www.unrealengine.com/en-US/eula/unreal) and
    [FAQ](https://www.unrealengine.com/en-US/faq) returned HTTP 403 and were
    not read.
