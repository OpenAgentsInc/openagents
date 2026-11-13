# A world's stored scripts v1

A world in the OpenAgents Metaverse names a realm script by the SHA-256 of
its module, in a row of its content tables
([realm-script-v1.md](realm-script-v1.md), `metaverse-engine/script/rows.coax`).
This page is the server's side: how the API stores a module behind the
module check, serves it, compiles a source into one, and how a script is
published, reported and switched off. The design is
[docs/research/2026-11-13-realm-scripts.md](../research/2026-11-13-realm-scripts.md).

The profile [proto/world/script-store-v1.json](../../proto/world/script-store-v1.json)
is written from the code and pins the limits, the codes and a fixture of
each shape:

```sh
coax/web/tool.sh metaverse-engine/tools/script-store-profile.coax proto/world/script-store-v1.json
```

The one codec is `metaverse-engine/script/store.coax`. The rules are
`coax/web/api/script-policy.coax` (`#authority`, zero surviving mutants),
applied by `coax/web/api/script-store.coax`. The routes are
`coax/web/api/scripts.c`; the rows are migration `0186_world_scripts.sql`.

## Routes

| Route | Who | Answer |
| --- | --- | --- |
| `GET /api/worlds/ID/scripts` | anyone who may read the world | `world.scripts.v1` |
| `GET /api/worlds/ID/scripts/HASH` | anyone | the module's bytes, `application/wasm`, cached for good (the hash is the name). 404 `not-stored`; 410 `withdrawn` |
| `GET /api/worlds/ID/scripts/HASH/source` | who may store | the source the module was compiled from, `text/plain`; 404 for a module that was uploaded |
| `POST /api/worlds/ID/scripts?name=NAME` | who may store | body: a module's bytes. Answer: its manifest, `realm.script.v1`. 422 `refused-module` |
| `POST /api/worlds/ID/scripts/compile?name=NAME` | who may store | body: a script's Coax source. Answer: the manifest, as above. 422 `refused-source`; 503 `no-compiler` |
| `POST /api/worlds/ID/scripts/HASH/report` | any account that is logged in | body: a reason. Answer: `world.script.done.v1` |
| `GET /api/admin/scripts/reports` | a site admin | `world.script.reports.v1`: the open reports |
| `POST /api/admin/scripts/HASH/off` | a site admin | body: why. The module is switched off everywhere and its open reports are marked `removed` |
| `POST /api/admin/scripts/HASH/on` | a site admin | switched back on |
| `POST /api/admin/scripts/HASH/keep` | a site admin | its open reports are marked `kept` |
| `POST /api/worlds/ID?confirm=HASH,HASH` | who may write the world | a publish batch (`world.edit.v1`), with the hashes of the scripts the publisher confirms |

### Storing a module

The server runs the module check and a load on the bytes
(`metaverse-engine/script/host.coax`, `sh_load`), the same check a page and a
room run. It writes the manifest from the checked module. From the author it
takes two things only: the name (`?name=`, lowercase letters, digits and
hyphens, a letter first) and the state record's fields, which decide how a
script's own state is kept across a swap. The fields go in the header
`X-Script-Fields` as the JSON array the build writes in `NAME.json`
(`state.fields`: `[{"name","type","at","size"},...]`); leave the header out
for a script that keeps nothing across a swap. A field that lies outside the
module's own state record is refused.

A module the world already stores is kept as it was stored: one row, the
first manifest.

### Compiling a source

The server hands the source to the compile job (below) and then checks and
stores the module the job made, exactly as an upload. Nothing the job says is
believed but the script's name and state fields. The row keeps the source, so
an editor can open the script again. `limits.compiler` in `world.scripts.v1`
is 1 when this server has a job and 0 when it has none; with none the route
answers 503 and the page says compiling is not available here.

### On a dev server

`coax/web/serve.sh` answers the same routes for its own world, so
`openagents realm world-scripts`, `world-script-put` and `world-script-compile`
work with `--origin http://127.0.0.1:PORT`, and the page fetches the modules
its rows name, before anything is deployed (#13485). The checks, the manifest
and every refusal are the API's (`metaverse/splice-bots/tools/script-local.coax`
loads `coax/web/api/script-store.coax`). The modules are files under
`apps/openagents-web/build/.worlds/ID.scripts/`: `HASH.wasm`, `HASH.json` (its
manifest) and `HASH.coax` (the source, for a compiled one). The dev server runs
`coax/web/script-job.sh` for a compile, keeps no day's counts, and treats
everyone on the computer as an admin, since it listens on 127.0.0.1 only.

Scripts are stored for a world whose content tables can name them. Today
that is Splice Bots. A realm's world (kind `realm`, #13422) has no scripts table,
and these routes answer 404 for it.

## Who may store

The world's owner or one of its admins, or a site admin; logged in, from this
site (`wsp_may`). Splice Bots has no owner, so today that is the site's admins: the
owner and admin half of the rule takes effect when a world with an owner runs
scripts. The rule is wider than who may write the
rest of a world's document (site admins, `wr_write`) for scripts only: the
owner lifted "realm code is ours or a reviewed pull request" for realm
scripts on 2026-11-13, and for nothing else.

## Limits

Each is stated in `world.scripts.v1` and in the refusal it causes.

| Limit | Value |
| --- | ---: |
| Scripts a world runs (rows of its `scripts` table) | 8 |
| A module's bytes | 65,536 |
| Modules a world stores | 64 |
| Modules an author sends in a day | 200 |
| Sources an author has compiled in a day | 100 |
| Reports an account sends in a day (and 6 a minute) | 20 |
| A source's bytes | 65,536 |
| A reason's bytes (plain ASCII, no quote or backslash) | 160 |

When a world's store is full, the modules that no revision names (dev, live
and the one kept for a release) and that were stored a day ago or more are
collected, and the store is tried again.

## Edits and publishing

A script row is a cell of the world's content tables, edited with
`world.edit.v1` like any cell. Two rules are added, with no change to that
shape.

**An edit names only what the world stores.** After the realm's own check
passes, a row whose hash the tables did not name before the batch must name a
module the world stores that is not switched off. Otherwise the batch is
refused (422) in words: `script N: this world has stored no module of that
hash: store the module first`, or `script N: that script is switched off, so
no edit names it`. A row that already named a module keeps standing when the
module is switched off later.

**A new version may do less, never more, unasked.** At a publish, each script
row in dev is compared with live's rows of the same name. A row needs the
publisher's confirmation when live has no row of its name (it is new), or when
its module has a handler, a power or a call that no live module of its name
has (`sm_growth`). A version that only drops handlers, powers or calls
publishes with no confirmation, and so does a hash live already runs. Without
the confirmation the publish is refused, 409, with a
`world.script.refusal.v1` whose `asks` list each script and what it adds:

```json
{"schema":"world.script.refusal.v1","code":"engine.diagnostic.script-grows.v1","retry":true,
 "error":"these scripts are new or do more than the live ones: confirm each by its hash","detail":"","at":-1,"limit":0,"left_ms":0,
 "asks":[{"name":"pacer","hash":"5e93…5186","new":false,"handlers":[],"powers":["engine"],"calls":["sa_strike"]}]}
```

A page shows the change and asks; the publisher repeats the same batch to
`POST /api/worlds/ID?confirm=HASH,HASH` with the hashes they accept.

## Reports and the switch

Any account that is logged in reports a stored script with a short reason. A
site admin reads the open reports and either keeps the script or switches its
module off by hash, with a reason. A module that is switched off:

- is not served (410, with why) and is not stored again;
- is not named by any new edit;
- stays in a world's published rows: the world keeps its revision.
  `world.scripts.v1` says `"off":true` and why, and the page and the room
  skip that script. A thing whose script is skipped walks its plan.

## `world.scripts.v1`

Compact JSON in one form, scripts ascending by hash:

```json
{"schema":"world.scripts.v1","world":"splice",
 "limits":{"scripts":8,"module_bytes":65536,"stored":64,"call_fuel":20000,"tick_fuel":400000,"uploads_a_day":200,"compiles_a_day":100,"reports_a_day":20,"source_bytes":65536,"why_bytes":160,"compiler":1},
 "scripts":[{"manifest":MANIFEST,"off":false,"why":"","source":true}]}
```

`MANIFEST` is the stored `realm.script.v1` object, the bytes the server
wrote. `source` says whether the server keeps the source it was compiled
from. A reader refuses any bytes but the writer's (`ss_list_read`).

## `world.script.refusal.v1`

```json
{"schema":"world.script.refusal.v1","code":ID,"retry":BOOL,"error":WORDS,"detail":NAME,"at":BYTE,"limit":N,"left_ms":MS,"asks":[...]}
```

`error` holds the words, as a world route's plain refusal does. `detail` and
`at` are a refused module's refusal name and byte (`""` and -1 otherwise).
`limit` is the limit that was reached and `left_ms` when a daily one lifts (0:
not that). A refusal for who is asking, or for a world that does not exist,
is the world routes' plain `{"error":WORDS}` (401, 403, 404).

| Number | Code (`engine.diagnostic.script-NAME.v1`) | Status | Retry | Means |
| ---: | --- | ---: | --- | --- |
| 1 | `refused-module` | 422 | no | The module check refused the bytes. |
| 2 | `bad-face` | 422 | no | The module's face answered out of range. |
| 3 | `not-stored` | 404 (422 at a publish) | no | The world has stored no module of this hash. |
| 4 | `withdrawn` | 410 | no | The module is switched off everywhere. |
| 5 | `grows` | 409 | yes | A publish adds a script or lets one do more. Repeat it with the hashes confirmed. |
| 6 | `store-full` | 422 | no | The world stores as many modules as it may. |
| 7 | `rate` | 429 | yes | The day's limit is spent. |
| 8 | `no-compiler` | 503 | no | This server compiles no source. |
| 9 | `refused-source` | 422 | no | The compile job refused the source; the words are its own. |
| 10 | `bad-request` | 400 | no | A name, a hash, a reason or a body that does not read. |

Codes 1 and 2 are the script host's own (`metaverse-engine/script/frame.coax`).

## The compile job

The compiler, clang and the linker are not a sandbox, so a source we did not
write is compiled in a job (`metaverse-engine/tools/script-job.coax`, through
`coax/web/script-job.sh SOURCE NAME OUTDIR`). The API runs the command named
by `OA_SCRIPT_JOB`; with the setting empty it compiles nothing.

- **The source names no file.** Before any compiler runs, every `#word` in
  the source is held to a list: `#script`, `#effects`, `#export_c`,
  `#persisted`, `#test`, `#assert`, `#forall`, `#steps`, `#run`, and `#load`
  only as the whole line `#load "api.coax";` (or `#load "../api.coax";`). So
  no `#import`, `#import_proto`, `#foreign` or `#export`. The scan does not
  parse: a `#word` in a comment or a string counts too.
- **An empty directory.** The build runs in a new temporary directory that
  holds only the source and copies of `metaverse-engine/script/api.coax`.
- **Limits**, stated in the tool and said in every refusal: a source of
  65,536 bytes, 60 seconds for the whole job, 60 CPU seconds a process, files
  of 16 MiB, and 2 GiB of address space where the platform enforces it.
- **No environment.** The API passes only where the tools are (`PATH`,
  `HOME`, `TMPDIR`, `COAX`, the Wasm toolchain's paths), and the wrapper
  starts the tool with `env -i` and a `HOME` and `TMPDIR` of its own.

What is and is not enforced:

| | macOS | Linux |
| --- | --- | --- |
| Environment taken away | yes | yes |
| CPU seconds, file size | yes (`ulimit -t`, `-f`) | yes |
| Address space | no: macOS does not enforce `ulimit -v` | yes (`ulimit -v`) |
| Network denied | yes, with `sandbox-exec` | yes where the kernel allows a user and network namespace (`unshare -rn`) |
| Writes outside the job's directory | denied under `/Users`, `/etc`, `/usr`, `/opt` by `sandbox-exec` | not denied by the wrapper |
| Reads of other files | not denied: the job runs as its caller's user | not denied by the wrapper |

`OA_SCRIPT_JOB_SANDBOX=required` makes the wrapper refuse to run where it can
deny no network; the default, `try`, runs and says so. A server sets
`required` and runs the job as a user that holds nothing, in a container with
no network and no service account. The Linux column is not yet run by a
check: the checks below ran on macOS.

Checks: `coax/web/tool.sh metaverse-engine/tools/script-job-check.coax`
(compiles the sample script; refuses a second `#load`, a `#foreign`, an
`#import`, an `#import_proto`, an oversize source, a `#run` that never ends,
and a script that names itself otherwise);
`coax/bin/coax test metaverse-engine/tools/script-job.coax --library`.
