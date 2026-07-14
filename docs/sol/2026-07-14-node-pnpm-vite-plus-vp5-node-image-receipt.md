# Node, pnpm, and Vite Plus VP-5 Node image receipt

- Class: receipt
- Date: 2026-07-14
- Status: complete for source, build, and retained runtime definitions
- Dispatch: no; closed evidence for #8799
- Parent: #8777

## Result

All retained JavaScript service Dockerfiles and build/deploy helpers now use
the pinned Node runtime and pnpm/Vite Plus build artifacts. Bun base images,
installers, preload commands, and runtime entrypoints are absent. The retained
Node images cover AIUR, the API Cloud Run service, Start, updates, audio,
audio-edge, queue worker, Live Hub, capture, QA, and acceptance surfaces.
Sites and their image definitions were deleted under the owner-authorized
retirement boundary.

Build stages may contain pnpm and Vite Plus; runtime stages execute built Node
artifacts. The repository build proved the web bundles, Worker dry-run,
Desktop bundle, Node service bundles, and public CLI outputs. Existing Node
runtime smokes cover HTTP/WebSocket readiness and shutdown, SQLite, subprocess
drain behavior, and installed executable help paths.

The local Docker CLI had no running daemon, so this receipt does not claim a
local OCI daemon build. That host limitation caused no source exception: every
tracked retained Docker `FROM`/entrypoint is statically Node-native, and the
same compiled entrypoints passed the non-container build/runtime gates.
Rollback remains the last published pre-cutover commit plus the service's
existing Cloud Run revision mechanism; no deployment was performed here.
