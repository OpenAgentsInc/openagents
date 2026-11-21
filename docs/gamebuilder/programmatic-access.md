# Programmatic access: everything in the UI, from a CLI, skills, MCP and HTTP

Written 2026-11-17. Status: specification. Sections marked **Built** say what
exists today, with paths; everything else is a proposal. Paths under
`vendor/openagents/` are in the pinned openagents tree (`openagents.lock`,
`d12efbd25a`), read-only.

The requirement, in the owner's words: "all of the features that are
accessible in the UI and such also need to be available via skills and MCP
and API."

## Summary

1. **One command table in Coax is the single source.** `coax/commands.coax`
   holds `GB_COMMANDS`: one row per thing a person can do in GameBuilder,
   with its name, typed arguments, result, effect, approval rule, limit,
   help, the surfaces that offer it and the UI control that does it. This
   is the pattern of the OpenAgents CLI's `REALM_COMMANDS`, taken further.
2. **Every surface goes through one call.** The app's buttons, the
   `gamebuilder` CLI, the MCP server, the in-app Agents pane and the HTTP
   API all run a row through `gb_run`, which calls the core's existing
   `#export_c` procedures. No surface has its own copy of a rule.
3. **Every surface answers the same bytes.** A result is the engine's
   `realm.result.v1` shape (`metaverse-engine/editor/op-result.coax`), and
   the level is the engine's `world.state.v1` and `world.edit.v1`. A golden
   case run through the core, the CLI, MCP and HTTP must give the same JSON.
4. **Generated, never hand-copied.** The CLI's help, the MCP tool list and
   schemas, the Claude skill, the capability manifest and the OpenAPI
   document are written from the table. Each has a `--check` that fails when
   its copy and the table differ.
5. **Drift is a test.** A `#test` holds that every row with a UI surface has
   a control, every control names a row, and every `#export_c` is either a
   row's implementation or listed as host plumbing.
6. **Permissions are columns, checked in one place.** Effect and approval
   travel with the row. An agent's edits carry its id, so its turns undo on
   their own. Publishing and anything other people see wait for a person.
7. **Platforms differ only in transport.** The Mac gets everything. Windows
   and Linux get the CLI and MCP first, because those need no UI host. The
   iPhone gets the HTTP API and remote MCP, never a local server. The web
   speaks the HTTP API.

## What exists today

### GameBuilder

**Built.** The core is about 240 `#export_c` procedures across `coax/*.coax`.
They are the true feature list, but each is a hand action for the Swift host:
there is no table, no name for a feature outside Swift, and no way in from
outside the app process.

The one programmatic seam is `coax/agent.coax` (issue 1, step 0, with eight
`#test`s):

| Export | Does |
| --- | --- |
| `ed_agent_scene` | The open level as `gamebuilder.level.v1` JSON: things, the shape table as `content`, `rev`, `locked` |
| `ed_agent_state` | The level as `world.state.v1`, the form the web answers for a world; its document is the level's file |
| `ed_agent_tables` | The level's kinds as the content table `shape` (`aethyr.content.v1`) |
| `ed_agent_apply` | One `world.edit.v1` batch as one undo step, or refused whole: `stale`, `locked`, `playing`, `busy`, `unreadable`, `refused` with the bad edit's index |
| `ed_agent_undo_turn`, `ed_agent_turns` | Take an agent's last turn back while only agents changed the level since; count what can still be taken back |

`coax/palette.coax` (commit `39758c5`) adds the palette and the inspector as
core only, and says in its header that "these are what they, an agent and a
command line all call": `ed_palette_count`, `ed_palette_kind`, `ed_add`,
`ed_add_refusal`, `ed_fields`, `ed_field_name`, `ed_field_unit`,
`ed_field_min`, `ed_field_max`, `ed_field`, `ed_set_field`,
`ed_field_refusal`, `ed_mine_rename`.

Nothing calls the `ed_agent_*` exports yet: no Swift file names them. There
is no `gamebuilder` command, no MCP server, no skill and no HTTP service for
the app. The website's chat (`web/chat.coax`, `web/blocks.coax`) has its own
plan-line grammar (`+ place`, `+ ball`, `+ size`, `+ move`, `+ remove`,
`+ clear`, `+ gravity`), read by `bl_plan`; it is reachable only through the
chat box.

### The pattern in OpenAgents

**Built**, in `vendor/openagents/apps/openagents-cli/`:

- **`commands.coax`** holds `REALM_COMMANDS`, 64 rows of `CliCommand`:
  `domain`, `verb`, `effect`, `approval`, `method`, `route`, `arguments`
  (`|`-separated kinds), `optional`, `flags` (`--name:kind|...`), `usage`,
  `help`, `label`, `many`. Effects are checked against `RC_EFFECTS` (`read`,
  `write`, `publish`, `player-facing`, `spend`, `destructive`); approvals
  against `RC_APPROVALS` (`none`, `owner`, `confirm`). `rc_needs_yes` makes a
  `confirm` row do nothing without `--yes`.
- **The parser and help** use the table: `rc_find`, `rc_validate`,
  `rc_help_line`.
- **`mcp.coax`** is `openagents mcp [--read-only] [--all]`, JSON-RPC over
  stdio. `mcp_tool` builds each tool from a row: name `realm_VERB`
  (`mcp_tool_name`), an input schema from the argument and flag kinds and
  their bounds, and `readOnlyHint` or `destructiveHint` from the effect.
  `MCP_LOADED` lists the 16 rows offered first; meta-tools `realm_find` and
  `realm_call` reach the rest. `mcp_handle` answers `tools/list` and
  `tools/call`; a call becomes CLI arguments (`mcp_argv`) and runs as a child
  `openagents realm VERB ... --json`.
- **`skill.coax`** writes a `SKILL.md` from the rows (`skill_text`); a
  `#test` holds that every row's help and usage are in it.
- **`tool-labels.coax`** writes the web's copy of the rows' tool words
  (`rc_tools_text`) and checks it, so the hosted chat's labels are the
  table's.
- **Checks** in `commands.coax`: unique verbs, every non-read row has an
  approval, every destructive row is `confirm`, labels well formed, the
  generated copy matches.
- **The one result**: `docs/protocol/realm-result-v1.md` and
  `metaverse-engine/editor/op-result.coax` (`OR_SCHEMA` `realm.result.v1`;
  states `done`, `running`, `needs-approval`, `dry-run`, `refused`,
  `unknown`; codes such as `stale-revision`, `edit-refused`,
  `not-permitted`, each with whether a retry can help).
- **The design doc**: `docs/cli/2026-11-01-openagents-cli-v1.md`, "One
  command table".

Around it:

- **The worlds API**: `coax/web/api/worlds.c` (`worlds_route`), with policy
  in `world-policy.coax`. `GET /api/worlds/<id>?lane=dev|live` answers
  `world.state.v1`; `POST /api/worlds/<id>` takes a `world.edit.v1` batch
  and answers 409 when stale, 422 when refused. There are also `/events`,
  `/card`, `/content` and `/assets` routes. Scripts are in `scripts.c`
  (`scripts_route`).
- **The resident service**: `coder host serve` (`coder/tui/host.coax`,
  `host-environment.coax`). Its methods are `DescribeEnvironment`,
  `ListSessions`, `ReadSession`, `OpenSession`, `SendMessage`, `StopTurn`
  and `RespondInteraction`. It listens on an owner-only Unix socket,
  `service.sock`, one JSON record a connection, and over the relay as
  NIP-44 events (`docs/protocol/environment-runtime-v1alpha1.md`).
- **In-process MCP for Claude**: `coax/lib/agent/claude/options.coax`
  declares `ClTool` and `ClServer`, registered with `cl_add_server` and
  `cl_add_server_tool`. `mcp.coax` answers `initialize` and `tools/list` in
  the library and resolves a call by name (`cl_server_tool`). The host
  answers with `cl_tool_answer` or `cl_tool_text` (`query.coax`).
- **A second table in the same style**: `metaverse-engine/script/api-table.coax`
  (`SCRIPT_API`), the one source for what a realm script may call, from which
  `docs/protocol/realm-script-api.md` is generated.

**What we take, and what we change.**

| OpenAgents does | GameBuilder does |
| --- | --- |
| Effects and approvals are strings checked against a list | Typed enums; the strings are only their names |
| An MCP call runs the CLI as a child process | MCP, the CLI and the pane call `gb_run` in the same process |
| A test compares routes with a hardcoded list | The server's route table is generated from the rows, and a test compares the two |
| No OpenAPI or capability manifest today (the old `docs/api/openapi.first-party.json` is gone) | Both are generated from the rows and served |
| Each in-process tool is dispatched by its host branching on the name | The pane's tool index maps to a row index, set when the tool is registered |
| `openagents commands --markdown` is described but not found | Not promised until it exists |

## The inventory

Grouped as the person meets them. "Core" is the export the feature uses.
"Today" says whether anything outside the app's own UI can reach it.

### Levels

| Feature | UI | Core | Today |
| --- | --- | --- | --- |
| List levels | Level menu | `ed_level_number`, `ed_level_title`; the host lists files (`Levels.numbers`) | None |
| Open a level | Level menu | `ed_open` (the host reads the file) | None |
| New level | ⌘N, New Level | `ed_level_next`, `ed_new` | None |
| Save | Automatic | `ed_save_due`, `ed_save`, `ed_saved` | None |
| Open a game's own level | Level menu, the game's row | `ed_open_game` | None |
| Make my own copy | Button on a locked level | `ed_copy` | None |
| The level as data | None | `ed_agent_scene`, `ed_agent_state`, `ed_agent_tables` | **In-process export only** |

### Games

| Feature | UI | Core | Today |
| --- | --- | --- | --- |
| The games that ship | Level menu | `ed_games`, `ed_game_title`, `ed_game_note`, `ed_game_id` | None |
| Your games | Level menu | `ed_mines`, `ed_mine_at`, `ed_mine_title`, `ed_mine_template`, `ed_mine_level`, `ed_mine_load`, `ed_mine_save` | None |
| New game of a game type | New Game's picker (`coax/picker.coax`, the same cards as gamebuilder.org's) | `ed_templates`, `ed_template_name`, `ed_new_game` | `list-templates`, `new-game` |
| Rename your game | The side panel's Game section (`panel.rename-game`) | `ed_mine_rename` | The `rename-game` row, every surface |
| Delete your game | The side panel's Game section, pressed twice (`panel.delete-game`) | `mine_drop`; the host deletes the file (`ed_drop_game_due`) | The `delete-game` row, every surface: its levels stay, in no game |
| Delete your level | The side panel's Level section, pressed twice (`panel.delete-level`) | `mine_drop_level`; the host deletes the file and its claims (`ed_drop_due`) and opens your first level, or a new one | The `delete-level` row, every surface: a game keeps one level at least |
| Name a level in its game | The side panel's Level section (`panel.rename-level`) | `mine_name_level`, kept in the game's file | The `rename-level` row, every surface; `list-levels` gives the name |
| Put a level into a game, or out | The side panel's Level section: Into GAME, Out of the game (`panel.move-level`) | `mine_move_level`: both games' files are written (`ed_game_due`) | The `move-level` row, every surface: 16 levels a game (MINE_LEVELS), a game of the level's own type |
| A game's owners | The side panel's Game section: Owners, Set owners (`panel.set-owners`) | `mine_owner`, `mine_set_owners`; `ed_may_edit` lets only an owner change it | `owners` on every surface but the app (it draws them); `set-owners` is the person's: the app, the CLI and the skill, only when logged in, with their own account among the 1 to 4 (OWNERS_MAX) |
| Who may change it | The locked banner | `ed_may_edit`, `ed_is_official`, `ed_template`, `ed_explores` | None |

### Editing

| Feature | UI | Core | Today |
| --- | --- | --- | --- |
| Select | Click, tap | `ed_down`/`ed_up` (picking); `ed_has_selection` | None. There is no select-by-id export |
| Tools: select, move, rotate, scale | Q W E R, toolbar | `ed_set_tool`, `ed_tool` | None |
| Move, rotate, scale a thing | Drag on the selection | The shared viewport's drag, ending as one `WorldEdit` | `ed_agent_apply` (move, rotate, scale) |
| Add a block | Toolbar | `ed_add_block` | `ed_agent_apply` (spawn) |
| Add from the palette | Not drawn yet | `ed_palette_count`, `ed_palette_kind`, `ed_add`, `ed_add_refusal`, `ed_kind_name` | Core only |
| Inspector fields | Not drawn yet | `ed_fields`, `ed_field_*`, `ed_field`, `ed_set_field`, `ed_field_refusal` | Core only |
| Delete | Delete, toolbar | `ed_delete` | `ed_agent_apply` (despawn) |
| Undo, redo | ⌘Z, ⇧⌘Z, toolbar | `ed_undo`, `ed_redo`, `ed_can_undo`, `ed_can_redo` | None |
| Copy, cut, paste, duplicate | ⌘C ⌘X ⌘V ⌘D, toolbar | `ed_clip_copy`, `ed_clip_cut`, `ed_clip_paste`, `ed_clip_duplicate`, `ed_clip_has` | None |
| Focus | F, toolbar | `ed_focus` | None |
| Camera: orbit, pan, dolly, fly | Mouse, keys, touch | `ed_down`, `ed_move`, `ed_up`, `ed_wheel`, `ed_key`, `ed_touch_pan`, `ed_touch_pinch` | None (input, not a command) |
| An agent's turn taken back | Not drawn yet | `ed_agent_undo_turn`, `ed_agent_turns` | **In-process export only** |

### Decide

| Feature | UI | Core | Today |
| --- | --- | --- | --- |
| ✦ The model makes the next edit | Toolbar | `ed_decide_body`, `ed_decide_url`, `ed_decide_sign`, `ed_decide_answer`, `ed_asking`, `ed_note`; `Decide.swift` sends the request | None |

### Play

| Feature | UI | Core | Today |
| --- | --- | --- | --- |
| Pick a mode and a size | PLAY, the picker | `ed_play_modes`, `ed_play_mode_name`, `ed_play_mode_blurb`, `ed_play_size_most`, `ed_play_setup`, `ed_play_mode`, `ed_play_size` | `list-modes`, and `play`'s `--mode` and `--size` |
| Pick a Scrapper and play | The picker (`play.play`) | `ed_play_classes`, `ed_play_class_name`, `ed_play_class_stat`, `ed_play_class_blurb`, `ed_play` | `list-scrappers`, `play` |
| Stop | Esc, the end screen (`play.stop`) | `ed_stop`, `ed_playing` | `stop`, `play-state` |
| Move, look, fire, jump, super, view, use | Sticks, buttons, keys, mouse | `ed_play_touch`, `ed_play_look`, `ed_play_fire`, `ed_play_view`, `ed_play_use`, `ed_key` | None (input) |
| The HUD, the feed, the events | Drawn | `ed_play_hud`, `ed_play_feed`, `ed_play_events` | None |
| SPLICEBOTS: Digger, Travel, the bag | G, T, 1 to 9 (`play.digger`, `play.travel`, `play.bag`) | `ed_digger_toggle`, `ed_farm_travel`, `ed_bag_select`, `ed_bag_name`, `ed_bag_count`, `ed_bag_selected`, `ed_scene_said` | `digger`, `travel`, `bag-select` |
| A match left to the bots | None | `play_test_go`, `play_test_run` (test helpers in `coax/play.coax`) | `#test` only: "the bots fight: a match left alone scores, stays inside the walls and ends" |

### The lobby and the party

| Feature | UI | Core | Today |
| --- | --- | --- | --- |
| Your lobby name | Lobby screen | `ed_lobby_name`, `ed_lobby_my_name` | `lobby-name`, `lobby-read` (the app's lobby sheet only; the person's alone) |
| Who is here and what they are doing | Lobby screen | `ed_lobby_online`, `ed_lobby_player_name`, `ed_lobby_player_game`, `ed_lobby_player_doing`, `ed_lobby_player_me` | `lobby-read` (the app's lobby sheet only; the person's alone) |
| Read and send chat | Lobby screen | `ed_lobby_messages`, `ed_lobby_message_*`, `ed_lobby_can_say`, `ed_lobby_say`, `ed_lobby_why` | `lobby-read`, `lobby-say` (the app's lobby sheet only; the person's alone) |
| Mute, report | A player's row | `ed_lobby_mute`, `ed_lobby_muted_player`, `ed_lobby_report` | `lobby-mute`, `lobby-report` (the app's lobby sheet only; the person's alone) |
| Invite, answer, leave | Lobby, the party card | `ed_party_invite`, `ed_party_invited`, `ed_party_answer`, `ed_party_leave` | `party-invite`, `party-answer`, `party-leave` (the app's lobby sheet only; the person's alone) |
| The party: who, sides, Scrappers, mode and size | The party card | `ed_party_on`, `ed_party_leads`, `ed_party_count`, `ed_party_full`, `ed_party_member_*`, `ed_party_has`, `ed_party_set_side`, `ed_party_set_kind`, `ed_party_kind`, `ed_party_set_match`, `ed_party_mode`, `ed_party_size`, `ed_party_fits` | `party-read`, `party-side`, `party-scrapper`, `party-match` (the app's lobby sheet only; the person's alone) |
| Quick match, start | The party card | `ed_party_look`, `ed_party_looking`, `ed_party_start`, `ed_party_pending`, `ed_party_playing` | `party-look`, `party-start` (the app's lobby sheet only; the person's alone) |

### The account

| Feature | UI | Core | Today |
| --- | --- | --- | --- |
| Log in with an email code | Account sheet | `Account.swift` over openagents' `AppAuth.swift`; then `ed_set_account` | None |
| Who is logged in | Account button | `ed_account` | None |
| Link this device's Nostr key | At login | `Account.linkKey`, `ed_lobby_link_proof`, `POST /api/auth/link/nostr` | None |
| Log out | Account sheet | `Account.logOut` | None |

### The lobby and the party, the person's own

The lobby's and the party's rows (`lobby-read`, `lobby-name`, `lobby-say`,
`lobby-mute`, `lobby-report`, `party-read`, `party-invite`, `party-answer`,
`party-leave`, `party-side`, `party-scrapper`, `party-match`, `party-look`,
`party-start`) are the app's lobby sheet's, in `coax/lobby-rows.coax`. The
lobby lives in the running app, signed with the person's key on the room's
relay, so these rows are on the app alone (gap `lobby`). Every one that
speaks, names, mutes, reports, invites, answers or plays with others has the
approval `person`: `ed_lobby_command` takes whether the call is the person's
own press on the sheet, and refuses it to anyone else (`not-permitted`). An
attached program is not offered them, and `gb_run` refuses the whole area, so
no agent speaks or invites for the person. A `#test` in the table holds that
in the lobby, every row but a read is the person's alone.

### Agents, publishing, and what is planned

| Feature | Where specified | Today |
| --- | --- | --- |
| The desktop Agents pane: start, talk, watch, undo, stop | Issues 1 and 4; `docs/agents.md` | Built (`coax/agents.coax` on the agent rows; each agent a record and log beside the levels) |
| The agents page, read only: your agents by game, and an agent's events | Issue 4; `docs/agents.md` | Built (`gamebuilder.org/agents`, on the hosted rows `agents` and `agent-events`; the app reports in through `coax/sync.coax`) |
| Starting, stopping and messaging from the page, computers and pairing (`/api/agents`, `/api/computers`) | `docs/agents.md` | Not built |
| Publish, versions, roll back, unpublish, delete, feedback, block, report | `docs/publishing.md` (its API section) | Not built |
| Markers | `docs/template-gap-analysis.md` §2 | Core built (`palette.coax`), not drawn |
| Inspector | §3 | Core built, not drawn |
| Tables for rules and tuning | §4 | Not built; cell edits are already `world.edit.v1` (`WE_CELL`) |
| Assets, characters, sounds | §5 | Not built |
| Behaviour as realm scripts | §7 | Not built; the engine's `SCRIPT_API` exists |
| Several maps in one game | §8 | Not built |

### gamebuilder.org

| Feature | UI | Today |
| --- | --- | --- |
| Log in | `/login` (`web/login.coax`) | None outside the page |
| Pick a game type (nine) | The picker (`web/page.coax` `gs_picker`, from `shared/game-types.coax`) | None |
| Shapes, placing modes, edit mode, camera, world settings | The hotbar (`docs/web-world.md`) | Through the chat's plan lines only |
| Chat and its router | The chat box (`web/chat.coax`) | None outside the page |
| Decide | Hotbar slot | Through the chat (the message `decide`) |
| Crates: reset | R | None |

### What is not a feature

Most exports are host plumbing: they hand the host memory sizes, vertices,
matrices, sounds or transport frames. They get no row; they go on a
`GB_PLUMBING` list in `coax/commands.coax` so the coverage test can tell
them apart. They are: `ed_init`, `ed_resize`, `ed_tick`, `ed_take_dirty`,
`ed_camera`, `ed_camera_floats`, `ed_draw`, `ed_instance_capacity`,
`ed_instance_floats`, `ed_cube`, `ed_cube_vertices`, `ed_sky`, the
`ed_farm_*` draws and light, `ed_model_*`, `ed_rave_*`, `ed_mesh*` and
`ed_meshes_open`, `ed_play_hud_floats`, `ed_play_sound*`,
`ed_play_captures`, `ed_save_max`, `ed_mine_doc_max`, `ed_decide_*_max`,
`ed_lobby_init`, `ed_lobby_pubkey`, `ed_lobby_relay`, `ed_lobby_retry`,
`ed_lobby_opened`, `ed_lobby_closed`, `ed_lobby_input`, `ed_lobby_tick`,
`ed_lobby_take`, `ed_lobby_take_changed`, `ed_lobby_account`,
`ed_lobby_activity`, `ed_lobby_connected`, `ed_party_begin`,
`ed_party_take_bye`, `ed_party_wire`, `ed_party_pump`, `ed_party_heard`,
and every `ed_net_*`.

## The command table

### Where it lives

`coax/commands.coax`, in this repository. The table is product, not engine:
it names GameBuilder's features. It loads the files whose exports it calls
and nothing loads it except the surfaces.

### A row

```coax
GbCommand :: struct {
  verb: string;          // "apply-edits": the one name, on every surface
  area: GbArea;          // level, game, edit, decide, play, lobby, party, account, agent, publish
  effect: GbEffect;      // below
  approval: GbApproval;  // below
  arguments: string;     // kinds, "|"-separated, as REALM_COMMANDS: "level|thing|field|value"
  optional: int;         // how many of the last arguments may be left out
  flags: string;         // "--rev:rev|--dry-run:bool|--key:key"
  result: string;        // the schema it answers: "realm.result.v1", "world.state.v1", ...
  limit: string;         // what bounds it, naming the constant: "64 edits a batch (WE_BATCH)"
  impl: string;          // the exports it calls: "ed_agent_apply"
  surfaces: u32;         // GB_UI | GB_CLI | GB_MCP | GB_SKILL | GB_HTTP | GB_PANE | GB_WEB | GB_CLOUD
  route: string;         // "POST /api/levels/:level", or "" when no HTTP route
  ui: string;            // the control that does it: "toolbar.undo", or "" when none
  usage: string; help: string; label: string; many: string;   // as REALM_COMMANDS
}
GB_COMMANDS :: [N] GbCommand.[ ... ];
GbVerb :: enum { read_level, read_state, apply_edits, ... }   // one per row, in order
```

A `#test` holds that `GbVerb`'s order is the table's, so a verb is a typed
value and a row's index at once.

### Effects

| Effect | Means | OpenAgents' name |
| --- | --- | --- |
| `read` | Changes nothing | `read` |
| `edit` | Changes a level or a game of yours. Undo takes it back | `write` |
| `view` | Changes only what this screen shows: the tool, the camera, the selection | (none) |
| `play` | Starts, stops or drives a match. Changes no level | (none) |
| `social` | Other people see or hear it: the lobby, the party | `player-facing` |
| `account` | Changes who is logged in, or a key linked to the account | (none) |
| `publish` | Changes what players at a link get | `publish` |
| `spend` | A model or a machine is paid for: Decide, a cloud agent | `spend` |
| `destructive` | It can't be undone | `destructive` |

### Approvals

| Approval | Means | Who checks |
| --- | --- | --- |
| `none` | Anyone who can reach it | |
| `owner` | Only on a level or game this account may change (`ed_may_edit`). Otherwise `not-permitted`, and the answer names `copy-level` | The core; for HTTP, the server checks the account itself |
| `confirm` | Two steps. Without `--yes` (CLI), `"confirm": true` (MCP, HTTP) or a tap (UI), it does nothing and answers `needs-approval` | `gb_run` |
| `person` | Only a person at a GameBuilder screen. A call from an agent or a script answers `needs-approval`, and the app asks the person (the pane's ask, or a sheet) | `gb_run`, by the caller's kind |

Rules a `#test` holds, as `commands.coax` does in OpenAgents: every row that
is not `read` or `view` has an approval other than `none`; every
`destructive` row is `confirm` or `person`; every `publish`, `social` and
`account` row is `person` unless the owner turns on "may publish" or "may
speak" for one agent (below).

### Argument kinds

Each kind has bounds from the core's own constants, so a schema, a parser
and a refusal say the same number.

| Kind | Bounds | From |
| --- | --- | --- |
| `level` | 1 to 9,999 | `LEVEL_MAX` |
| `game` | 0 to `GAMES` − 1 | `coax/games.coax` |
| `mine` | a game of yours, at most 32 on a device, 16 levels each | `MINE_MAX`, `MINE_LEVELS` |
| `template` | 0 to `TEMPLATES` − 1: a game type's id (`list-templates` names all twelve) | `GT_ID` (`shared/game-types.coax`) |
| `thing` | an entity id in the open level | the level |
| `kind` | 0 to `KINDS` − 1, and in this game type's palette; a website type's place, sizes and count are held to its limits (`gt_place_why`, `GT_SIDE_*`, `ACTORS_MAX`), refused in gamebuilder.org's words with their numbers | `GK_WORD` (`shared/kinds.coax`), `gt_palette` |
| `field` | 0 to `FIELDS` − 1, by name (`Kind`, `East`, `Up`, …) | `FIELD_NAME` |
| `value` | the field's `FIELD_MIN` to `FIELD_MAX` | `palette.coax` |
| `rev` | a revision as `ed_agent_scene` gives it | `coax/agent.coax` |
| `batch` | one `world.edit.v1` batch, at most 64 edits | `WE_BATCH` |
| `mode`, `size` | `ed_play_modes`, 1 to `ed_play_size_most` | `coax/modes.coax` |
| `scrapper` | 0 to `ed_play_classes` − 1 | `coax/play.coax` |
| `text:N` | at most N bytes (lobby chat 280, a game's name 48) | `LB_TEXT`, `MINE_TITLE` |
| `player` | a lobby player's public key, not a list index | new: the lobby exports take an index today |
| `agent` | an agent's id | `ed_agent_apply`'s `agent` |

Names come in as JSON (MCP, HTTP, the pane) or argv (CLI). Both are read by
one reader, `gb_args_read(row, ...)`, into a typed `GbArgs`. A command's name
is matched exactly against the `verb` column (`gb_find`), the same bounded
lookup `rc_find` does. Nothing here reads a sentence: a person's words go to
a model (the pane's Claude, the website's router, Decide), and the model
picks a row by a typed tool call.

### Some rows

Proposed, in the table's own form.

```coax
GbCommand.{verb="read-level", area=.level, effect=.read, approval=.none, arguments="", flags="--level:level",
  result="gamebuilder.level.v1", limit="", impl="ed_agent_scene", surfaces=GB_ALL & ~GB_UI,
  route="GET /api/levels/:level/view", ui="",
  usage="[--level N]", help="the level as data: its things, its shape table, its revision and whether it is yours to change",
  label="Read the level", many="Read levels"},
GbCommand.{verb="apply-edits", area=.edit, effect=.edit, approval=.owner, arguments="batch",
  flags="--rev:rev|--dry-run:bool|--key:key", result="realm.result.v1",
  limit="64 edits a batch (WE_BATCH); 512 things a level (ACTORS_MAX); 200 m from the middle (AGENT_REACH)",
  impl="ed_agent_apply", surfaces=GB_ALL, route="POST /api/levels/:level", ui="gesture.drag",
  usage="BATCH.json [--rev N]", help="carry out a world.edit.v1 batch as one undo step, or refuse it whole",
  label="Change {count} things", many="Change the level"},
GbCommand.{verb="undo", area=.edit, effect=.edit, approval=.owner, arguments="", flags="",
  result="realm.result.v1", limit="128 steps (HISTORY_STEPS)", impl="ed_undo", surfaces=GB_ALL,
  route="POST /api/levels/:level/undo", ui="toolbar.undo|key.cmd-z", usage="", help="take back the last edit",
  label="Undo", many="Undo"},
GbCommand.{verb="set-field", area=.edit, effect=.edit, approval=.owner, arguments="thing|field|value", flags="--rev:rev",
  result="realm.result.v1", limit="the field's range (FIELD_MIN, FIELD_MAX)", impl="ed_set_field|ed_field_refusal",
  surfaces=GB_ALL, route="POST /api/levels/:level/things/:thing", ui="inspector.field",
  usage="THING FIELD VALUE", help="set one of a thing's fields (kind, place, turn, scale, size) as one edit",
  label="Set {field} to {value}", many="Set fields"},
GbCommand.{verb="copy-level", area=.level, effect=.edit, approval=.none, arguments="", flags="",
  result="realm.result.v1", limit="9,999 levels (LEVEL_MAX)", impl="ed_copy", surfaces=GB_ALL,
  route="POST /api/levels/:level/copy", ui="banner.copy", usage="", help="make your own copy of a level you may not change",
  label="Make my own copy", many="Copy levels"},
GbCommand.{verb="play-test", area=.play, effect=.read, approval=.none, arguments="mode|size", optional=2,
  flags="--seconds:seconds|--seed:seed", result="gamebuilder.playtest.v1", limit="600 seconds of match time",
  impl="ed_play_setup|ed_play|ed_tick", surfaces=GB_CLI|GB_MCP|GB_SKILL|GB_PANE|GB_HTTP, route="POST /api/levels/:level/playtest",
  ui="", usage="[MODE SIZE] [--seconds S]", help="play the level headless with bots on both sides and report how it went",
  label="Play-test", many="Play-test"},
GbCommand.{verb="lobby-say", area=.lobby, effect=.social, approval=.person, arguments="text:280", flags="",
  result="realm.result.v1", limit="280 bytes (LB_TEXT); one a second (LB_SAY_EVERY)", impl="ed_lobby_say|ed_lobby_why",
  surfaces=GB_UI|GB_CLI, route="", ui="lobby.send", usage="TEXT", help="say something in the lobby, signed with your key",
  label="Say in the lobby", many="Lobby chat"},
GbCommand.{verb="publish", area=.publish, effect=.publish, approval=.person, arguments="", flags="--yes:bool",
  result="realm.result.v1", limit="30 publishes an hour; 256 KB a level", impl="ed_save", surfaces=GB_ALL & ~GB_PANE,
  route="POST /api/games/:game/versions", ui="share.publish", usage="[--yes]",
  help="publish the level as a new version at its link", label="Publish", many="Publish"},
```

The sketch above is the first draft. The `publish` row as built (`coax/commands.coax`, area `publish`) takes `LEVEL NAME`, is the person's own (approval `person`, on the app, the CLI and the skill), and holds `docs/publishing.md`'s first limits in code (`coax/publish.coax`: 256 KB a level, 30 publishes an hour).
`gamebuilder.playtest.v1` is new (see "Determinism and tests").

### The full list of rows

Every row of the table, where it is, and the reasons it is not on the other
surfaces (`GB_GAPS`, above). This table is made from `coax/commands.coax` by
`tools/docs-rows.coax`, and `scripts/check.sh` fails when it is not what the
table makes. Not rows yet: `select` and `set-camera` for the app's level, which
need exports first.

<!-- rows: inventory -->
| Area | Row | Surfaces | Not on the others because |
| --- | --- | --- | --- |
| level | `list-levels` | cli, mcp, skill, http | `drawn`, `open-level` |
| level | `read-level` | cli, mcp, skill, http, pane, cloud | `drawn` |
| level | `read-state` | cli, mcp, skill, http | `drawn`, `pane-reads` |
| level | `read-content` | cli, mcp, skill, http | `drawn`, `pane-reads` |
| level | `new-level` | ui, cli, mcp, skill, http | `open-level` |
| level | `copy-level` | ui, cli, mcp, skill, http | `open-level` |
| level | `write-content` | cli, mcp, skill, http | `auto`, `open-level` |
| level | `open-level` | ui | `screen`, `view` |
| level | `delete-level` | ui, cli, mcp, skill, http | `open-level` |
| level | `rename-level` | ui, cli, mcp, skill, http | `open-level` |
| level | `move-level` | ui, cli, mcp, skill, http | `open-level` |
| game | `list-games` | cli, mcp, skill, http, web | `drawn`, `open-level` |
| game | `list-templates` | cli, mcp, skill, http | `drawn`, `open-level` |
| game | `new-game` | ui, cli, mcp, skill, http | `open-level` |
| game | `rename-game` | ui, cli, mcp, skill, http | `open-level` |
| game | `read-game` | cli, mcp, skill, http | `drawn`, `open-level` |
| game | `open-game` | ui | `screen`, `view` |
| game | `delete-game` | ui, cli, mcp, skill, http | `open-level` |
| game | `owners` | cli, mcp, skill, http | `drawn`, `open-level` |
| game | `set-owners` | ui, cli, skill | `person`, `open-level` |
| edit | `apply-edits` | cli, mcp, skill, http, pane, cloud | `gesture` |
| edit | `add` | ui, cli, mcp, skill, http | `batch` |
| edit | `set-field` | ui, cli, mcp, skill, http | `batch` |
| edit | `delete` | ui, cli, mcp, skill, http | `batch` |
| edit | `list-palette` | cli, mcp, skill, http, pane, cloud | `drawn` |
| edit | `list-fields` | cli, mcp, skill, http, pane, cloud | `drawn` |
| edit | `undo` | ui | `screen`, `view` |
| edit | `redo` | ui | `screen`, `view` |
| edit | `copy` | ui | `screen`, `view` |
| edit | `paste` | ui | `screen`, `batch` |
| edit | `duplicate` | ui | `screen`, `batch` |
| edit | `cut` | ui | `screen`, `batch` |
| edit | `undo-turn` | pane | `pane` |
| edit | `ask` | pane | `pane`, `live` |
| edit | `claim` | cli, mcp, skill, http, pane, cloud | `claim` |
| edit | `claims` | cli, mcp, skill, http, pane, cloud | `drawn` |
| edit | `release` | cli, mcp, skill, http, pane, cloud | `claim` |
| play | `list-modes` | cli, mcp, skill, http, pane, cloud | `drawn` |
| play | `play-test` | cli, mcp, skill, http, pane | `headless` |
| play | `play` | ui | `screen`, `match` |
| play | `stop` | ui | `screen`, `match` |
| play | `play-state` | ui | `screen`, `match` |
| play | `list-scrappers` | cli, mcp, skill, http, pane, cloud | `drawn` |
| play | `travel` | ui | `screen`, `match` |
| play | `digger` | ui | `screen`, `match` |
| play | `bag-select` | ui | `screen`, `match` |
| account | `whoami` | cli, skill | `login`, `open-level` |
| account | `login` | cli, skill | `login`, `person` |
| account | `logout` | cli, skill | `login`, `open-level` |
| engine | `engine` | cli, mcp, skill, http | `drawn`, `open-level` |
| engine | `engine-list` | cli, mcp, skill, http | `drawn`, `open-level` |
| engine | `engine-read` | cli, mcp, skill, http | `drawn`, `open-level` |
| engine | `triage` | cli, skill | `door`, `live` |
| engine | `engine-ready` | cli, skill | `computer`, `live` |
| engine | `engine-mine` | cli, skill | `computer`, `live` |
| view | `set-tool` | ui | `screen`, `view` |
| view | `focus` | ui | `screen`, `view` |
| view | `agents-pane` | ui | `screen`, `view` |
| view | `changes` | cli, mcp, skill, http | `drawn`, `pane-reads` |
| view | `show-changes` | ui, cli, mcp, skill | `screen`, `view` |
| view | `dismiss-changes` | ui, cli, mcp, skill | `screen`, `view` |
| view | `agents-account` | ui | `screen`, `view` |
| view | `pictures` | cli, mcp, skill, http | `drawn`, `pane-reads` |
| view | `picture` | ui, cli, mcp, skill, http | `pane-reads` |
| decide | `decide` | ui | `spend`, `live` |
| agent | `agents` | cli, mcp, skill, http, web | `drawn`, `agents` |
| agent | `agent-read` | cli, mcp, skill, http, web | `drawn`, `agents` |
| agent | `agent-events` | cli, mcp, skill, http, web | `drawn`, `agents` |
| agent | `agent-new` | ui, cli, mcp, skill, http, web | `agents` |
| agent | `agent-message` | ui, cli, mcp, skill, http, web | `agents` |
| agent | `agent-answer` | ui, cli, mcp, skill, http | `agents` |
| agent | `agent-stop` | ui, cli, mcp, skill, http, web | `agents` |
| agent | `agent-undo` | ui, cli, mcp, skill, http, web, cloud | `agents` |
| agent | `agent-report` | cli, mcp, skill, http | `runner`, `agents` |
| agent | `agent-picture` | cli, mcp, skill, http | `runner`, `agents` |
| agent | `computers` | cli, mcp, skill, http, web | `drawn`, `agents` |
| agent | `computer-report` | cli, mcp, skill, http | `runner`, `agents` |
| agent | `agent-new-on` | ui, cli, mcp, skill, http, web | `agents` |
| code | `code-brief` | cli, mcp, skill, http | `drawn`, `open-level` |
| code | `code-agent` | ui | `screen`, `view` |
| lobby | `lobby-read` | ui | `lobby`, `live` |
| lobby | `lobby-name` | ui | `lobby`, `live` |
| lobby | `lobby-say` | ui | `lobby`, `live` |
| lobby | `lobby-mute` | ui | `lobby`, `live` |
| lobby | `lobby-report` | ui | `lobby`, `live` |
| lobby | `party-read` | ui | `lobby`, `live` |
| lobby | `party-invite` | ui | `lobby`, `live` |
| lobby | `party-answer` | ui | `lobby`, `live` |
| lobby | `party-leave` | ui | `lobby`, `live` |
| lobby | `party-side` | ui | `lobby`, `live` |
| lobby | `party-scrapper` | ui | `lobby`, `live` |
| lobby | `party-match` | ui | `lobby`, `live` |
| lobby | `party-look` | ui | `lobby`, `live` |
| lobby | `party-start` | ui | `lobby`, `live` |
| web | `place` | web | `web` |
| web | `ball` | web | `web` |
| web | `put` | web | `web` |
| web | `turn` | web | `web` |
| web | `kind` | web | `web` |
| web | `size` | web | `web` |
| web | `move` | web | `web` |
| web | `remove` | web | `web` |
| web | `clear` | web | `web` |
| web | `set` | web | `web` |
| web | `reset` | web | `web` |
| web | `gravity` | web | `web` |
| web | `select` | web | `web` |
| web | `spin` | web | `web` |
| web | `goes` | web | `web` |
| site | `funnel` | cli, mcp, skill, http | `site` |
| publish | `publish` | ui, cli, skill, http | `person`, `open-level` |
| publish | `published` | cli, mcp, skill, http | `drawn`, `open-level` |
| publish | `versions` | cli, mcp, skill, http | `drawn`, `open-level` |
| publish | `set-live` | ui, cli, skill, http | `person`, `open-level` |
| publish | `unpublish` | ui, cli, skill, http | `person`, `open-level` |
| publish | `remove-published` | ui, cli, skill, http | `person`, `open-level` |
| publish | `feedback` | cli, mcp, skill, http | `drawn`, `open-level` |
| publish | `feedback-hide` | ui, cli, skill | `person`, `open-level` |
| publish | `block` | ui, cli, skill | `person`, `open-level` |
| publish | `report` | ui, cli, skill | `person`, `open-level` |
<!-- /rows -->

Continuous input is not a row: orbit, pan, dolly, fly, a drag in progress,
the play sticks and the mouse look. Each control of that kind names the row
its result equals (a move drag is `apply-edits`, a pick is `select`), and a
program uses that row. A playtest agent that must steer a Scrapper uses the
engine's driver (`metaverse-engine/agent/drive.coax`), a later row.

### One call

```coax
GbCaller :: struct { kind: GbCallerKind; agent: s32; }   // person, pane, cli, mcp, http
gb_run :: (e: [] Editor, l: [] Lobby, who: GbCaller, verb: GbVerb, a: GbArgs, out: [] u8) -> GbResult;
ed_command :: (e: [] Editor, l: [] Lobby, who: s32, agent: s32, verb: s32, args: [] u8, out: [] u8) -> s64 #export_c;
```

`gb_run` checks the approval against the caller, checks `--dry-run` and
`--key` as `realm.result.v1` defines them, calls the row's exports, and
writes the result. `ed_command` is the one export the host and every surface
call. `gb_run` is a `switch` on `GbVerb`; no surface switches on a string.

Refusals keep the core's words and add the code and, where a limit refused
it, the number (`AGENTS.md`: a limit is said in words, with its number).
`ed_agent_apply`'s errors map to codes: `stale` to `stale-revision`;
`locked` to `not-permitted`; `refused` and `unreadable` to `edit-refused`
and `bad-request`; `playing` and `busy` to a retry-able
`operation-failed` whose words say a match or a drag is going on.

Whether the result is written as `realm.result.v1` itself, or a
`gamebuilder.result.v1` profile of it, is open question 1.

### The UI goes through it

The app's buttons are rows too. The controls the host draws are a Coax
table, `GB_CONTROLS`: an id (`toolbar.undo`), the row, the kind (button,
key, menu item, gesture), its icon and its words. New interface is drawn in
Coax already (the pane, the palette, the inspector), so their controls come
from this table directly. The SwiftUI toolbar in `host/Sources/App.swift`
reads it through `ed_controls`, `ed_control_verb`, `ed_control_name` and
`ed_control_icon`, and calls `ed_command`, until it is redrawn in Coax. A
button can then only exist with a row behind it.

### Drift checks

| Check | Where | Holds |
| --- | --- | --- |
| The table is well formed | `#test` in `coax/commands.coax` | Unique verbs; every kind and flag known; every bound from a constant; approvals as above; `GbVerb`'s order |
| UI and rows agree | `#test` | Every row with `GB_UI` has a control; every control names a row |
| Exports are covered | `tools/commands-check.coax`, run by `scripts/check.sh` | Every `ed_` export in `coax/*.coax` is in a row's `impl` or in `GB_PLUMBING`, not both; every name in `impl` exists |
| The docs' row tables are the table's | `tools/docs-rows.coax --check docs/cli.md docs/programmatic-access.md`, run by `scripts/check.sh` | The text between each file's `rows` comment markers (`cli` in docs/cli.md, `inventory` here) is what the table makes; run the tool without `--check` to make it again |
| The website's plan lines are rows | `tools/web-rows-check.coax web/blocks.coax web/chat.coax`, run by `scripts/check.sh` | Each word `bl_plan` and `bl_edit` read is a row in the `web` area (only `GB_WEB`, gap `web`), and each such row is a word they read; each row has a `#test` in `web/blocks.coax` that runs its line; each `+ VERB USAGE` line the chat's model is told is a row's, its words the row's `usage` exactly |
| The hosts go through the rows | `tools/commands-check.coax --host host/Sources/*.swift host/linux/main.c host/panel/*`, run by `scripts/check.sh` | A host names only the ways in (`ed_command`, `ed_lobby_command`, the controls), `GB_PLUMBING` (drawing, input, a control's state), a read row's `impl`, and the `impl` of a row the host carries out (`GB_HOST_DECIDE`, `GB_HOST_NEW_GAME`, `GB_HOST_FILE`); an action row's export called past its row fails |
| MCP agrees | `#test` | Each tool's schema is the row's kinds and bounds; the hints are the effect's |
| The skill agrees | `#test` | The skill text names every row with `GB_SKILL`, its usage and its help |
| The manifest and OpenAPI agree | `tools/commands.coax --check` | The committed or served copies are the table's |
| The server agrees | `#test` in `server/` | The route table is made from the rows; a route the rows lack is a failure |
| Same answers | The golden cases below | Every row has a case (unless its gap says `live`), and each case gives the same JSON on every surface its row has (`tools/commands-golden.coax`, run by `cli/check.sh`) |
| The cloud Builder's rows are the marked ones | `#test` "the rows a cloud Builder is offered are the rows the table marks for it" in `coax/cloud-rows.coax` | For every row: it is named in the Builder's environment, and a command that starts with its name reaches it, exactly when the table marks it `GB_CLOUD`; a marked row runs on the account's files and shows no `--agent`, `--key` or file |
| Every surface, or a reason | `#test` "every row is on every surface, or says why not" in `coax/commands.coax` | A row is on the app (`GB_UI`), the CLI, MCP, the skill, HTTP and the Agents pane unless its `gap` column names a reason from `GB_GAPS` that excuses each one it lacks; a reason must be true of the row and excuse a surface it lacks |

### Surfaces and their gaps

Every row is on every surface: the app's controls, the CLI, MCP, the skill,
HTTP and the Agents pane. A row that is not on one says why in its `gap`
column, in words from one closed list, `GB_GAPS` in `coax/commands.coax`:

| Reason | Excuses | Means |
| --- | --- | --- |
| `screen` | CLI, MCP, skill, HTTP | It acts on what the app's screen holds (the selection, the clipboard, the tool, the history, the panes); a program reaches it in the running app with `--attach` |
| `pane` | app, CLI, MCP, skill, HTTP | The Agents pane's own: an agent's turn taken back, or its question to the person |
| `open-level` | pane | It reaches past the open level, and the pane's agent gets the open level only |
| `pane-reads` | pane | The pane's agent reads the open level as `read-level`; this other form of it is for programs |
| `view` | pane | It changes only what the person's screen shows or keeps |
| `batch` | pane | The pane's agent makes every change as one `apply-edits` batch, which `undo-turn` takes back whole |
| `drawn` | app | A read: the app shows what it answers where it is used |
| `headless` | app | It plays with no screen; on the screen the person plays the level |
| `gesture` | app | The app makes it with a drag on the selection, not a press |
| `auto` | app | The app does it on its own: levels move between devices as they are saved |
| `spend` | CLI, MCP, skill, HTTP, pane | It pays for a model with the app's own login, and only the person presses it |
| `person` | MCP, HTTP, pane | Only the person may do it, never an agent for them |
| `login` | app, MCP, HTTP | Each surface has its own login |
| `agents` | pane | An agent's record and log belong to the surface the person runs it from; the pane's agent does not drive other agents |
| `runner` | app | A running agent reports its own events through it; the person reads them in the Agents pane |
| `claim` | app | A claim keeps one agent out of another's work and never refuses the person, so the app has no control for it; it draws every claim on the ground |
| `match` | pane | It starts, steers or reads the match on the person's screen; the pane's agent plays a level headless instead (`play-test`) |
| `lobby` | CLI, MCP, skill, HTTP, pane | The lobby and the party live in the running app, signed with the person's key on the room's relay; only the person is in the room |
| `web` | app, CLI, MCP, skill, HTTP, pane | It is a plan line of gamebuilder.org's own world, which its chat's model and its own tools write alike (`web/blocks.coax`); a level's file takes `apply-edits` instead |
| `site` | app, pane | It reads gamebuilder.org's own counts (`funnel`), which the app has no screen for and a level's agent no use for; the owner reads them at a command line |
| `live` | none | It waits on something a golden case cannot stand in for, so it has no case |

The manifest (`gamebuilder commands --json`) gives each row's `gap`. The
website (`GB_WEB`) has its own grammar and is held to the rows apart (below).
The cloud Builder (`GB_CLOUD`) is outside this rule too: it has the few
rows marked for it, not every row less a reason (below).

### Versions

A build says which it is: `gb_version` in `coax/commands.coax` writes the
release, the commit it was made from (`build/stamp.coax`, which
`scripts/env.sh` writes; `.dirty` when what the core or the CLI is built
from had changes not committed) and `gb_table_digest`, a digest of every
column of every row. `gamebuilder version`, the manifest (`version` and
`table`), OpenAPI's `info.version`, MCP's `serverInfo.version`, the app's
About (`ed_version`) and every HTTP answer (`X-GameBuilder-Version`,
`X-GameBuilder-Table`) say it. A `#test` holds that a change to any column
of any row changes the digest.

Two ends that differ say so. `--attach` and `--remote` send the digest of
the row they call (`gb_row_digest`; the attach call's `row`, the
`X-GameBuilder-Row` header), and the app (`attach.coax`) or the server
(`gb_api_row_same`) refuses it as `version-mismatch` when its own row is not
the same. When only other rows differ, the CLI notes it on stderr.
`scripts/build.sh` installs the CLI and the skill it built
(`scripts/install-cli.sh`), so the CLI on the Mac is the app's.
`gamebuilder --remote version` compares the live `/api/capabilities` with
this build's table; `deploy/api/deploy.sh` runs it after a deploy and
`.github/workflows/hosted.yml` runs it every day.

## The surfaces

### The `gamebuilder` CLI

A Coax program, `cli/main.coax`, that loads `coax/commands.coax` and the core,
and runs with no window and no GPU. It is the first piece that runs on
Windows and Linux, because it needs no UI host.

```text
gamebuilder [--dir DIR] [--attach] [--remote URL] [--json] VERB [ARGS] [FLAGS]
gamebuilder help [VERB]           every row's usage and help, from the table
gamebuilder commands --json       the capability manifest
gamebuilder mcp [--read-only] [--all]
gamebuilder skill [--install]
```

It reaches a level three ways:

| Mode | What it edits | When |
| --- | --- | --- |
| Files (default) | Level and game files in `--dir`, read and written with `ed_open` and `ed_save`, exactly as the app does | Offline; scripts; CI; Linux and Windows today |
| `--attach` | The level open in the running app, through its local socket. Edits land in the open editor, with undo and the lasting highlight | The person is in the app |
| `--remote` | The account's levels and games over the HTTP API | Any computer; the phone's levels once they sync |

In files mode, the CLI refuses a level the app has open, so neither
overwrites the other. How it knows is open question 4. The app keeps its
levels in Application Support under its bundle ID (`Levels.folder` in
`host/Sources/Levels.swift`); the CLI's default `--dir` is that folder.
Where the sandboxed Mac app's folder lands on disk is not confirmed here.

Output is one line a command; `--json` prints the result. Exit 0 for `done`
and `dry-run`, 1 for `refused`, 2 for `needs-approval`, 3 for `unknown`.

### The MCP server

`gamebuilder mcp` speaks JSON-RPC 2.0 over stdio, as `apps/openagents-cli/mcp.coax`
does. A person adds it to their own agent:

```text
claude mcp add gamebuilder -- gamebuilder mcp --attach
```

- **Tools** are rows with `GB_MCP`, named `gb_VERB` with `-` as `_`
  (`gb_apply_edits`). The input schema is made from the row's kinds and
  bounds, and `additionalProperties` is false. `readOnlyHint` is set for
  `read` and `view`, `destructiveHint` for `destructive`. The description
  ends with the effect and approval, as `[edit; the owner]`.
- **What is offered**: the level, edit and play rows by default; `--all`
  adds the rest; `--read-only` offers only `read`. Meta-tools `gb_find` and
  `gb_call` reach a row that was not listed, as `realm_find` and
  `realm_call` do.
- **A call** runs `gb_run` in the same process, with the caller kind `mcp`
  and an agent id for the connection. It does not start a child process.
  Each session is an attached agent with a record and a log of its calls
  (built: issue 6; `docs/agents.md`, "Built: attached agents").
- **Rows that need a person** answer `needs-approval`; with `--attach`, the
  app shows the ask.
- **Remote MCP**: the same tools at `https://GAMES_DOMAIN/mcp` over MCP's
  Streamable HTTP transport, for rows with `GB_HTTP`, signed in as the
  account. How a remote client signs in is open question 5.

### Claude skills

`gamebuilder skill --install` writes `.claude/skills/gamebuilder/SKILL.md`
from the table, as `skill.coax` does: when to use it, each row's usage and
help, the effect and approval, the two formats an agent writes
(`world.edit.v1`, with a spawn and a whole-row cell edit as examples) and the
limits with their numbers. A skill teaches the CLI; MCP teaches tools. One
skill with a section per area is proposed (open question 6).

### The HTTP API

Two services, as already specified elsewhere; this spec gives them one
source.

| Service | Routes | Spec |
| --- | --- | --- |
| GameBuilder's own service (`server/`, at `GAMES_DOMAIN`) | Games, versions, feedback, blocks, reports; the player's routes | `docs/publishing.md`, "API" |
| The same service, levels on the account | `GET /api/levels/:level` (`world.state.v1`), `POST /api/levels/:level` (a `world.edit.v1` batch; 409 when stale, 422 when refused), and a route for each other `edit` row | New. The shape of openagents.com's `/api/worlds/<id>` (`worlds.c`), served by GameBuilder's service |
| The same service, agents | `/api/agents`, `/api/agents/ID/...`, `/api/computers` | `docs/agents.md` |
| Discovery | `GET /api/capabilities` (`gamebuilder.capabilities.v1`: every row, its kinds, bounds, effect, approval, route) and `GET /api/openapi.json` (OpenAPI 3.1) | New. Both generated from the table |

Each row's `route` column is its route. The server's router is made from the
rows, its policy is Coax (as `world-policy.coax` is for worlds), and creator
routes are signed with NIP-98 from a key linked to the account, as
`docs/publishing.md` proposes.

Today levels are the device's, not the account's (`AGENTS.md`, "Accounts and
owners"). Until a level is kept on the account, the HTTP API reaches
published versions, games of yours once synced, and the web's world, not a
level that only exists on a phone. Reaching a device's own levels from
another computer is the relay route of the resident service
(`environment-runtime-v1alpha1.md`), later.

### The Agents pane's tools

The pane (issue 1) registers one in-process server, `gamebuilder`, with
`cl_add_server`, and one tool per row with `GB_PANE` through
`cl_add_server_tool`. The model sees `mcp__gamebuilder__read_level`. When a
call comes in, the event's tool index maps to the row through an array
filled at registration. No host code branches on a tool's name.

| Pane tool | Row |
| --- | --- |
| `read_level` | `read-level` |
| `apply_edits` | `apply-edits` |
| `undo_turn` | `undo-turn` |
| `ask` | `ask` (pane only: a question with buttons; the turn waits) |
| `play_test` | `play-test` |
| `list_palette`, `list_fields` | `list-palette`, `list-fields` |

These are issue 1's tool names, so the rows take them as their verbs. Built-in
tools stay off, as issue 1 says, and a `#test` holds that the session starts
with no tool outside these rows.

### The cloud Builder's rows

A cloud Builder (docs/agents.md, "Built: a cloud agent") runs on Microcoder's
loop on the hosted server. Each command of a step is one row, written as the
CLI takes it (`apply-edits 3 '{...}'`) and read by the table's own argument
reader. Its rows are the ones marked `GB_CLOUD`, the surface `cloud` in the
manifest and in the list above:

| Row | As the Builder writes it |
| --- | --- |
| `read-level` | `read-level LEVEL` |
| `apply-edits` | `apply-edits LEVEL '{...}'` (the batch inline; no file) |
| `list-palette`, `list-fields`, `list-modes`, `list-scrappers` | as the CLI's |
| `claim`, `claims`, `release` | `claim LEVEL box:X0,Z0,X1,Z1|things:ID,ID`, `claims LEVEL`, `release LEVEL` |
| `agent-undo` | `agent-undo AGENT`, on itself only |

The level is always its own, and the server says who calls: `--agent` and
`--key` are refused. `play-test` is not marked: a match would hold the one
server for minutes. A `#test` in `coax/cloud-rows.coax` fails if what the
Builder is told, what the server carries out and what the table marks ever
differ.

### gamebuilder.org

The website's edits are plan lines: the same words whether its chat's model
writes them or its own tools and edit mode do (`bl_edit` hands every edit to
`bl_plan`). Each is a row in the `web` area, on `GB_WEB` only, its gap `web`:
`place`, `ball`, `put`, `turn`, `kind`, `size`, `move`, `remove`, `clear`,
`set`, `reset`, `gravity`, and edit mode's `select` and the building tools'
`spin`. `limit` is no row: it only says on the page why a tool's edit was
refused. Each row's `usage` is the plan line the model is told, word for
word, and `tools/web-rows-check.coax` holds the three together (above). A web
row reached any other way is refused (`not-permitted`), and its golden case is
the website's own `#test` that runs its line. The next step, once the web
world takes `world.edit.v1` batches (it is kept as a `world.doc.v1` level file
now, `web/level.coax`), is for the chat's model to send the same batches as
everyone else. The page's tool labels come from the table, as
`rc_tools_text` does for openagents' `/chat`.

## Permissions

- **Who is asking.** Every call carries a `GbCaller`: a person at a screen,
  the pane's agent, the CLI, an MCP connection, or an HTTP request with an
  account. The app's rule for who may change a game is `coax/games.coax`
  (`ed_may_edit`), and it holds on every surface. A server never trusts it:
  the publishing service and the levels routes check the account themselves
  (`docs/agents.md`, `docs/publishing.md`).
- **Locked levels.** An `owner` row on a level this account may not change
  answers `not-permitted`, names `copy-level`, and changes nothing. An agent
  makes the copy only if the person tells it to.
- **Agents on undo.** Each agent has an id, passed to `ed_agent_apply` as
  `agent`. The CLI session, each MCP connection and each pane agent get
  their own. Its turns undo with `undo-turn` while only agents changed the
  level since (never under the person's own change); the person's ⌘Z undoes them as ordinary steps. 32 turns are kept
  (`AGENT_TURNS`). A turn names its agent, so the pane and the agents page
  can say who changed what.
- **What waits for a person.** `publish`, `set-live`, `unpublish`,
  `delete-game`, `lobby-say`, `lobby-report`, `party-invite`, `login` and
  `logout` are `person`. An agent's call answers `needs-approval` and the app
  asks. The owner can allow one agent to publish ("may publish", off by
  default, `docs/agents.md`); speaking in the lobby or a party for the person
  stays a person's act (open question 7).
- **Two-step rows.** `delete-game` and anything else `destructive` needs
  `--yes`, `"confirm": true` or a tap, as `rc_needs_yes` does.
- **Spending.** `decide` is `spend`: the System One door budgets each key
  (`AGENTS.md`, "Decide"). A cloud agent's cost is `docs/business/agents.md`'s.
  A `spend` row shows its cost before it runs where the cost is known.
- **Secrets.** No row takes a token, key or password as an argument. The CLI
  keeps its login where the app's Mac build keeps one (`~/.openagents/`),
  never prints it, and never puts it in a URL.

## Which surface each platform gets

| Surface | Mac, App Store | Mac, direct download | Windows, Linux | iPhone, iPad | Web |
| --- | --- | --- | --- | --- | --- |
| The app's UI | Yes | Yes | When the Coax UI host comes | Yes | gamebuilder.org |
| `gamebuilder` CLI | A separate download; files and remote | Yes, with `--attach` | Yes: the first piece there | No | No |
| Local MCP (stdio) | Through the CLI, with `--attach` (below) | Yes | Yes | No | No |
| Remote MCP, HTTP API | Yes | Yes | Yes | Yes: the only way in | Yes |
| Claude skill | Where Claude Code runs | Yes | Yes | No | No |
| Agents pane | Through the resident service (issue 1, step 2, route A) | In-process SDK | In-process, later | Cloud agents only | The `/agents` page |

The App Store build is sandboxed and may only connect out on the network
(`host/GameBuilder-macOS.entitlements`). It may still listen on a Unix socket
inside its own container, which is how `--attach` reaches it (C5) with no new
entitlement. Starting the person's `claude` is another matter: that waits for
issue 1's step 2. The iPhone runs no local server and no `claude`; a phone's
levels reach other surfaces only through the account.

## Determinism and tests

- **Golden cases.** `tests/commands/` holds, for each row, at least one case:
  a starting level file, the call, and the result JSON. A `#test` runs each
  through `gb_run`. `scripts/check.sh` runs the same cases through the CLI
  binary, through `gamebuilder mcp` (JSON-RPC lines in, answers out), and,
  once `server/` exists, through a local server. The JSON must be the same
  bytes after the transport's envelope is taken off. Revisions are digests of
  the level (`ed_agent_rev`), so they repeat.
- **Play-test.** `play-test` runs a match headless: the level, a mode, a
  size, bots on every seat, a fixed step of 1/60 s, a seed, and at most 600
  seconds. A match already seeds its random numbers the same way every time
  (`play_rand`, seeded at the match's start) and runs to its end in a
  `#test`. It answers `gamebuilder.playtest.v1`: whether it ended and how,
  the score, the seconds, knockouts, bots stuck and where, and markers no bot
  reached, and each bot's path. Every other game type is walked through by a
  bot on the engine's walking grid. Two runs, and the CLI, MCP and pane, give
  the same answer. Dt must not come from a clock, and the seed is part of the
  call.
- **Pictures.** The app draws a picture of the level before and after each
  agent's change and of each play-test, with the level's own renderer, and
  keeps the last 32 beside the level; the `pictures` and `picture` rows read
  them (docs/agents.md, "Built: pictures, the replay and the play-test
  picture"). A picture is not drawn without the app: a `shot` row that draws
  one from the command line waits for the engine's renderer without a window
  (`docs/openagents-integration.md`).
- **The web.** `tools/player-test.coax` drives gamebuilder.org with real
  input today (`docs/player-testing.md`). Its journeys stay; the API gets its
  own golden cases.

## Order

Status, 2026-11-17: C0 is done and C1 runs. `coax/commands.coax` holds the
rows whose core exists (levels, games, editing through the palette and
inspector, the agent seam, play modes), with `gb_run` and the manifest;
`gamebuilder` (and `gb`) runs them in files mode on the Mac (docs/cli.md). The
app's own buttons are rows too: undo, redo, set-tool, focus, copy, cut, paste,
duplicate and decide are rows offered only to the UI (`GB_UI`) until
`--attach` (C5) lets the CLI reach the open level. `coax/controls.coax` is
the app core's root: `GB_CONTROLS` lists every toolbar button, menu item and
banner with its row, `ed_command` carries a row out with JSON arguments
(filling in the open level and the selection), and the SwiftUI toolbar, menu
bar, Edit menu and banner draw from `ed_control_*` and press through
`ed_control_press`. Decide and New Game's picker of game types are carried out by the
host (`GB_HOST_DECIDE`, `GB_HOST_NEW_GAME`); the side panel's palette and
inspector call the row's exports as they draw (`GB_HOST_PANEL`).
`tools/commands-check.coax`, run by `scripts/check.sh`, holds that every
`ed_` export is in a row's `impl`, in `GB_PLUMBING` (drawing, continuous
input, what a sheet shows), and that the hosts name no action row's export past
its row (`--host`). `GB_LATER` is gone: open-level and open-game (the core
says which file, `ed_open_due`, and the host reads it), play, stop and the
match's rows, and the lobby's and party's rows are rows now. C1's `--key` is built: an edit row
takes a key, the CLI keeps the answer in `DIR/results/K` and answers it again
on a retry (docs/cli.md). The golden cases are `tests/commands/*.case` (a
`> VERB {json}` call and its `< answer`); `tools/commands-golden.coax`, run by
`cli/check.sh`, carries out each call through the core (`gb_command_run`),
the CLI and `gamebuilder mcp`, and holds that all three answer the case's JSON
byte for byte (`--write` fills a `< ?` line where they agree). Every row has
a case now, on every surface it has: the app's own rows (undo and the rest)
run through the core on one editor from call to call, as on the screen
(`tests/commands/app.case`), and the account's through the CLI.
`.github/workflows/linux.yml` builds the Coax compiler at the pin (a
published build, or a bootstrap with `COAX_MUTATION=0`: under three minutes
on an arm64 container), then runs `scripts/check.sh` and builds the Linux
app. It passed in an Ubuntu 24.04 container on 2026-11-17; GitHub doesn't
start workflows in the org's private repos yet (NEEDS_OWNER.md). C4 is
built: `coax/playtest.coax`'s `ed_play_test` plays an arena level headless
with bots in every seat (`Play.test` makes yours a bot's), in `PLAY_STEP`
steps from the call's seed, for at most 600 seconds, and answers
`gamebuilder.playtest.v1`; the `play-test` row offers it to the CLI, MCP, the
skill and the pane. `stuck` is a Scrapper that moved less than 0.5 m in five
seconds of running match; `unreached` is a goal, hoop, ball place or health
pack nothing came within 3 m of. Two runs give the same bytes (a `#test`, and
`tests/commands/play.case` through the core, the CLI and MCP), and a goal
walled off from the arena is reported unreached. Every other game type is
walked through (`playtest_walk`): a bot walks from your start to each thing it
can reach on the engine's walking grid, nearest first, and the answer says
what it reached, where it came nearest what it couldn't, and its path.
C5 is built: `coax/attach.coax` is the app's side of `--attach`. The host
listens (`ed_attach_listen`) on a socket in its temporary folder, which the
sandboxed App Store build may do inside its container, so it keeps its
sandbox (no new entitlement), and names it in the Levels folder's `attach`
file. It polls (`ed_attach_poll`, a 20 Hz timer on the Mac, five times a
second on Linux). A call runs through `gb_command_run` on the open editor,
like a press. A dry run runs on a copy. What the call changed stays
highlighted until dismissed (`coax/changes.coax`: an outline in the caller's
colour, a ghost where a thing was; the cube pass's edge lines, tinted).
The CLI and `gamebuilder mcp --attach` offer the app's own rows too (never
Decide, which spends). Open question 4 is answered by this: attach, not a
lock file. The in-app check (an edit from the terminal is highlighted and ⌘Z takes it
back in the running app) is the owner's, in NEEDS_OWNER.md.
C3 is built: `coax/agents.coax` is issue 1's step 1, drawn by the component
kit with no SwiftUI view for it (`host/panel/agents-host.c` carries it; the
toolbar's Agents is the `agents` row). An agent is the person's own Claude
Code, started by the Coax Claude Agent SDK in-process, with built-in tools
off, no settings files, 20 turns and $2 of the plan a message, and
`ANTHROPIC_API_KEY` taken out of its environment; a session reporting any
key source but the login is stopped. Its tools are the `GB_PANE` rows
(below), carried out by `gb_command_run_as` with the agent's id, so a batch
is one step ⌘Z takes back, `undo-turn` takes back only the agent's own, and
what it changed stays highlighted in the agent's colour (`coax/changes.coax`)
and is an edit event of its record with its words. `ask` is the pane's own
row: the turn waits on the person's answer, a button or their own words. One session runs at a time and a second agent queues.
Starting `claude` needs a build without the App Sandbox, so step 1 takes
docs/agents.md's way B for now: `GameBuilder-macOS-Dev` (`scripts/build.sh
macos-dev`), while the App Store target keeps its entitlements and its pane
says the next step in words. `#test`s feed the SDK the lines `claude` would
send: the session's arguments have no built-in tool and allow only the pane's
tools; an apply lands, stays highlighted and undoes; a stale batch and a locked level are
refused; an ask waits and an answer goes back; Stop ends the session. The
live run with a signed-in Claude Code is the owner's, in NEEDS_OWNER.md.
C6 is built on this computer: `coax/api.coax` is the HTTP API's pure part
(the request read strictly, routes found by exact paths and the table's
verbs, a route's arguments read as an MCP call's, a refusal's code as its
status: 403 not yours, 404 not found, 409 stale, 422 refused), with
`GET /api/capabilities` (the manifest) and `GET /api/openapi.json` (OpenAPI
3.1) made from the rows. `gamebuilder serve` carries it on 127.0.0.1 around
the same `cli_exec` as the CLI and MCP, and the golden cases now run four ways
(core, CLI, MCP, HTTP) to the same bytes. A row is offered over HTTP with
`GB_HTTP`, which every files-mode row has; the account rows and the app's own
buttons are not routes. The hosted half runs at gamebuilder.org:
`gamebuilder serve --accounts DIR` keeps each account's levels in its own
folder, `DIR/ACCOUNT`. Every signed call carries the OpenAgents login as a
bearer (the same one the app and `gamebuilder login` hold), and the server asks
the login service whose it is (`GET /api/auth/account`) once per call, with no
cache. No login, or one that ended, is 401 with
`WWW-Authenticate: Bearer realm="gamebuilder"`. Discovery stays open, and a
browser `Origin` from another site is refused before any login is asked.
A browser logged in on the site has no bearer: its login is the login
service's HttpOnly cookie. The server takes that cookie as the caller's login
only on a `POST` whose `Origin` is the site itself (`coax/api.coax`
`gb_api_cookie_call`; any other is 403 before the login service is asked),
and passes it to the login service as it passes a bearer. That is how
gamebuilder.org keeps a logged-in person's game on their account with the
same rows (`web/account.coax`, issue 16; docs/web-world.md, "Kept on your
account"). `tools/web-account-check.coax` holds the website's saving to the
app's, end to end.
An agent's newest picture is the one thing served with no login at all:
`GET /api/pictures/ADDRESS.jpg` (`image/jpeg`) on the hosted server, its
address the first 16 bytes of the SHA-256 of the agent's id and the JPEG, as
hex, which only the `agents` row's answer gives (issue 9; docs/agents.md,
"Built: pictures, the replay and the play-test picture"). Any other method is
405, an address it doesn't keep 404, and a server on this computer keeps none.
`write-content LEVEL REVISION FILE` puts a level's file whole, only at the
revision it was read at (`read-content` and `list-levels` say each level's
revision), so two writers never lose each other's change: the second is 409.
`tools/hosted-check.coax` runs the whole thing on this computer, standing in
for the login service: two accounts kept apart, the 401s, 409 stale, MCP over
HTTP, `--remote` and the files on disk. Deployed 2026-11-17 on the owner's
go-ahead (`deploy/api/deploy.sh`): Cloud Run `gamebuilder-api`, one instance
at most, each account's files in the bucket `openagentsgemini-gamebuilder-levels`
mounted at `/data`, reached only through gamebuilder.org's load balancer
(`scripts/web-deploy.sh domain` sends it the API's paths). Checked live from a
terminal: `/api/capabilities` and `/api/openapi.json` answer, an unsigned
`/api/rows/list-levels` and `/mcp` are 401, and `gamebuilder --remote
list-levels` with the real CLI login reads that account's levels.
C7 has begun the same way: `POST /mcp` on `gamebuilder serve` is MCP over
HTTP (the Streamable HTTP transport answering with JSON, no event stream),
the same `gb_mcp_handle` as `gamebuilder mcp`. A server here refuses any
request whose `Host` or browser `Origin` isn't this computer, so neither a
web page nor DNS rebinding reaches it. The same `POST /mcp` on the hosted
server is remote MCP signed in as the account, and `gamebuilder --remote`
sends any CLI or `gamebuilder mcp` call to the site with the CLI's login, so
an agent on another computer edits the account's levels with the same rows.
The phone's half is `coax/sync.coax`: every few seconds while logged in, a
pass compares each level's revision on the device, on the account and at the
last pass. Only the device changed: it goes to the account
(`write-content`). Only the account changed: it comes to the device, and the
level on screen reopens. Both changed: the account's copy keeps the number and
the device's becomes a new level on both, so nothing is lost. The app
(`host/Sources/Sync.swift`) only carries the bytes. Games are kept the same
through `list-games`, `new-game`, `rename-game`, `move-level`, `delete-game`
and `delete-level`, and a deletion on one side is made on the other rather
than undone (issue 19; the rules are in `docs/web-world.md`, "The app has the
same games"; `tools/sync-games-check.coax` holds both directions against a
real `serve --accounts`). Its `#test`s play both
sides, including an agent's change reaching the phone's open level. Run live on
this computer with the real login service: `--remote --server` with the CLI's
login made a level and added a block in that account's own folder on a local
`serve --accounts`, the login checked by gamebuilder.org's service.
`scripts/sync-check.sh` adds the phone: the simulator's app syncs with that
server and the block appears in its open level. The phone's side has also run
live in the Simulator, logged in to a stand-in login service on this computer
(a test account and code, the same three auth routes, the app pointed at it
with its `OAAPIOrigin` setting), syncing with a local `serve --accounts`
that asks that stand-in who each caller is. The phone's levels reached the
account in its first pass; a tree an agent added with `--remote --server`
showed in the phone's open level about 6 seconds later; a ball added on the
phone reached the account in the next pass; and the agent's deletes of both
came back to the phone. The Simulator's app must be signed (ad hoc is
enough): unsigned, it has no Keychain, its login's tokens aren't kept and it
never syncs, so `sync-check.sh` builds it signed. The same run with the real
login (the owner's email code) and the deployed one on a real phone are in
NEEDS_OWNER.md.
C2 has begun: `gamebuilder skill` writes the Claude skill from the table
(`gb_skill`; the repo keeps a checked copy at
`.claude/skills/gamebuilder/SKILL.md`), and the `engine` row says which
openagents commit GameBuilder is built on and how to propose an engine change
as a pull request at OpenAgentsInc/openagents. One skill, with a section per
area, answers open question 6 for now. `gamebuilder mcp` runs too
(`coax/mcp.coax`): a tool for each `GB_MCP` row, a call read and carried out
in the same process as the CLI's. Every row is listed for now, so `--all`,
`gb_find` and `gb_call` wait until the table is long enough to need them.

| Stage | What ships | Done when |
| --- | --- | --- |
| **C0, the table** | `coax/commands.coax` with every row whose core exists (levels, games, editing, palette and inspector, play setup, lists, the agent seam), `gb_run`, `ed_command`, `GB_CONTROLS`, `GB_PLUMBING` | The drift tests pass; the toolbar's buttons come from `GB_CONTROLS` and call `ed_command`; the app behaves exactly as before; `scripts/build.sh` passes |
| **C1, the CLI** | `gamebuilder` in files mode on the Mac and Linux | Every golden case gives the same JSON through the CLI as through `gb_run`; `--dry-run` and `--key` work; a Linux build runs in CI |
| **C2, MCP and the skill** | `gamebuilder mcp`, `gamebuilder skill --install` | A person runs `claude mcp add gamebuilder -- gamebuilder mcp` and asks for two ramps; they appear in the level file, undo as one step, and the MCP answers equal the CLI's |
| **C3, the pane** | Issue 1, step 1, with its tools from the table | Issue 1's "Done when" list |
| **C4, play-test** | `play-test` as an export and a row | Two runs give the same bytes; a level whose goal no bot reaches says so |
| **C5, attach** | `--attach` to the running app | An edit from the terminal appears in the open app, stays highlighted, and ⌘Z takes it back; the App Store build stays sandboxed |
| **C6, the HTTP API** | Routes from the rows (`coax/api.coax`, `gamebuilder serve` on this computer and `serve --accounts` hosted at gamebuilder.org), `/api/capabilities`, `/api/openapi.json`, levels on the account | The route test passes; the golden cases give the same JSON over HTTP |
| **C7, remote MCP and the phone** | MCP over HTTP; the phone's levels sync | An agent on another computer edits a phone's synced level, and the phone shows the change |
| **C8, as the UI grows** | Rows for tables, scripts, assets, publishing, the lobby and party | Each lands with its UI in the same change, or the drift test fails |

The rule from C0 on: a change that adds a feature to the UI adds its row in
the same commit. The drift test makes that the only way to land it.

## Open questions for the owner

1. **The result's name.** Emit the engine's `realm.result.v1` as it is, or a
   `gamebuilder.result.v1` profile of it? Proposed: the engine's, with
   `realm` holding the level or game id.
2. **Where the levels routes run.** GameBuilder's own service (proposed, as
   `docs/publishing.md` recommends), or openagents.com's worlds API with
   GameBuilder levels as worlds? Built for now: its own service, `gamebuilder
   serve --accounts`, with each account's level files on a mounted volume
   (no database yet: a level is a file, and its revision is its digest),
   deployed at gamebuilder.org on Cloud Run.
3. **The CLI's login.** Its own email-code login in Coax, the `openagents`
   CLI's login shared, or no login until C6?
4. **Files mode and the open app.** A lock file the app writes for the level
   it has open, or files mode only when the app is not running? Taken in C5:
   neither; `--attach` reaches the open level, and docs/cli.md says to close
   a level before changing its file.
5. **Remote MCP sign-in.** OAuth as MCP clients expect, or a token made in
   the app? Built for now: the OpenAgents login as a bearer, which
   `gamebuilder login` and the app already hold; `--remote` uses it. MCP
   clients that need OAuth discovery wait on this answer.
6. **Skills.** One `gamebuilder` skill, or one per area (build, play,
   publish)?
7. **Agents and other people.** May an agent ever speak in the lobby or
   invite to a party for the person? Proposed: never in the first stages.
8. **Lobby and party from the CLI.** They need a WebSocket in Coax; the app
   uses Swift's today. Worth it, or app only?

## What I could not confirm

- That the sandboxed Mac app's `Levels` folder is under
  `~/Library/Containers/com.openagents.gamebuilder/`. `Levels.swift` asks for
  Application Support; where the sandbox puts that was not checked.
- That `coax/play.coax` has no source of randomness besides `play_rand` and
  the fixed per-id seed near line 1159; no headless run was made outside the
  `#test`s.
- That the core runs with no host at all. The `#test`s run it in the Coax
  test runner without a window, which is strong evidence, but no CLI binary
  was built.
- That a Coax HTTP or relay client is ready for the CLI's `--remote` and the
  lobby. The pinned tree has `coax/lib/net/http.coax` and `relay_conn.coax`;
  whether they serve a headless CLI as they stand was not checked.
- The count of exports: 263 `ed_` exports on 2026-11-17, every one accounted
  for by `tools/commands-check.coax`.
