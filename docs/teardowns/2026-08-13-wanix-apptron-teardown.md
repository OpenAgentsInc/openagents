# Wanix and Apptron Teardown — 2026-08-13

Read-only architecture and product audit of Tractor's in-browser compute
stack: the MIT `tractordev/wanix` kernel at
[`6594fe3763eb8712e81914f78b79243bb403f5cc`](https://github.com/tractordev/wanix/commit/6594fe3763eb8712e81914f78b79243bb403f5cc)
("9p: trim constant per-op round-trips on the ls path", committed
2026-08-01) and the unlicensed `tractordev/apptron` product at
[`c4cb2dba29dd245638b55674f6d38ea9cf97a92b`](https://github.com/tractordev/apptron/commit/c4cb2dba29dd245638b55674f6d38ea9cf97a92b)
(merge of Playwright editor-load tests, 2026-07-13). Local clones:
`~/work/projects/repos/wanix` and `~/work/projects/repos/apptron`. Nothing
in either tree was modified, built, or executed. No Apptron account, Hanko
session, v86 guest, or Cloudflare Worker was exercised.

Wanix is the substrate. Apptron is the product. Together they are the
strongest public evidence in this catalog for **a Plan 9 namespace as a
browser compute toolkit** and **a local-first Linux workroom that only
happens to be an IDE**. They overlap OpenAgents' workroom, sandbox, and
browser-compute questions more directly than any prior web-only subject —
and they are the wrong isolation class for production agent labor.

Evidence labels (per [README](./README.md)):

- **`[source]`** — observed in a commit-pinned source snapshot
- **`[schema]`** — encoded in a typed wire, storage, or config contract
- **`[docs]`** — stated by checked-in documentation
- **`[test]`** — encoded in a checked-in test or CI surface
- **`[history]`** — supported by Git history at or before the pin
- **`[public]`** — corroborated by a named public source, fetched 2026-08-13
- **`[vision]`** — stated as intended direction, not necessarily present
- **`[inferred]`** — concluded from several observations
- **`[limitation]`** — a boundary on what this source-only audit can prove

## TL.DR

Wanix is a **bind-table operating system compiled to Wasm**, not a syscall
kernel. A root `Task` plus a Plan 9-style namespace (`vfs.NS` over
`bind.Table`) exposes processes, terminals, VMs, pipes, signals, OPFS,
IndexedDB, DOM, and live JavaScript as files. Tasks clone the bind table.
Compute is a `TaskDriver`: Go/TinyGo Wasm (`gojs`), WASI in a SharedArrayBuffer
worker, a sketch JS worker, or (on the host) native `os/exec`. Linux is a
**v86 task**, not a hypervisor: virtio 9P mounts the host namespace into the
guest, and the guest can export its own namespace back. HTML custom elements
(`<wanix-namespace>`, `<wanix-bind>`, `<wanix-task>`, `<wanix-vm>`,
`<wanix-term>`, `<wanix-workbench>`) are the UX. Two wires speak one
namespace: Duplex/CBOR for JS and the VS Code workbench, 9P2000.L for WASI,
v86, iframe/WebSocket federation, and guest export. [source] [docs]

Apptron is a **product shell around Wanix, not a fork**. Alpine i386 boots
in v86 with a 9P root. A Cloudflare Worker plus one shared `Session`
container supply Hanko identity, an HTTP filesystem over R2, published
static sites on `aptn.pub`, and a single `10.0.0.0/8` virtual NIC that
turns guest LISTEN ports into `https://tcp-{port}-{iphex}-{user}.apptron.dev`.
Persistence is Docker-image-shaped: `/project`, `/home/$USER`, and `/public`
are IDBFS + `syncfs` to R2; everything else is `cowfs` over a bundled
rootfs and resets on reload. WASM magic bytes are registered with
`binfmt_misc` so guest `./hello.wasm` becomes a **host Wanix task**, not a
v86 process. [source] [docs]

```text
browser tab                                              cloud (Apptron only)
----------------                                         --------------------
wanix.min.js + wanix.wasm                                CF Worker (Hanko, R2
  root Task + vfs.NS                                       HTTP-FS, projects)
  #task #term #vm #web #ramfs #pipe                      R2 bucket as POSIX
  custom elements / VS Code web                            /usr /env /etc/index
       |                                                 one Session container
       | virtio 9P  (host NS -> guest /)                   go-netstack 10/8
       v                                                   /x/net slirp +
  v86 Alpine i386                                          tcp-*-*.apptron.dev
  /project /home /public  <--syncfs 2-5s-->  httpfs /data
  apk, go 1.25, aptn ports, wexec -> #task
```

The five most important findings:

1. **Isolation is a cloned bind table plus the browser origin, not a
   capability kernel and not a microVM.** `TaskFS.Alloc` snapshots the bind
   table; `#ramfs` / `#pipe` / `#signal` allocate fresh backing stores;
   everything else is a shared `fs.FS`. Same-origin page JavaScript can
   `_open9P("1")` and see the whole root. `#js` reflects `globalThis` with
   "security sandboxing policy (left to caller)." v86 is an emulator. There
   is no uid, no seccomp, no grant, no receipt. [source]
2. **The persist-versus-image split is the product idea worth taking.**
   Workspace, home, and published site are durable. The environment image is
   ephemeral unless an explicit `.apptron/envbuild` overlay is committed.
   That is the same four-resource honesty Ascii Box almost has (filesystem
   vs process vs ingress vs session) expressed as mounts. [source] [docs]
3. **Public ingress is real and unauthenticated.** Guest DHCP on a shared
   `go-netstack`, `aptn ports` watching `/proc/net/tcp` state `0A`, Worker
   paths `/x/net` and `tcp-*` forwarded with no Hanko check, WebSocket
   `CheckOrigin: return true`. Knowing the URL is enough while the tab
   lives. Cross-tab "session IPs are routable" is one shared NIC in
   `max_instances = 1`, not a mesh of isolated networks. [source] [schema]
4. **Wanix is MIT and reusable as a library. Apptron has no license.**
   GitHub reports `license: null` for Apptron; there is no `LICENSE` on
   `main`. Some third-party pages say MIT; that is not in the tree. Treat
   Wanix as a pattern-and-optionally-pinned-library donor. Treat Apptron as
   a pattern donor only. [source] [public] [limitation]
5. **This is not OpenAgents containment and not a desktop direction.**
   Firecracker/jailer on GCE, the managed-sandbox contract, and Omega/Zed
   remain the isolation and IDE authorities. Harvest the namespace
   composition, the 9P guest/host bridge, the HTTP-FS/R2-FS protocol, the
   image-versus-workspace split, and the "WASM runs on the host kernel, not
   in the emulator" trick. Reject v86-as-sandbox, the shared NIC, query
   tokens, hardcoded `progrium` admins, VS Code web as a product shell, and
   any claim that browser Linux is a workroom.

## 1. Identification and scope

### 1.1 Exact source identity

| Field | Wanix | Apptron | Evidence |
| --- | --- | --- | --- |
| Repository | `tractordev/wanix` | `tractordev/apptron` | [public] |
| Homepage | https://wanix.dev | https://apptron.dev | [public] |
| Audited commit | `6594fe3763eb8712e81914f78b79243bb403f5cc` | `c4cb2dba29dd245638b55674f6d38ea9cf97a92b` | [source] |
| Commit time | 2026-08-01 (author 2026-07-31) | 2026-07-13 12:02 -0700 | [history] |
| Subject | `9p: trim constant per-op round-trips on the ls path` | Merge PR #308 Playwright editor-load | [history] |
| License | MIT, Copyright 2023 Jeff Lindsay | **None in tree.** GitHub `license: null` | [source] [public] |
| Module | `tractor.dev/wanix`, Go 1.26 | `apptron.dev`, Go 1.25.0 | [source] |
| Product version | npm `wanix` `0.4.0-beta`; extras `0.4.0-rc2`; tag `v0.4-preview` (2026-07-17) | `version.txt` `0.7.0`; tag `v0.7.0` | [source] [history] |
| Scale | ~250 Go files / ~43k lines excluding vendored `misc/cbor`; 33 `_test.go`; 14 HTML examples | 8 Go files / ~1.6k lines; 17 TS files; Playwright only | [source] |
| History | 769 commits; default-branch rewrite for the v0.3 Plan 9 redesign; earliest remaining tag `v0.1` (2024-02-10). Repo created 2023-10-15 | 322 commits on this history; first commit 2024-07-17 ("README: add") with no parents. Repo created 2021-12-14; `origin/legacy` is a different desktop product | [history] [public] |
| Maintainers | Jeff Lindsay 756 / Joël Franušić 5 / others single digits | Jeff Lindsay 273 / Tara Kirkland 49 | [history] |
| Traction | 815 stars, 42 forks | 1,206 stars, 37 forks | [public] 2026-08-13 |
| Positioning | "A compute toolkit for the web with the depth of Plan 9 from Bell Labs." "Agent sandboxes: utilize browser sandboxing." | "Local-first development platform." "It also only happens to be an IDE… similar to Smalltalk." | [docs] |

Wanix's HEAD commit carries `Co-Authored-By: Claude`. The README's AI
disclosure is honest and stricter than most of this catalog: components
were written with assistance; "vibe coded" PRs are refused; a human must
know how the PR works. [docs] [history]

Apptron pins `tractor.dev/wanix v0.0.0-20260206032626-2b310a9cbd1a` — a
February 2026 Wanix, not the August pin. Runtime JS is copied from
`ghcr.io/tractordev/wanix:runtime` into `assets/wanix.min.js`. `make wanix`
clones the kernel for a local `go.work`; that clone is gitignored. Apptron
is a consumer, not a fork. [source]

Simon's December 2025 [research note](https://github.com/simonw/research/blob/main/apptron-analysis/README.md)
is cited from the Apptron README. It is a useful map and is stale against
this SHA (Playwright suite, share/embed, envbuild, and several worker
paths landed later). [docs] [public] [limitation]

### 1.2 Limits

Nothing booted. v86 performance, 9P latency, syncfs conflict behavior,
R2 consistency, and the live `apptron.dev` topology are unread. Source
proves intended implementation at these commits. It does not prove every
path is exercised in production. [limitation]

## 2. Wanix: a bind-table OS in the browser

### 2.1 Kernel shape

Wanix is not a Unix kernel and not a hypervisor. The "kernel" is:

- a root `*Task` allocated by `wanix.NewRoot()` / `NewRootWithTasks`
- a `*vfs.NS` wrapping an atomic-snapshot `bind.Table`
- `#` device filesystems bound into that namespace
- a Wasm `main` (`wasm/wasm.go`) that installs `_openPort`, `_open9P`, and
  `_setupNamespace` on the JS `WanixKernel` and then `select {}`

```text
<wanix-namespace>                 WanixKernel (elements/kernel.js)
        |                              window.__wanix[id]
        | fetch wanix.wasm
        v
  wanix.NewRoot()
    #wanix/version
    #task   TaskFS
    #term   term.Device
    #vm     vm.Device
    #web    console, dom, caches, worker, dl, opfs
    #pipe   Allocator (fresh duplex per bind)
    #signal Allocator (fresh broadcast per bind)
    #ramfs  allocfs -> memfs
    #js     jsfs over globalThis
    #httpfs #idbfs #cowfs   factories, not in the README table
```

`#` is just a destination path. Paths do not start with `/`. `.` is the
namespace root. [docs] [source]

A `Task` is shaped like a process — `cmd`, `args`, `env`, `dir`, fds, an
optional `TaskDriver`, an optional export `fs.FS` — and is controlled by
writing files: `#task/new/<kind>` allocates; `ctl` accepts `start` /
`bind` / `unbind`; `cmd`, `env`, `dir`, `alias`, `exit` are field files.
If the parent is set, `Alloc` does `parent.ns.Clone(ctx)`: a copy-on-write
snapshot of the bind table, not a live shared table. Parent and child
still share backing `fs.FS` objects unless the source implements
`BindAllocator` (ramfs, pipe, signal, httpfs, idbfs, cowfs). Binding
`#ramfs/new` is the documented "fresh ramfs" trick. [source] `task.go`,
`fs/vfs/vfs.go`, `fs/bind/table.go`

`UnbindAll` keeps `#…` bindings. Child-task `_setupNamespace` currently
does **not** call `UnbindAll`; the unbind-all plus `fsys` base path is
commented out in `wasm/wasm.go`. A child inherits whatever the parent
table held at clone time, plus its own binds. [source] [limitation]

### 2.2 Filesystem: Plan 9, then not

`tractor.dev/wanix/fs` is an extended `io/fs` (`BindFS`, `UnbindFS`,
`RouteFS`, `OpenContextFS`, xattr). The bind table is an
`atomic.Pointer` snapshot; writers copy-and-swap. Placement options are
`after` (default), `before`, `replace`. [source]

The documented invariant versus Plan 9, written into `fs/vfs/vfs.go`:

> Directory unions are recursive: when multiple bindings share a bind
> point, Open merges directory listings at every descendant path where
> those trees overlap. This differs from Plan 9, where union applies
> only at the bound name itself.

Writes `Route` to the first `CreateFS` member. Merged views are produced
in `Open`. That is a real design choice, not an accident, and it is the
kind of thing a port must not "fix" back to Plan 9 without noticing.
[source]

| Package | Role |
| --- | --- |
| `fs/memfs` | In-memory tree. `#ramfs` factory |
| `fs/cowfs` | Read-only base + writable overlay, tombstones, rename-chain collapsing, whiteouts |
| `fs/fskit.UnionFS` | Read-only union. Used by `#term`, `#vm`, workers |
| `fs/localfs` | Host directory. CLI `wanix serve`, guest `hostexport` |
| `fs/httpfs` | POSIX-over-HTTP. Spec'd headers: `Content-Mode`, `Content-Modified`, `Content-Ownership`, `application/x-directory` |
| `fs/r2fs` | Same metadata model on Cloudflare R2. Separate `go.mod`. Used by `site/`, not bound in the Wasm kernel |
| `fs/p9kit` | 9P2000.L client (`ClientFS`) and server (`Attacher`) over `github.com/progrium/p9` |
| `fs/fusekit` | go-fuse v2 host mount. Native only |
| `fs/syncfs` | Local-first; debounce-patch to a `RemoteFS` (`Index` / `Patch`) |
| `fs/tarfs` | Read-only tar. Archive binds copy into writable memfs |
| `web/fsa` | File System Access / OPFS via `navigator.storage.getDirectory` |
| `web/idbfs` | IndexedDB. Apptron prefers this over OPFS |
| `web/jsfs` | Live JS value graph as POSIX (`:obj :ref :json :type` suffixes) |
| `fs/pipe`, `fs/signal` | Duplex ports and broadcast files |

The HTTP-FS and R2-FS specs are the most portable artifacts in the tree:
a small, header-centric POSIX projection over object storage, with tar
PATCH, `/:attr/` xattrs, and directory listings as plain text. Apptron's
`worker/src/r2fs.ts` is the TypeScript twin. [schema] [source]

### 2.3 Task drivers and two wires

`web.New` registers three drivers on the root task:

| Kind | Check | Start |
| --- | --- | --- |
| `auto` | never matches | first registered driver whose `Check` is true |
| `gojs` | Wasm detect `"gojs"` | worker blob from `gojs/worker` |
| `wasi` | detect `"wasi"` | worker blob from `wasi/worker` + `@bjorn3/browser_wasi_shim` |
| `js` | arg0 ends `.js` | Blob URL of file contents. Marked "a sketch atm" |
| `native` / `exec` | not `.wasm`/`.js` | `os/exec` + pty. Host only (`hostexport`) |

Workers receive two MessagePorts: `_openPort` (Duplex/CBOR,
`api.PortResponder`, the `WanixHandle` JS API) and `_open9P`
(`p9kit.Attacher(task.NS())`). WASI needs the 9P port plus a
SharedArrayBuffer `CallBuffer` so blocking syscalls can be faked from an
async parent worker. [source] `web/web.go`, `web/worker/worker.go`,
`wasi/worker/worker.js`

That split is load-bearing. JS and the workbench want a high-level FS
RPC. WASI, v86 virtio, iframe federation, and guest export want 9P. One
namespace, two encodings. [inferred]

### 2.4 v86, guest export, hostexport

`<wanix-vm>` is not an in-kernel emulator. It allocates `#vm/new/v86`
and starts a **gojs task** running `#vm/v86/v86-vm.wasm` from the extras
archive bound at `#vm/v86`. The worker reads `v86.wasm`, SeaBIOS, VGA
BIOS, and `boot/bzImage`; virtio 9P forwards to the task's 9P port so the
**host namespace is the guest root**. On ready, an export MessagePort
carries guest 9P frames the other way. Host side: `p9kit.ClientFS` →
`wanix.Export(task)` and `vm.SetGuest(metacache.New(exportFS))` bound at
`#vm/<id>/guest`. [source] `elements/vm.js`, `v86/main.go`,
`web/worker/worker.go`

Guest Linux (`extras/linux/bin/init`) is busybox. Optional
`EXPORTDEV=/dev/$export hostexport &` runs a **native Wanix root inside
the guest** that 9P-exports `/` plus rebound `#task`/`#term`, and
registers `native.ExecDriver`. `wexec` is the guest helper that
allocates `/task/new/wasi` on that exported host task fs — Wasm that
looks like a guest binary actually runs as a host Wanix task. [source]
`extras/hostexport/main.go`, `extras/wexec/main.go`

Workbench `task-ns` / `term-ns` can point at `#vm/1/guest/#task` so the
editor lives on the host namespace while the integrated shell is the
guest. [docs] [source]

### 2.5 Federation, workbench, site

Export: set `id` and `allow-origins` on the kernel host. A
`postMessage({request: "wanix-import"})` whose `location.hash` matches
`id` and whose origin is on the list (or `*`) receives `_open9P("1")` —
the **root** namespace. Import: `<wanix-bind type="import"
src="https://host/app.html#id">` or `wss://…`. The import `postMessage`
target is `"*"`. `wanix serve` accepts any WebSocket upgrade as a 9P
server of the served directory and sets `CheckOrigin: return true`, plus
COOP `same-origin` / COEP `require-corp` / `Access-Control-Allow-Origin: *`.
[source] `elements/kernel.js`, `elements/bind.js`, `cmd/wanix/serve.go`

The workbench is VS Code Web **1.108.2** from
`progrium/vscode-web`, scheme `wanix:`, `WanixBridge` as
`FileSystemProvider`, workspace `trusted: true`, UI state in IndexedDB.
Service-worker binding in `web.New` is commented out. The `site/`
Cloudflare Worker (`*.wanix.site`, R2 `wanix-site`, Hanko SSO at
`io.wanix.site`) is optional hosting, not the runtime. [source]

`wanix` CLI on this pin is `wanix serve [dir]` only. v0.3 bundles and
related commands were dropped. [history] [source]

### 2.6 What Wanix isolation actually is

README claims: "Isolation by design. Each task gets its own namespace."
"Agent sandboxes: utilize browser sandboxing to isolate an agent
environment you construct." "No backend required." [docs] [vision]

What the code delivers:

- Browser origin isolation (OPFS, Cache, IndexedDB, fetch).
- Wasm + Worker threads (no DOM on the worker).
- A cloned bind table per task. Later parent binds are invisible. Shared
  backing stores remain shared.
- v86 as an emulator with the host NS **explicitly mounted** into the guest.

What is not a security boundary:

- Same-origin JS holds `kernel.root` and `_open9P("1")`.
- `allow-origins="*"` is documented and used in examples.
- `#js` exposes `globalThis` with no policy.
- `JSDriver` runs fetched JS as a module worker with a live FS handle.
- Workbench workspace is trusted.
- No capability tokens beyond "you have a MessagePort to this NS."
- COOP/COEP only on `wanix serve`, not on CDN static hosting.

A hostile agent with a handle to `#task/1` can bind `#js`, `#web/opfs`,
fetch, and any imported remote namespace. Isolation is **what you bind
into the task**, not a kernel MAC policy. That is an honest Plan 9
lesson. It is not an OpenAgents grant. [source] [inferred]

## 3. Apptron: the workroom product on Wanix

### 3.1 Boot

`boot.go` is `//go:build js && wasm`, built to `assets/wanix.wasm`. The
page (`assets/_env.html` + `assets/lib/apptron.js`) constructs
`WanixRuntime` from `/wanix.min.js`, points `network` at
`ws(s)://{host}/x/net`, preloads `/bundles/sys.tar.gz`, and loads the
Wasm. `window.apptron` must exist (`origin`, optional `user`, `env`,
`mode`, `embedded`, `publishURL`) or `main` fatals. [source]

Boot sequence, condensed from `boot.go`:

1. `wanix.New()`; bind `#web`, `#vm`, `#pipe`, `#commands`, `#|`, `#ramfs`.
2. `virtio9p.Setup(root.Namespace(), …)` so the guest 9P hook exists
   before the VM starts.
3. Bundle tar → `#bundle`. Environment =
   `cowfs{Base: bundle/rootfs or IDBFS custom root, Overlay: memfs}` as
   `#env`. Optional IDBFS overlay unions over the base.
4. Allocate `vm/new/default`. Bind `#env` and the host devices into
   `vm/{id}/fsys`. Write `etc/profile.d/apptron.sh` (`ENV_EMBED`, `USER`,
   `ENV_MODE`, `PUBLIC_URL`, `HOME`, `ENV_NAME`, `ENV_UUID`).
5. Start the VM immediately:

   ```
   init=/bin/init rw root=host9p rootfstype=9p
   rootflags=trans=virtio,version=9p2000.L,aname=vm/{id}/fsys,cache=none,msize=131072
   mem=1008M  -m 1G  -netdev user,type=virtio,relay_url={network}
   ```

6. Then, in parallel, sync home / project / public, bind a `ctl` file
   (`cmd`, `bind`, `reload`, `bundle`, `cp`, `sync`), and `_wasmReady`.
   The goroutine then **blocks on serving 9P**.

IDBFS is a deliberate OPFS rejection:

> IDBFS is still origin-private if not exactly OPFS. Not only does it
> work in older Safari, but it's 50% faster than OPFS.
> `opfs := idbfs.New("apptron-rev1")`

[source] `boot.go:204–206`

### 3.2 Linux guest

The rootfs is `i386/alpine:3.22` plus `fuse make git esbuild`, a
`GOARCH=386` `aptn`, and `system/bin/*`. The kernel is a custom i386
`bzImage` from `ghcr.io/tractordev/apptron:kernel` with `CONFIG_9P_FS`,
`CONFIG_NET_9P_VIRTIO`, `CONFIG_VIRTIO_NET`, `CONFIG_BINFMT_MISC`,
`CONFIG_FUSE_FS`, `CONFIG_SHMEM`. Netfilter/NAT off. ASLR bits 0.
[source] `system/kernel/kernel.config`, `Dockerfile`

`/bin/init`:

- mount proc/sys/binfmt_misc
- register Wasm: `':wasm:M::\x00asm::/bin/wexec:'`
- source `/etc/profile` (which loads `apptron.sh`)
- if not embed and `USER` is set: `udhcpc -i eth0 -s /bin/post-dhcp`,
  export `SESSION_IP`
- if `.apptron/envbuild` (or `.envbuild`) is newer than the IDBFS
  overlay: `exec /bin/rebuild`
- first run: `open /apptron/WELCOME.md`
- `aptn ports &` (`aptn fuse` is commented out)
- `exec /bin/start` → banner, envrc, interactive shell on `ttyS0`

`/bin/wexec` is `exec /bin/aptn exec`. `aptn exec` talks 9P to the host
Wanix `#task` allocator and runs the Wasm as `wasi` or `gojs`. Guest
`go build` of a Wasm binary, then `./hello`, does **not** execute inside
v86. It escapes to the browser kernel. That is the "first environment
that lets you write and compile Go entirely in-browser" claim, and it is
why Go 1.25 is shipped as brotli bundles (`goroot`, `gocache-386`,
`gocache-wasm`) mounted via `source /etc/goprofile` rather than `apk add
go`. [source] [docs]

`.apptron/envrc` runs every session. `.apptron/envbuild` chroots into
`/apptron/.buildroot`, copies the result to
`web/idbfs/apptron/env/$ENV_UUID/overlay`, and `ctl reload`. That is the
"commit the image" path. It is experimental and file-mtime gated.
[source] `system/bin/rebuild`

### 3.3 Three durable mounts, everything else is a layer

| Path | Local | Remote | Interval | Who |
| --- | --- | --- | --- | --- |
| `/home/$USER` | IDBFS `usr/{userID}` | `httpfs $origin/data/usr/{userID}` | 5s | signed-in user |
| `/project` | IDBFS `env/{uuid}/project` (owner) or memfs (viewer) | `httpfs $origin/data/env/{uuid}/project` | 5s | owner read/write; public GET |
| `/public` | IDBFS `env/{uuid}/public` | `httpfs $origin/data/env/{uuid}/public` | 2s | owner only |
| rest of `/` | `cowfs` over bundle/custom root + memfs scratch | none | — | resets every pageload |

Non-owners of a public project get a **memfs** project: they can edit in
the tab; nothing lands in their IDBFS; R2 stays GET-only. Admins
(`[]string{"progrium"}` in **both** `boot.go` and
`worker/src/config.ts`) additionally bind a cached `httpfs` of
`$origin/data` at `root/data`. [source]

Publish is `ctl sync` from project (or a subpath) onto `/public`, then
the Worker serves `{username}.aptn.pub/{envName}` from R2
`/env/{uuid}/public` with `index.html` fallback, a generated
`offline.js` service worker, and `404.html`. The Share UI's "invite"
copy has **no invite API**. Private means owner-only; everyone else
gets **404**, not 403, by design. [source] [test] `tests/auth.test.ts`

### 3.4 Cloud: identity, object FS, one NIC

`wrangler.toml` routes `*.apptron.dev`, `apptron.dev`, `*.aptn.pub`.
Assets bind to `./assets`. One Cloudflare Container:

```
[[containers]]
class_name = "Session"
image = "./Dockerfile"
max_instances = 1
instance_type = "standard-4"
```

`Session extends Container { defaultPort = 8080; sleepAfter = "2h" }`.
The Worker forwards **without auth** when `ctx.portDomain`, path
`/x/net`, or path `/bundles/`. Everything else is Worker TypeScript:
Hanko session validate, project CRUD, R2 HTTP-FS under `/data`,
dashboard/shell gates, publish host. [schema] [source]
`worker/src/worker.ts`

R2 layout is the directory of record:

| Key | Role |
| --- | --- |
| `/etc/index/{username}` | username → user uuid |
| `/etc/index/{username}/{project}` | project metadata: uuid, visibility, description, publish_source |
| `/usr/{user_id}` | home |
| `/env/{uuid}` | name, owner, ownername |
| `/env/{uuid}/project` | project files |
| `/env/{uuid}/public` | published site |

`/data/usr/{uuid}` requires matching JWT `sub`. `/data/env/*` writes are
owner-only; public projects are GET/HEAD for others. `/etc`, `/data`
roots, and `/:attr/` are admin-username only. CORS on the Worker is
`Access-Control-Allow-Origin: *`. [source]

Auth is Hanko passkeys. The Worker injects `<meta name="auth-url">`.
Tokens arrive as `?token=`, cookie `hanko=`, or `Authorization: Bearer`.
`validateToken` POSTs to `{hanko}/sessions/validate`. JWT claims
(`sub`, `username`) are `atob`-decoded without verify and used only
after `validateToken` returns true. Cookie domain is `.{appHost}` off
localhost. [source] `worker/src/auth.ts`, `assets/lib/apptron.js`

"Local-first" and "does not depend on the cloud" are README claims. The
implementation is **hybrid**: compute and the editor run in the tab;
identity, the project index, multi-device sync, bundle bytes, and the
virtual NIC depend on Cloudflare. An offline tab with a warm IDBFS can
edit; it cannot DHCP, cannot `apk add`, cannot publish, cannot open a
private project on a new device. [docs] [inferred]

### 3.5 Virtual network and public URLs

`worker/cmd/worker/main.go` constructs one
`github.com/progrium/go-netstack` network:

```go
vnet.New(&vnet.Configuration{
    Subnet:            "10.0.0.0/8",
    GatewayIP:         "10.0.0.1",
    GatewayMacAddress: "5a:94:ef:e4:0c:dd",
})
```

`/x/net` is a WebSocket that `vn.AcceptQemu`s via a 4-byte
length-prefixed adapter (`CheckOrigin: return true`). Guest `udhcpc`
gets a `10.x` address. `aptn ports` polls `/proc/net/tcp` for LISTEN
(`0A`) and prints

```
https://tcp-{port}-{ipv4hex}-{user}.apptron.dev
```

The Worker classifies `tcp-` subdomains as `portDomain` and reverse-proxies
HTTP onto `vn.Dial("tcp", ip:port)`. Non-HTTP is the same NIC over the
WebSocket. The WELCOME doc tells you to `apk add apache2 && httpd
-DFOREGROUND` and click the URL. The URL works for anyone on the
internet **as long as the tab is running**. [source] [docs]

README: "Session IPs are routable to each other, allowing communication
across browser tabs and devices." True only because every tab attaches
to the **same** `vnet` in the **one** Session container. There is no
per-user, per-project, or per-tab network. [inferred] [schema]

### 3.6 VS Code, `aptn`, tests

VS Code Web **1.103.2** (older than Wanix's own 1.108.2 extras) from the
same `progrium/vscode-web` release line. Extension
`progrium.apptron-system` registers scheme `wanix` with root prefix
`vm/1/fsys`, folder URI `wanix:/project`, PTY on `#console/data`, and a
command stream on `#commands/data1` (`open-vm`, `open-file`,
`open-folder`, `open-preview`). A monitor webview attaches the v86
canvas. Workspace is trusted; Tractor Dark; activity bar and status bar
hidden. The preview extension is a generic iframe, defaulting to Google,
not wired to session-IP URLs. [source]
`extension/system/src/web/{extension,bridge}.ts`, `assets/lib/vscode.js`

`aptn` commands: `exec`, `fuse` (shm 9P → FUSE `/x`, **not started**),
`ports`, `shm9p` (guest `/` over shared memory; host client in `boot.go`
is **commented out**), `shmtest`. The interesting 9P/FUSE host bridge is
staged and off. Virtio 9P is the shipped bridge. [source]

Playwright against `wrangler dev :8788`: Mailinator + virtual WebAuthn +
Hanko signup, then dashboard heading, terminal `echo` → explorer, public
share in a fresh context, private URL 404 anonymous. No unit tests for
`r2fs`, `vnet`, or `boot.go`. [test] [limitation]

### 3.7 Security posture, product-level

On top of Wanix's already-weak isolation:

- One shared container, one `10.0.0.0/8`, no per-tenant net.
- Port ingress unauthenticated.
- Tokens in query strings.
- Admins hardcoded in client Wasm and Worker.
- Public project = world-readable R2 tree of `/project`.
- Embed CSP is `frame-ancestors *`. The iframe `sandbox=` attribute is
  commented out.
- COOP/COEP exist in `assets/_headers` and are commented out on the
  Worker HTML path.
- No guest secret store. Secrets would live in persisted home/project
  (IDBFS + R2).
- Opening a public project runs that project's guest code **in your
  browser**. v86 is not a sandbox against a malicious project.

This is a personal compute environment and an embeddable playground. It
is not a multi-tenant agent execution plane. [source] [inferred]

## 4. What OpenAgents should take

### 4.1 Harvest

1. **Namespace composition as the workroom filesystem law.** One bind
   table, `#` devices, cloned tables per task, union-at-every-level
   documented as a deliberate non-Plan-9 choice, allocate-on-bind for
   ramfs/pipe/signal. OpenAgents already owns grants and receipts; the
   missing piece this stack demonstrates is a **composable filesystem
   view** that makes "what this agent can see" a bind list, not a
   prompt paragraph. Adapt the law. Do not vendor the Go NS.
   [inferred]

2. **Image versus workspace versus home versus ingress.** Four
   resources, four lifetimes. Ascii Box almost says this; Apptron
   implements it as mounts. A workroom snapshot that mixes apk state,
   `/project`, the agent's home, and a live port is a lie. Keep them
   separate on the OpenAgents workroom and managed-sandbox seams.
   [inferred]

3. **HTTP-FS / R2-FS as a POSIX-over-object-store protocol.** Spec'd
   headers, tar PATCH, directory listings, xattrs. Useful as a
   compatibility target or an internal sync projection over object
   storage — beside, not instead of, OpenAgents event/receipt
   authority. Wanix MIT-licenses the Go side. [schema]

4. **9P as a guest/host/remote FS pipe, not as identity.** virtio 9P
   root, guest export, iframe/WebSocket import, `wanix serve`. The
   protocol is the interesting part. The origin policy (`*`,
   `CheckOrigin: true`) is not. If OpenAgents ever exposes a
   namespace to a browser or a microVM, the attach is a grant, the
   origin list is fail-closed, and the receipt names the NS
   generation. [inferred]

5. **WASM on the host kernel, not in the guest.** `binfmt_misc` →
   `wexec` → `#task`. The emulator runs Linux userspace; language
   toolchains run where they are fast. That split is how Apptron makes
   in-browser Go tolerable. For OpenAgents it is a reminder that
   "the sandbox" and "the compiler" need not be the same machine —
   and that escaping the guest is a feature only when the host is
   still contained. In Apptron the host is the page. In OpenAgents
   the host must remain the receipted workroom or Firecracker guest.
   [inferred]

6. **Local-first sync with an honest remote directory of record.**
   IDBFS (or OPFS) first, debounce 2–5s, `syncfs` patch to HTTP-FS.
   Non-owner viewers get memfs. Conflict policy is underspecified
   (`TODO` in `syncfs.go`) and must not be copied. The shape — local
   write, remote reconcile, viewer isolation — is right. [source]

### 4.2 Reject

- **v86 / 32-bit Alpine as an agent sandbox or workroom isolation
  class.** No KVM, no jailer, no uid, host NS mounted in. The
  Firecracker provisioner, GCE managed-sandbox contract, and
  `docs/cloud/INVARIANTS.md` stay authoritative. Browser Linux is a
  demo and a personal compute environment.
- **VS Code web as an OpenAgents surface.** Omega/Zed is the desktop
  and IDE destination. Wanix's own workbench and Apptron's
  `wanix:` provider are evidence that a custom FS scheme plus a PTY
  is enough to drive a web editor — useful if a browser workroom
  viewer is ever scoped, not a reason to adopt vscode-web.
- **The shared `go-netstack` NIC and unauthenticated
  `tcp-*-*.apptron.dev` ingress.** Powerful playground. Illegal as a
  multi-tenant or agent-ingress design. Sarah's explicit-dispatch
  plane and the managed-sandbox network allow-list are the opposite
  shape.
- **Apptron source.** No license. Do not copy. Do not vendor
  `boot.go`, the Worker, or the Alpine bits.
- **Hardcoded admins, query tokens, CORS `*`, `frame-ancestors *`,
  trusted workbench, `#js` as ambient authority.**
- **"No backend required" as a platform claim.** True for a Wanix
  CDN demo. False for Apptron the product. OpenAgents should keep
  saying where the directory of record lives.
- **FUSE/`shm9p` as a shipped host bridge.** Staged, commented out.
  Virtio 9P is what actually runs.

### 4.3 Where this sits in the catalog

| Neighbor | Relationship |
| --- | --- |
| [Ascii Box / Optibox](./2026-07-19-ascii-box-optibox-openagents-gcp-analysis.md) | Closest product-shape cousin: persistent machine, snapshot, stop/resume. Apptron's snapshot is three mounts + an optional overlay, not a box archive. Keep the OpenAgents GCP substrate. |
| [Crabbox](./2026-07-13-crabbox-teardown.md) | Execution-infrastructure seam. Crabbox leases real runners. Wanix/Apptron leases a browser tab. Do not confuse them. |
| [OpenChamber](./2026-07-12-openchamber-product-teardown.md) | Persistent coding workroom. OpenChamber's continuation is server-owned; Apptron's compute dies with the tab. |
| [Zed / Omega](./2026-07-18-zed-teardown.md) | IDE authority. This stack does not challenge it. |
| Firecracker lane in cloud invariants | Real isolation. v86 is not a candidate provisioner. |

## 5. Central decision

**Treat Wanix as a pattern donor and an optional MIT library for
namespace experiments. Treat Apptron as a pattern donor only. Do not
adopt either as an isolation class, a workroom runtime, a desktop, or
an ingress plane.**

If a later program wants a browser-attached playground — docs demos,
embeddable reproductions, a "try this workroom in a tab" viewer — Wanix
custom elements plus a fail-closed import origin list are the starting
shape, on OpenAgents grants, with no shared NIC and no Apptron code.
Production agent labor stays on the receipted host and the Firecracker
or GCE managed-sandbox path.

The dated teardowns remain evidence. The Sol roadmap, Desktop
guarantees, and cloud invariants own the requirement.
