# A realm's code from the browser v1 (`realm.code.v1`)

The editor shell that `?edit` opens over an engine page has a Code pane on the dev server
(`metaverse-engine/editor/shell.coax`, GitHub #13421). The pane lists the realm's own source
files, opens one, and saves it. This shape is what the shell asks the dev server and what the
server answers.

- Codec: `metaverse-engine/editor/code-wire.coax`. The shell writes an ask with `rc_ask` and reads
  the answer with `rc_answer_read` and `rc_file_at`. The dev server (`coax/web/serve.c`) reads the
  ask with `rc_ask_read` and writes the answer with `realm_code_head` and `realm_code_file_line`.
- Pinned bytes: `proto/world/code-v1.json`.
- Check: `coax/web/tool.sh metaverse-engine/tools/code-profile.coax proto/world/code-v1.json`.

## Where it is answered

Only `coax serve` answers it, and only while it watches (the dev server):
`POST /coaxial/code` on `http://127.0.0.1:PORT` or `http://localhost:PORT`.

- `--static` (a deployed page's server), the API and the realm origin have no such route.
- The browser must say the request is same-origin (`Sec-Fetch-Site: same-origin`), and the
  `Host` must be this computer. Another site, or another machine, gets `403`.
- A deployed page's shell is never told that code can be opened, so it asks nothing and shows
  no Code button.

Check: `coax/web/tool.sh code-route-check` (a fixture repository, a stand-in compiler, 21 cases).

## Asks

An ask is one line, and for a save the file's text after it.

| Ask | What it asks for |
| --- | --- |
| `realm.code.v1 list PAGE` | The realm's source files. |
| `realm.code.v1 read PAGE PATH` | One file's text. |
| `realm.code.v1 save PAGE PATH BYTES` then a line end and `BYTES` bytes | Write the file, then check it. |

`PAGE` is the page's module: `a-z`, `0-9` and `-`, a letter first, 32 bytes at most.

`PATH` is the file from the repository's root. `rc_path_ok` holds it to the page's own realm:

- It starts with `metaverse/PAGE/`.
- Its parts are letters, digits, `.`, `_` and `-`. No part starts with a dot, so there is no
  `..` and no hidden file. It does not start with `/`.
- It ends `.coax` or `.wgsl`.
- It is 160 bytes at most.

The server holds a path to more than its name. The file must be one the page's module was built
from (`APP/build/.drive/PAGE.sources`, or a source in the dependency list of the unit that writes
`PAGE.wasm` or `PAGE.wgsl`), and a plain file that is where its name says: a link is refused.

## Answers

| Answer | What it says |
| --- | --- |
| `realm.code.v1 files PAGE N` then `N` lines `f PATH BYTES` | The realm's files and their sizes, in order of path. A page that is not a realm's has none. |
| `realm.code.v1 file PATH BYTES` then a line end and `BYTES` bytes | The file's text. |
| `realm.code.v1 saved PATH 0` | The file was written, and the compiler's check found nothing. |
| `realm.code.v1 errors PATH BYTES` then a line end and `BYTES` bytes | The file was written. The bytes are the compiler's words. |
| `realm.code.v1 refused 0 WORDS` | Nothing was read or written. The words say why. |

A refusal has a status that is not 200: `400` for bytes that are not an ask (this includes a
path outside the realm), `403` for a request that is not this computer's own page, `404` for a
file that is not in the realm's list, `413` for a file longer than the editor opens.

## The check a save answers with

The server writes the file, then runs `coax check` on the module the file is part of, with the
resident compiler:

- a file under `metaverse/PAGE/gpu/` is checked through `metaverse/PAGE/gpu/PAGE.coax`;
- any other Coax file through `metaverse/PAGE/PAGE.coax`;
- a `.wgsl` file is not checked.

A part of a module is checked as the module loads it, never alone: alone, it would not know
the names its module gives it. The server starts this check once when the list is asked for, so
the compiler has seen the module before the first save. Measured on Splice Bots: the answer to a save
of a 104 KB file came back in 110 ms with errors and 684 ms without.

The file is written whether or not the check passes. The dev server's watch then builds the
module again. A build that fails keeps the last good engine running (`coax/web.md`).

## Limits

| Limit | Value |
| --- | --- |
| Bytes in a page's name | 32 |
| Bytes in a path | 160 |
| Files in a list | 96 |
| Bytes in a file the editor opens and saves | 196,608 (192 KB) |
| Bytes of the compiler's words in an answer | 4,096, cut at a line's end |

The Code pane says so when a file is longer than it opens, and when what is typed is longer
than it saves.
