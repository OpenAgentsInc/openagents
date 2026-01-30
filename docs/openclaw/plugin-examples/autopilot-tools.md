# OpenClaw Plugin Example: Autopilot Tools

This is a minimal, documentation-only example showing how an OpenClaw plugin
might register the `autopilot.run` and `autopilot.approval` tools and validate
payloads against the OpenAgents schemas.

> Note: This is illustrative. Adjust paths and imports to match the OpenClaw
> plugin system in the target repo.

## File layout (suggested)

```
extensions/autopilot/
├── index.ts
├── schemas/
│   ├── autopilot.run.params.json
│   ├── autopilot.run.response.json
│   ├── autopilot.approval.params.json
│   └── autopilot.approval.response.json
└── README.md
```

## index.ts (pseudo-code)

```ts
import runParamsSchema from "./schemas/autopilot.run.params.json";
import runResponseSchema from "./schemas/autopilot.run.response.json";
import approvalParamsSchema from "./schemas/autopilot.approval.params.json";
import approvalResponseSchema from "./schemas/autopilot.approval.response.json";

export default function register(api) {
  api.registerTool({
    name: "autopilot.run",
    description: "Run Autopilot in a local repo and return Verified Patch Bundle",
    schema: runParamsSchema,
    async handler(params) {
      // 1) Validate params
      api.validateSchema(params, runParamsSchema);

      // 2) Execute `autopilot run` with explicit args
      const result = await api.exec("autopilot", [
        "run",
        params.task,
        "--repo",
        params.repo_path,
        ...(params.model ? ["--model", params.model] : []),
      ]);

      // 3) Extract session id from stdout, then query bundle
      const sessionId = parseSessionId(result.stdout);
      const show = await api.exec("autopilot", [
        "session",
        "show",
        sessionId,
        "--json",
      ]);

      const response = JSON.parse(show.stdout);
      api.validateSchema(response, runResponseSchema);
      return response;
    },
  });

  api.registerTool({
    name: "autopilot.approval",
    description: "Handle approval requests from Autopilot",
    schema: approvalParamsSchema,
    async handler(params) {
      api.validateSchema(params, approvalParamsSchema);

      // Example: route to UI or policy engine
      const decision = await api.approvals.request(params);

      api.validateSchema(decision, approvalResponseSchema);
      return decision;
    },
  });
}
```

## Operational notes

- `autopilot.approval` should be allowlisted explicitly.
- Prefer `OPENCLAW_APPROVALS_URL` and tool policy to control exposure.
- `autopilot.run` should run with explicit `access` policy and repo path allowlists.
- Always surface `PR_SUMMARY.md` and `RECEIPT.json` in the OpenClaw UI.
