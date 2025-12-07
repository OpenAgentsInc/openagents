# $TS Type Fixes Progress

- Added a helper in `src/tasks/service.ts` to merge `TaskUpdate` payloads without introducing `undefined`, and stopped overriding properties with `undefined` during updates.
- Guarded `tasksPath` inclusion when calling `updateTask` so optional CLI parameters no longer force `undefined` through the new SQLite API surface.
- Ran `bun run typecheck` ➜ still failing (~many modules) with the expected remaining errors around archivist, bench, CLI, and trainer layers.
