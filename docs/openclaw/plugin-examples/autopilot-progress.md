# OpenClaw Plugin Example: Autopilot Progress

This is a documentation-only example showing how an OpenClaw plugin can
consume Autopilot progress events sent via `sessions_send` and attach
structured metadata to OpenClaw sessions.

## Expected payloads

Autopilot sends progress by calling `sessions_send` with:

- `session_id`
- `message` (short progress line)
- `metadata` (optional) matching `autopilot.progress.metadata.json`

Schemas:
- `docs/openclaw/schemas/autopilot.progress.params.json`
- `docs/openclaw/schemas/autopilot.progress.metadata.json`

## index.ts (pseudo-code)

```ts
import progressParamsSchema from "./schemas/autopilot.progress.params.json";
import progressMetadataSchema from "./schemas/autopilot.progress.metadata.json";

export default function register(api) {
  api.registerTool({
    name: "sessions_send",
    description: "Send a message into an OpenClaw session",
    schema: progressParamsSchema,
    async handler(params) {
      api.validateSchema(params, progressParamsSchema);

      // 1) Persist the message
      await api.sessions.send(params.session_id, params.message);

      // 2) If metadata is present, store it for status cards or dashboards
      if (params.metadata) {
        api.validateSchema(params.metadata, progressMetadataSchema);
        await api.sessions.attachMetadata(params.session_id, params.metadata);
      }

      return { ok: true };
    },
  });
}
```

## Rendering ideas

- Show a compact “Autopilot progress” card with:
  - phase (`guidance/status`, `guidance/step`, `guidance/response`, `fullauto/decision`)
  - signature or model if present
  - confidence if present
- Aggregate progress lines into a collapsible section to avoid chat spam.
