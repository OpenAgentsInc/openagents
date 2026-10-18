# archil

A Rust port of Archil's official TypeScript SDK (the [`disk`](https://www.npmjs.com/package/disk)
npm package, v1.9.0, source at <https://github.com/archil-data/archil-sdk>).

* Async on `tokio`; HTTP via `reqwest` with `rustls` (ring) — no OpenSSL, no `unsafe`
  (`#![forbid(unsafe_code)]` is set in `Cargo.toml`), edition 2024.
* `serde` types for every request and response; `tokio-tungstenite` for the sandbox process WebSocket.

```rust
use archil::{Archil, CreateSandboxRequest, ProcessStartOptions};

#[tokio::main]
async fn main() -> archil::Result<()> {
    // Reads ARCHIL_API_KEY and ARCHIL_REGION (or pass ClientOptions explicitly).
    let client = Archil::from_env()?;

    let mut sandbox = client.sandboxes().create(CreateSandboxRequest::new()).await?;
    let result = sandbox.exec("uname -a", ProcessStartOptions::new()).await?;
    println!("{}", result.stdout);

    sandbox.stop().await?;
    sandbox.delete().await?;
    Ok(())
}
```

`cargo run --example claude_code` creates a sandbox, installs and runs Claude Code in it,
streams the output, and deletes the sandbox (see [Gaps](#gaps-and-things-not-verified) about this example).

## Coverage

| Area | TypeScript | Rust |
| --- | --- | --- |
| Client config | `new Archil({apiKey, region, baseUrl, s3BaseUrl, tls})`, env fallbacks `ARCHIL_API_KEY`, `ARCHIL_REGION`, `ARCHIL_S3_BASE_URL`, `key-` prefix handling, region table, S3 URL derivation | `Archil::new(ClientOptions)`, `Archil::from_env()`, `ClientOptions::{api_key, region, base_url, s3_base_url, ca_pem, poll_interval}`, `resolve_base_url`, `derive_s3_base_url`, `REGION_URLS` |
| Module-level functions | `configure`, `createDisk`, `listDisks`, `getDisk`, `createSandbox`, `listSandboxes`, `getSandbox`, `listApiKeys`, `createApiKey`, `deleteApiKey`, `exec`, `workspace` | `archil::{configure, client, create_disk, list_disks, get_disk, create_sandbox, list_sandboxes, get_sandbox, list_api_keys, create_api_key, delete_api_key, exec, workspace}` |
| Disks | `disks.create/list/listPage/get/exec` | `client.disks().create/list/list_page/get/exec` |
| Disk | `addUser`, `removeUser`, `createToken`, `removeTokenUser`, `listDelegations`, `revokeDelegation`, `createBranch`/`listBranches`/`getBranch`, `getAllowedIPs`/`setAllowedIPs`/`addAllowedIP`/`removeAllowedIP`, `delete`, `exec`, `grep`, `share` | same names in snake_case on `Disk` |
| Objects (S3) | `getObject`, `headObject`, `objectExists`, `putObject` (auto multipart), `appendObject`, `deleteObject`, `deleteObjects`, `listObjects`, `listObjectsPages`, `multipart.{create,uploadPart,complete,abort,listParts,listUploads}`, POSIX `mode`/`uid`/`gid`, `effectiveUploadPartSize` | `get_object`, `head_object`, `object_exists`, `put_object[_with]`, `append_object[_with]`, `delete_object`, `delete_objects`, `list_objects`, `list_objects_pages`, `multipart().{create,create_with,upload_part,complete,abort,list_parts,list_uploads}`, `PosixCreateAttrs`, `effective_upload_part_size` |
| Serverless exec / grep | `archil.exec({disks, command})`, `disk.exec`, `disk.grep` | `client.exec(ExecOptions)`, `disk.exec`, `disk.grep(GrepOptions)` |
| Workspace | `archil.workspace({...})`, `addDisk`, `removeDisk`, `diskNames`, key routing, fan-out `listObjects`/`grep`, `exec`; `FileSystem` interface | `Workspace`, `WorkspaceMount`, `FileSystem` trait (implemented by `Disk` and `Workspace`), `to_segments` |
| Sandboxes | `create` (wait), `list({disk})`, `get` | `client.sandboxes().create/create_with/list/get` |
| Sandbox lifecycle | `refresh`, `start({wait, mounts})`, `stop`, `pause` (`SandboxPauseError`), `resume`, `fork({name, wait})`, `setTimeout`, `delete`, `toJSON` | `refresh`, `start[_with]`, `stop[_with]`, `pause[_with]`, `resume[_with]`, `fork[_with]`, `set_timeout`, `delete`, `to_response` |
| Sandbox networking | `exposePort`, `listPorts`, `unexposePort`, `createPortToken`, `getPortToken`, `listPortTokens`, `listPortTokensPage`, `deletePortToken`, `getNetwork`, `updateNetwork`, egress policy/rules/transforms, mounts | same names in snake_case; `SandboxNetwork`, `SandboxEgressPolicy`, `SandboxEgressRule`, `SandboxMountSpec` |
| Processes | `run`, `attach`, `exec`; `SandboxProcess.{id,cursor,status,stdout,stderr,connected,sendInput,resize,closeStdin,disconnect,wait,kill}`, `onOutput`, `collectOutput`, `terminal`, `cwd`, `env`, `timeoutSeconds` | `run`, `attach`, `exec`; `SandboxProcess::{id,cursor,status,stdout,stderr,connected,send_input,resize,close_stdin,disconnect,wait,kill,next_output}`, `on_output`, `collect_output`, `stream_output`, `terminal`, `cwd`, `env`, `timeout_seconds` |
| Sandbox files | `files.uploadFile`, `files.downloadFile` | `files().upload_file`, `files().download_file` (any `AsyncWrite`), `download_bytes`, `download` (pull `FileDownload::next_chunk`) |
| Images | `images.build` (poll, interrupted retry, timeout), `images.get` | `client.images().build/build_with/get` |
| API keys | `tokens.list/create/delete` | `client.tokens().list/create/delete` |
| Errors | `ArchilError`, `ArchilApiError`, `ArchilS3Error`, `ImageBuildError`, `SandboxFileTransferError`, `SandboxPauseError` | `archil::Error` enum with `Api`, `S3`, `ImageBuild`, `FileTransfer`, `SandboxPause`, generic `Archil{message,status,code}` (e.g. `IMAGE_BUILD_TIMEOUT`), plus `Config`, `InvalidArgument`, `Transport`, `Process`, `Decode`. `Error::status()` / `Error::code()` mirror `ArchilError.status/.code`. |

### Behaviors carried over from the TypeScript SDK

* **Retries.** Three retries with full-jitter exponential backoff (100 ms base, 2 s cap) and the same three modes:
  *transient* (reads and idempotent writes: transport errors and 429/500/502/503/504), *connect*
  (creations, execs, deletes: only when no connection was established), and none. S3 `append_object` and
  multipart `complete` are never retried; branch creation reuses one `Idempotency-Key` across retries.
  The WebSocket connection is retried up to three times, requesting a fresh signed URL each attempt.
* **Pagination.** `disks().list` follows `nextCursor` in pages of 100, honours `limit`, truncates servers that
  ignore it, and stops on a repeated cursor; a `name` filter returns one page. `list_objects` follows
  continuation tokens (stopping on repeats); `list_port_tokens` follows cursors.
* **Waiting.** Create/start/resume poll (500 ms by default) until the sandbox leaves `pending`;
  stop/pause poll while `stopping`/`pausing`; pause failing raises `SandboxPauseError`; `fork` pauses a running
  source, forks the returned checkpoint, and resumes the source as soon as the fork is accepted.
  Image builds poll with backoff, re-request interrupted builds up to three times, and time out after 30 minutes.
* **Envelope handling.** `{success, data, nextCursor}` unwrapping and the exact error messages
  (`API request failed with status N`, `Unknown API error`), S3 XML error parsing into `S3Error`.

### Where the Rust API differs on purpose

* Optional-argument objects become builder-style structs (`ProcessStartOptions::new().cwd("/w")`), and
  `foo`/`foo_with(options)` pairs where the options are rare.
* Lifecycle methods on `Sandbox` mutate it in place (as in TypeScript) and so take `&mut self`.
* Streaming output: besides the `on_output` callback, `stream_output(true)` lets you pull chunks with
  `SandboxProcess::next_output().await`; the queue ends when the process exits or the connection drops.
* `Disk` derefs to its `DiskResponse` fields (`disk.id`, `disk.name`).
* `Workspace` takes `Disk` objects (not bare id strings) by construction, which is what the TypeScript SDK enforces at runtime.
* Debug output of secrets (`RegistryAuth.password`, `CreatedSandboxPortToken.token`) is redacted.

## Tests

```text
cargo test                                  # 128 tests against a mocked HTTP/WebSocket server
ARCHIL_API_KEY=... ARCHIL_REGION=aws-us-east-1 cargo test --test live -- --ignored --nocapture
```

Mock-server tests (`wiremock`, plus an in-process `tokio-tungstenite` server standing in for the sandbox runtime) cover
config, disks, API keys, the retry policy, objects/multipart, images, sandboxes, processes, files, workspaces and the
module-level functions. The live test (`tests/live.rs`) is `#[ignore]`d and also returns immediately unless both
`ARCHIL_API_KEY` and `ARCHIL_REGION` are set; it creates and deletes a disk and a sandbox.

## Gaps and things not verified

Not ported from the TypeScript package, and why:

* **`Disk.mount()`** — loads the platform-specific native addon `@archildata/native` (a Node `.node` binary for the
  data-plane client). It has no Rust equivalent in the SDK sources.
* **CLIs** (`disk`, `sandbox`, profiles, interactive shell, `bin/`, `src/cli/`) — executables, not part of the library API.
* **`internal/tools` and the framework packages** (`ai-sdk`, `langchain`, `mastra`, `eve`, `just-bash`, `sqlite`, `mock`) —
  separate npm packages that adapt the SDK to JavaScript agent frameworks.
* **Deprecated `sandbox.processes.{start,connect}`** and `Disk.dataSize` — marked "removed in the next version"; use `run`/`attach` and `active_data_bytes`.
* **Browser build, `ArchilTlsOptions` via Node `tls`, Undici HTTP/2 pooling and GOAWAY replay interceptor** — Node/JS transport
  details. `reqwest` negotiates HTTP/2 and pools connections itself (idle connections are retired after 30 s, as in the TS client),
  but GOAWAY-refused-request replay is whatever `hyper` provides, not a port of the interceptor.
* **Custom CA for the process WebSocket** — `ClientOptions::ca_pem` is applied to all HTTP (control plane and S3) traffic, but the
  `wss://` connection uses the built-in webpki roots.
* **Python-only behavior** — the Python SDK's `max_throttle_wait` retry on capacity 503s is not in the TypeScript SDK and was not ported.
* **Billing and activity endpoints** are in the OpenAPI schema (`@archildata/api-types`) but not in the TypeScript SDK, so they are not here.

Not verified, so treat accordingly:

* **Nothing was run against the real Archil API** — no credentials were available. Behavior is checked against the
  TypeScript sources and the OpenAPI types only; the live test has compiled but never executed. In particular the
  path `/mnt/archil/...` assumed by the live test and the wire format of WebSocket frames come from the TS source, not from a live server.
* **`examples/claude_code.rs` is not a copy of Archil's Claude Code guide.** `docs.archil.com` (including `llms.txt`) was not reachable from the
  build environment and a web search found no Archil page, so the example follows the task description (create a sandbox, run a command,
  stream output, delete it) using `npm install -g @anthropic-ai/claude-code` and `claude -p`. Check it against the guide before relying on it.
* **`cargo clippy` and `rustfmt` could not be installed** (the toolchain download was blocked), so neither was run; the code is only
  checked by `cargo build`/`cargo test` with no compiler warnings.
