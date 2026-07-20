# @openagentsinc/ide-runtime

Reserved root package for the portable IDE runtime schemas and pure services.
Packet AFS-05 extracts the project, root, worktree, attachment, generation,
context, cursor, and proposal vocabulary here from the current Desktop IDE
contracts. AFS-00 reserves the package, its manifest, its export map, and its
import boundary. It owns portable IDE schemas and pure services. It must not own
platform adapters.

- Owner boundary: `docs/sol/2026-07-20-apple-fm-router-to-full-agent-system-plan.md`.
- Root export only. No app, Electron, React, React Native, Node file or process
  API, provider SDK, SQL driver, or cloud client import.
