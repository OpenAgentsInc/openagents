# Reuse what exists

Owner rule, 2026-11-11: **never hand-roll something the repository already
has.** Before you write a text box, a button, an undo stack, a camera, a
controller, a parser, a hash or a model call, find the shared one in the
table below and use it. If it falls short, extend the shared one so every
app gets the fix. Never write a second copy, and never write a "small local
version for now".

If nothing fits, say so in your commit message and put the new code where the
next app can reuse it (`components/`, `metaverse-engine/`, `coax/lib/`, `everglade/`),
never inside one page.

Why: a hand-rolled copy is missing what the shared one already got right.
WorldBuilder's chat box was hand-rolled and lost Cmd+A, Cmd+C and Cmd+V; its
undo was hand-rolled and had no redo, while the OpenAgents Metaverse Engine's edit log had both.

## Where to look

| You need | Use | Where |
| --- | --- | --- |
| A text box: typing, selection, caret, undo, paste, IME | The edit buffer | `components/core/edit.coax` |
| Cmd/Ctrl+A, C, X, V and word deletion in a box on the website | The site's text keys | `apps/openagents-web/textkeys.coax` |
| Buttons, toggles, sliders, tabs, lists, panels, bars | The control kit, drawn in a skin | `components/kit/controls.coax`, `components/skin/` |
| A text surface on the web (a chat, a settings page, a document): buttons, fields, a composer, messages, tool calls, code blocks, notices, rows, menus | The frames set in the mono skin, drawn by the browser | `components/kit/frames.coax`, `components/skin/mono.coax`, `components/core/tree.coax`, `components/backend/dom.coax`, `coax/web/dom-host.js`; see them at `/components` |
| Panes round something that still shows (a canvas, a map, a document): a strip of actions, side panes that scroll, a field with its name, a fact | The dock, in the mono skin | `components/kit/dock.coax` (`k_dock`, `k_pane`, `k_prop`, `k_fact`), `mono_dock_css`; see it at `/components` |
| A field for many lines of code (Enter makes a line, Tab indents), and a wide right pane to hold it | The code field and the dock's wide pane, in the mono skin | `components/kit/frames.coax` (`k_code_field`), `components/kit/dock.coax` (`DK_WIDE`); see it at `/components` |
| Opening and saving a realm's source from the editor on the dev server | The editor's Code pane and `realm.code.v1`; one rule for which paths are the realm's own (`rc_path_ok`) | `metaverse-engine/editor/shell.coax`, `metaverse-engine/editor/code-wire.coax`, `coax/web/serve.c` (`code_local`) |
| A place to drop a file or pick one, and a file in a list | The drop target and the file row, in the mono skin; the browser's drag-and-drop and file picker are the host's (`dom-host.js`, "Files") | `components/kit/frames.coax` (`k_drop`, `k_file`); see them at `/components` |
| Turning a dropped glTF binary, PNG or sound file into an entry of a world's asset pack, with a declared licence and enforced limits | The editor's importer, on the existing pack writer (`pack_write`, read back with `pack_read`); its policy is `#authority` | `metaverse-engine/editor/pack-import.coax` (`pi_import`), `pack-import-policy.coax`, `pack-import-read.coax` (what a realm loads to read the pack) |
| Listing a world's imported assets, or sending an import, from the editor | `world.assets.v1`, `world.asset.import.v1` and `world.asset.upload.v1` (a file named before a direct upload), one codec; the shell's Assets pane | `metaverse-engine/editor/asset-wire.coax`, `metaverse-engine/editor/shell-assets.coax`, [the shapes](protocol/world-assets-v1.md) |
| Answering a world's asset routes on a server (the list, a file named before it is sent, an import with its fit check) | One body of code for the dev server's tool and the API | `metaverse/splice-bots/world-assets.coax` (`swa_list`, `swa_import`, `splice_world_asset_*`) |
| A world's stored realm scripts on a server: the list, a refusal with its code, the limits, a publish's confirmation; reading a world's script rows from its content tables | The stored scripts' codec and the rows' scan; the server's rules are `#authority` | `metaverse-engine/script/store.coax` (`ss_list_read`, `ss_refusal_read`), `metaverse-engine/script/rows.coax` (`srw_scan`), `coax/web/api/script-policy.coax`, [the contract](protocol/world-scripts-v1.md) |
| Compiling a realm script's source that someone else wrote | The compile job, never the build tools directly; on a server, the compile service in front of it, run by the one job runner | `coax/web/script-job.sh`, `metaverse-engine/tools/script-job.coax`, `coax/web/script-job/` (`oa-script-job`), `coax/web/api/script-job-run.c` |
| An identity token to call a sibling Cloud Run service | The API's one token call | `coax/web/api/inference.c` (`api_id_token`; `api_id_token_for_url` takes the audience from a URL's origin) |
| Opening a direct upload to the private bucket, reading an object back, or deleting one, from an API route | The drops routes' bucket calls: an upload for an exact size and content type, answered to this site's page | `coax/web/api/drops.c` (`drops_open_upload`, `drops_inspect`, `drops_fetch`, `drops_delete_object`) |
| A placeable kind whose mesh is an imported mesh | A `pieces` row whose `kit` is `pack/NAME`: drawn from the world's import pack, its footprint held to the mesh's bounds | `metaverse/splice-bots/pieces.coax` (`sp_mesh_write`, `sp_imports_read`), `metaverse/splice-bots/world.coax` (`spw_mesh_rows`, `spw_import_kind`) |
| An editor over an engine page (`?edit`): the world's things, the selected thing's fields, undo, redo, publish, refusals | The editor shell; a page tells it its world as `editor.model.v1` and takes its `world.edit.v1` batches | `metaverse-engine/editor/shell.coax`, `shell-model.coax`, `docs/protocol/editor-model-v1.md`; `coax/web/tool.sh editor-skin-check` |
| Picking a thing in the world, a move, turn or size gizmo that snaps, a fly camera, pause, step and simulate, a brush on the land | The viewport's tools: `vp_pick` (`query_ray`), `vp_press`, `vp_drag`, `vp_release`, `vp_fly_*`, `vp_run_dt`, `vp_act`, `vp_dab` | `metaverse-engine/editor/viewport.coax`; Splice Bots' use is `spv_*` in `metaverse/splice-bots/world.coax`, WorldBuilder's is `tool_*` in `apps/openagents-web/worldbuilder.coax` |
| A world kept in the browser behind `world.edit.v1`: typed edits checked on the page, batches fenced by revision, a model for the shell | WorldBuilder's world | `apps/openagents-web/worldbuilder/worldbuilder-world.coax` (`wbe_thing`, `wbe_dab`, `wbe_batch`, `wbe_model`) |
| A value that is stepped, not typed (a brush's radius, a count) | The stepper, in the mono skin | `components/kit/dock.coax` (`k_stepper`); see it at `/components` |
| A named thing that is on or off, in a pane (a board's row, a tool visitors get) | The switch row, in the mono skin: a name, a line under it and the kit's one switch (`k_switch`) | `components/kit/dock.coax` (`k_switch_row`); see it at `/components` |
| A world's content table whose rows are each on or off, shown and switched in the editor | The model's `list` and `sw` lines (`em_list`, `em_switch`; `atonce` for a world of one lane) and the shell's lists; a press is a `cell` edit | `metaverse-engine/editor/shell-model.coax`, `metaverse-engine/editor/shell-lists.coax`, [the model](protocol/editor-model-v1.md) |
| A whole number dragged between two ends (a body's height, a volume) | The slider, in the mono skin: the browser's own range control, so the arrow keys move it | `components/kit/dock.coax` (`k_slider`); see it at `/components` |
| Panes of a page's own beside its canvas (a creation screen's steps): a module the browser draws, told what the page holds, its actions sent to the page as messages | A page's own pane: the page exports `coaxial_pane` and `coaxial_pane_model`; the host opens the module and frames the canvas in what the panes leave free | `coax/web/host.js` ("A page's own pane"); Crew's is `apps/openagents-web/crew-pane.coax` |
| A person drawn in the cel style whose body is a record of numbers and whose clothes are fitted to it; a smooth skin over a row of rings, and a line, a patch or a frame lying on one | The crew's body, head, wardrobe and document, on the cel lofts | `metaverse-engine/kit/crew-body.coax`, `crew-head.coax`, `crew-wardrobe.coax`, `crew-doc.coax`, `metaverse-engine/render/cel-loft.coax`; see them at `/worlds/crew` |
| Where things go when they are asked for by an anchor and a side, several spread out, one nudged clear; a group saved as a template and placed as one edit | The placement planner: `pl_plan` behind a catalogue (`PlaceKind`), `pl_group`, `pl_beside`, `pl_template_*` | `metaverse-engine/editor/place.coax`; WorldBuilder (`wb_place`, `wb_kind`) and Splice Bots (`spw_plan_add`, `spw_kind`) use it |
| An icon | Apps SDK UI's 755, by name (`IC_COPY`), drawn from their SVG files; `k_icon` and `k_icon_button` in the frames set | `components/core/icons.coax`, `components/icons/apps-sdk/` |
| ARWES frames, easings, the animator, text effects, backgrounds | The ARWES ports | `components/core/arwes/` |
| A chat on the GPU (the desktop app): composer, transcript, Markdown, tool chips | The earlier chat set | `components/zeron/`, `components/core/markdown.coax` |
| Measuring and wrapping text, fonts | Layout | `components/layout/` |
| A game HUD: bars, hotbar, windows, numbers | The HUD sets | `components/kit/hud.coax`, `everglade/hud/`, `metaverse/hud.coax` |
| Drawing UI on the GPU or in a terminal | Backends | `components/backend/` |
| Editing a world: typed edits, undo and redo, revision-checked saves | The Metaverse Engine's edit log | `metaverse-engine/editor/edit.coax`, `metaverse-engine/editor/doc.coax` |
| Publishing or packing a realm | Metaverse Engine | `metaverse-engine/editor/publish.coax`, `metaverse-engine/editor/pack.coax` |
| The player character, walking, WASD, jump, touch stick | The Everglade zone controller | `everglade/zone.coax`, `everglade/zone-touch.coax`, `metaverse-engine/move/walk.coax`, `metaverse-engine/move/player.coax`; the jump alone, for a realm with its own walk: `metaverse-engine/move/jump.coax` |
| Which clip a character plays: standing, a gait by its speed, a cued action, a fall, an action over the upper body | The blend graph. A game supplies a table (clips, states, transitions, blend spaces, layers with a joint mask, IK nodes); `anim_graph_step` answers with the clip, the clip it left and the cross-fade. Everglade's table and the ECH1 locomotion table are the two in use | `metaverse-engine/character/graph.coax`, `everglade/scene/body-graph.coax`, `metaverse-engine/character/echo-graph.coax` |
| A posed character from the graph's answer, a foot on rising ground, a head or hand toward a point; a knee or elbow for a reach | The graph's pose path and its IK nodes, on the one two-bone reach | `metaverse-engine/character/graph-pose.coax` (`anim_graph_pose`, `anim_graph_ik`), `metaverse-engine/anim/two-bone.coax` (`two_bone_mid`, `two_bone_pole`, `two_bone_from_to`) |
| An angle: atan2, a yaw from a direction, wrapping to [-PI, PI) | The engine's angle module, which any module can load (a file without a module of its own can't be loaded under two) | `metaverse-engine/core/angle.coax` (`angle_atan2`, `angle_yaw_of`, `angle_wrap`) |
| A follow, orbit or free camera | The Metaverse Engine's cameras | `metaverse-engine/cam/follow.coax`, `metaverse-engine/physics/orbit.coax`, `metaverse-engine/cam/camera.coax`, `metaverse-engine/cam/` |
| Characters, rigs, faces, poses, materials | The Metaverse Engine's character | `metaverse-engine/character/`, `metaverse-engine/character/character-appearance.coax` |
| Ground, terrain, heights, ground rays | The Metaverse Engine's terrain | `metaverse-engine/world/terrain-tile.coax`, `metaverse-engine/physics/ground.coax` (the one sampler), `metaverse-engine/physics/ground-query.coax` (rays), `metaverse-engine/physics/ground-point.coax` (plain height, normal and ray readers) |
| Collision, contact, queries | Metaverse Engine | `metaverse-engine/physics/collide.coax`, `metaverse-engine/physics/sim/contact.coax`, `metaverse-engine/physics/query.coax` |
| Scattering things, grass, props | Metaverse Engine | `metaverse-engine/world/scatter.coax`, `metaverse-engine/render/` |
| Rendering, lighting, shadows, quality tiers | The Metaverse Engine's renderer and governor | `metaverse-engine/render/`, `metaverse-engine/core/quality.coax`, `metaverse-engine/core/frame-pacing.coax` |
| Multiplayer: transport, replication, prediction, rewind | The Metaverse Engine's netcode | `metaverse-engine/net/` |
| Steering and pathfinding | The Metaverse Engine: the steering service, and the navigation cells built from colliders ([how](metaverse/navigation.md)) | `metaverse-engine/ai/`, `metaverse-engine/world/nav/`, `everglade/shared/nav.coax` |
| A world in cells: which cells load, simulate and draw round the players | The Metaverse Engine's cell controller, and Everglade's manifest ([how](metaverse/one-world.md)) | `metaverse-engine/world/cells.coax`, `everglade/world/zone.coax` |
| The Grid | The Grid and its session | `metaverse/grid.coax`, `metaverse/gridwalk.coax` |
| 3D models | The asset registry | `assets/registry/` (read its README) |
| Hashes, signatures, encryption | Coax crypto | `coax/lib/crypto/` |
| Nostr events, NIPs, NIP-98, zaps | Coax Nostr | `coax/lib/nostr/` |
| An app's login to the account: email code, a key, passkeys, access and refresh tokens, a request with `Authorization: Bearer` | The apps' login (iPhone and Mac) | `apps/shared/host/apple/AppAuth.swift` (`authorize` for any request) |
| A hardware key that ties an app's login to its device (Secure Enclave, Android Keystore), and its proof | The shared device key | `apps/shared/host/apple/DeviceKey.swift`, `apps/shared/host/android/device-key.c`, `coax/web/api/auth-device-crypto.coax` |
| Bitcoin and Lightning | Coax Bitcoin and Lightning | `coax/lib/bitcoin/`, `coax/lib/lightning/`, `coax/lib/lnaddr/` |
| A wire format: messages between apps, the API, rooms or devices; a file or log line a program stores | The protocol's schemas and the one codec file each shape has | `proto/` (read its README). Look the shape up in `proto/registry.json` and load its `codec` file; never write its JSON, frame or id by hand in a second file. For a new shape, add the schema and a registry row ([how](../proto/README.md#register-a-wire-shape)). `coax/web/tool.sh wire-check` fails on an unregistered shape |
| JSON, protobuf, base64, Unicode, regex | Coax formats | `coax/lib/json/`, `coax/lib/proto/`, `coax/lib/text/` |
| HTTP, sockets, processes, files, time | Coax system | `coax/lib/net/`, `coax/lib/sys/`, `coax/lib/clock.coax` |
| Browser storage | The host's byte storage | `coax/web/STORAGE.md` |
| A page's state layout: a field's offset, size and type by path; what a code reload keeps | The build's field table (`coax layout`), which `coax drive` reads and a dev module carries | `build/.drive/APP.layout`, `coax/web/drive.coax`, `coax/web/state-keep.js` |
| A model call (Gemini, embeddings) | The API's Vertex client | `coax/web/api/inference.c`, `coax/lib/systemone/` |
| A decision (classify, route, rank) | The Decisions API | `coax/lib/systemone/` |
| A decision (classify, route, rank): Gemini Decision 1 first, the model you used before as the fallback (since 2026-11-13, #13473) | The Gemini door: `gd_decide`, or `so_decide_gemini` for a System One body, with `so_gemini_final` to tell when to fall back; in Coder, `agr_hosted` and `agr_send` | `coax/lib/systemone/google.coax`, `coax/lib/systemone/gemini.coax`, `coax/lib/agent/engine/route.coax` |
| A game character that asks the Decisions API through `/api/v1/systemone` with `"model":"auto"` and must accept either door's answer (Gemini Decision 1, or the pinned Elephant checkpoint), keep which door answered through replay and evaluation, and read either door's probabilities | The guards' door rule | `everglade/npc/guard-door.coax` (`gdr_probability`, `gdr_pack`, `gdr_door`, `gdr_identity`); `guard-decision.coax` and `guard-guide.coax` use it |
| Agents: engines, transcripts, tiers | Coax agent | `coax/lib/agent/` |
| A question over a context far bigger than one prompt (a long file, a folder): the model programs over the context in a safe Python subset and calls sub-models on pieces | Recursive Language Models | `coax/lib/rlm/` (read its README) |
| A fact about Coax code for an agent: where a name is declared, who uses it, what a file declares or depends on, a declaration's text, a file's diagnostics as records, a procedure's effects, and an edit that names the hash of what was read | The compiler questions (they run `coax query`, `check` and `deps`; never a second parser, index or grep-and-guess) | `coax/lib/agent/intel/` (read its README) |
| A coding agent whose repository stays out of the prompt: the model reads, searches, edits and runs commands in a workspace through the RLM's environment | RLM code | `coax/lib/rlm/code.coax` (README "RLM code") |
| A coding agent's memory of a run: what it read (so the same lines are never fetched twice), findings with their sources, edits, command runs, the task's requirements, what an edit made stale, and whether the run may finish | The run store and its finish rule | `coax/lib/agent/workmem/` (read its README) |
| Tensors, autodiff, training, inference | Psionic | `psionic/lib/` (read `psionic/README.md` and its book) |
| QR codes, images, media | Coax | `coax/lib/qr/`, `coax/lib/image/`, `coax/lib/media/` |
| A terminal UI | Coax TUI | `coax/lib/tui/` |

## How to check before writing

1. Search for the thing by what it does, not by your name for it:
   `git grep -n -i "redo\|undo" -- '*.coax'`.
2. Read the shared module's header comment and its checks.
3. If two shared modules both seem to fit, use the one in the lower layer
   (`components/`, `metaverse-engine/`, `coax/lib/`) over one inside an app.
4. If you find a hand-rolled copy while working, move it onto the shared
   module in the same change, or file an issue naming both.
