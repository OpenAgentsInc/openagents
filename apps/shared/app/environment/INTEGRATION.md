# Environment and repository selection: integration

The two chips above the chat composer and their picker (GitHub #13645, for
#13620). They say where the next message runs and which repository it works
in. Owner: `claude-env-select`.

## Files

| File | What it is |
| --- | --- |
| `select.coax` | The state (`EnvSelect`): the account's computers, whether each one answers, the repositories each one has checked out, and each draft's choice. No host calls, no drawing |
| `station.coax` | `es_station_read`: a StationReport body into the fact `select.coax` keeps, through the router's one codec (`coax/lib/agent/router/wire.coax`) |
| `view.coax` | `es_chips` (what the composer's two chips show) and `es_picker` (the open picker, drawn with the GPU kit on a frame context) |
| `bridge.coax` | The only file that names the app core: reads go through the core's outbox as the guest `OWN_ENVIRONMENT`, and the current draft's choice fills the core's slots (`ACT_ENVIRONMENT`, `ACT_REPOSITORY`) |
| `*-checks.coax` | Checks. Example computers and repositories live only here |

## Where the data comes from

Both lists use the environment contract as it is:

- The computers: `GET /api/chat/environments/v1`, read by `ea_catalog_read`
  (`coax/lib/agent/environment/wire.coax`).
- Whether a computer answers, and its repositories: its StationReport,
  `GET /api/chat/environment/station/v1?host=KEY` (`Station.repos` in
  `proto/openagents/router/v1alpha1/router.proto`). A 503 or 404 means it does
  not answer; any other failure reads as "We couldn't check".

The repository picker lists the repositories that the answering computers
have checked out, each once. With a computer chosen, it lists only that
computer's. There is no GitHub-wide repository listing on the server, so the
picker does not offer repositories that no connected computer holds.

## What the root does

Load the core first, then `bridge.coax` (it loads `select.coax` and
`station.coax`) and `view.coax`. Keep one `EnvSelect` (start it with
`es_new()`) and one zeroed `EsBridge` next to the `AppCore`.

```coax
#load "apps/shared/app/core/core.coax";
#load "apps/shared/app/environment/bridge.coax";
#load "apps/shared/app/environment/view.coax";

env := [1] EnvSelect.[es_new()]; bridge: [1] EsBridge;

// after every app_dispatch / app_host_* call, outside a dispatch:
es_bridge_sync(c, bridge[..], env[..]);

// app_dispatch answered APP_GUEST: offer the result to the picker first
if !es_bridge_guest(c, bridge[..], env[..], event_text, now_ms) { /* another guest's */ }

// a tap whose committed row id is one of ours ("composer.environment",
// "composer.repo", "env.*"):
if es_press(env[..], id, now_ms) { es_bridge_sync(c, bridge[..], env[..]); }
if es_take_login(env[..]) { app_dispatch(c, ACT_LOGIN_SHOW with flag true); }

// each composer frame: the chips' values
k := es_chips(env[..]);
composer_surface_chips(composer, true, k.environment, k.environment_on, k.repository, k.repository_on);

// each page frame, while env[0].sheet != ES_SHEET_NONE: the picker over the chat
size := es_picker(f, env[..], x, y, w, h, CHECK_ICON);
es_scroll(env[..], drag_dy, size.content, size.view);   // on a drag inside it
```

`es_picker` draws nothing when no picker is open. Redraw when
`app_dirty_take` includes `SURF_ENVIRONMENT`, and after any `es_press`.

The bridge calls `app_dispatch` itself for `ACT_ENVIRONMENT` and
`ACT_REPOSITORY`, so `es_bridge_sync` must not run inside a dispatch.

## Saving choices with the drafts

Each draft keeps its own choice. A new chat's choice moves to its
conversation when the conversation gets its ID. To keep choices across a
relaunch, the host saves `es_save(env, out)` (plain text, one line per draft
with a choice) next to the drafts and calls `es_restore(env, text)` once after
launch. A saved computer that is no longer the account's turns back to Default
when the list is read.

## Services needed

Only `FX_HTTP` GET through the core (`app_guest_http`); both paths pass
`up_path`. No new host service. The picker never calls the host.

## Capabilities: what the #13620 lane must add

Today the chat submit request has no field for a computer or a repository:
the server routes every task itself (docs/router/README.md). So both choices
are off by default:

- `es_capabilities(env, manual_ready = false, repo_ready = false)`.
- With `manual_ready` off, the environment picker lists Default and every
  computer with whether it answers, and the computer rows take no press. The
  chip always says Default.
- With `repo_ready` off, the repository chip is drawn disabled with
  "No repository" and takes no press.

To turn either on, the #13620 lane adds to the submit request (and the router
input): the chosen host key (`app_environment(c)`, 64 hex, "" for Default)
and the repository name (`app_repository(c)`, "" for none). The core then
sends its slots with the request and calls `es_capabilities(env, true, true)`.

## Checks

```sh
coax/remote/remote.sh run -- bash -c 'coax/bin/coax test apps/shared/app/environment/select-checks.coax \
  && coax/bin/coax test apps/shared/app/environment/view.coax \
  && coax/bin/coax test apps/shared/app/environment/bridge.coax --library --traversal 1000000000 \
  && coax/web/tool.sh apps/shared/app/environment/station-checks.coax'
```
