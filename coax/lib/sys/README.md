# Tool libraries

What Coax command-line tools stand on (see [tools-in-coax.md](../../tools-in-coax.md)).
A tool is a Coax file with `tool_main :: () -> int #export_c`; run it with
`coax/web/tool.sh FILE.coax ARGS...` (or `coax/web/tool.sh NAME` for
`coax/web/NAME.coax`), which builds it with the platform effects and caches
the binary in `coax/web/build/tools/`. `coax/platform/tool-native.c` owns
`main`: it runs `tool_main` on a 1 GiB (lazily committed) stack, because
Coax keeps buffers in locals.

| File | What | Effects |
| --- | --- | --- |
| `args.coax` | argv copy, positional/flag/value reads, `args_number` (JavaScript `Number()` on plain decimals), `args_size` (`WxH`) | none (`#authority`) |
| `tool.coax` | `args_load`, stderr, the OS | `tool-native.c` |
| `fs.coax` | read/write/append whole files, kind, size, mkdir -p, temp dirs, recursive remove, sorted listing, cwd | `store-file-native.c` (`cxfs_*`) |
| `proc.coax` | spawn with args/env/cwd, quiet stdio, lines, wait with timeout, kill the group, `proc_run`, sleep, getenv | `process-native.c` |
| `../json/json.coax` | JSON parse into a node tree, key/index/path reads, unescape, writer | none (`#authority`) |
| `../text/base64.coax` | padded encode, lenient decode | none (`#authority`) |
| `../net/cdp.coax` | Chrome DevTools: launch headless Chrome, connect, commands, events | `proc`, `fs`, `socket-native.c` |

`coax test FILE --library` runs the pure checks; foreign effects do not run
while checking, so `coax/web/tool.sh coax/lib/sys/selftest.coax one "two words"`
exercises the real files, processes and argv and prints `sys selftest: ok`.
