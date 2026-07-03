# Third-Party Target Adapter Onboarding

Status: adapter contract for QA Swarm third-party onboarding (#8069). This is
not product-promise copy; it is the integration contract a customer app must
satisfy before a swarm run can honestly claim coverage.

## Contract

A Target adapter is a public-safe JSON-shaped record that tells `qa-runner` how
to point one scenario corpus at a customer app without bespoke integration code.
It covers four things:

- **Auth**: whether the app needs no auth, env-supplied auth, device login, or a
  seeded test account.
- **Fresh identity**: every run must start from an isolated browser context and,
  for authenticated apps, a non-human test identity that can be reset or safely
  discarded.
- **Restart**: the runner may optionally restart a local/preview target by a
  command or HTTP hook. Production targets normally set `none`.
- **Production policy**: external production targets are read-only. The runner
  refuses mutating steps before touching the browser.

The code schema lives in `src/target-adapter.ts` as
`TargetAdapterContract`:

```ts
{
  schemaVersion: "openagents.qa_runner.target_adapter.v1",
  id: string,
  displayName: string,
  target: {
    name: string,
    baseUrl: string,
    environment: "local" | "preview" | "staging" | "prod" | "fixture",
    owner: "first-party" | "external",
    capabilities: Array<"browser" | "terminal">,
    restrictions?: Array<"read-only">
  },
  auth: {
    kind: "none" | "env" | "device" | "seeded-test-account",
    loginUrl?: string,
    envVars?: string[],
    freshIdentity: {
      required: boolean,
      strategy: string
    }
  },
  restart: {
    kind: "none" | "command" | "http",
    command?: string,
    url?: string
  },
  prodReadOnly: {
    policy: "read-only" | "blocked",
    allowedStepKinds: string[],
    blockedStepKinds: string[],
    notes?: string
  },
  scenarioSeeds: Array<{
    id: string,
    title: string,
    startPath: string,
    commitment: string
  }>,
  checklist: string[]
}
```

`targetFromAdapter()` converts the adapter into the runner `Target`. If
`target.owner` is `external` and `target.environment` is `prod`, it always adds
the `read-only` restriction even when the adapter omits it. If the adapter sets
`prodReadOnly.policy` to `blocked`, the run fails before a browser session can
start.

## Checklist

Before a third-party app is accepted for a QA Swarm run:

- The adapter decodes with `decodeTargetAdapterContract`.
- The base URL is the exact app surface to test; no private local paths appear
  in the adapter or artifacts.
- Auth uses a test identity, not a human operator account.
- Fresh identity is explicit: new browser context plus reset/seed strategy for
  any server-side state the scenario can touch.
- Production is read-only for external targets. Allowed steps are observation
  only: `navigate`, `wait-for`, `screenshot`, and `assert`.
- Mutating steps such as `click` and `type` are blocked on external production
  targets. Run writable flows against local, preview, staging, fixture, or a
  dedicated seeded test environment.
- Restart hooks are absent for production unless the target owner explicitly
  supplies a safe read-only refresh endpoint.
- Scenario seeds name commitments that can be confirmed from public-safe run
  receipts: result JSON, video, trace refs, screenshots, coverage ledger rows,
  and distilled tests.
- Artifacts pass the public-safety tripwire. Do not include raw prompts,
  credentials, cookies, provider payloads, private repo data, or customer
  secrets.

## Worked Example: Public Fixture App

This example uses a public read-only fixture target. It is intentionally not an
OpenAgents-owned surface, so it exercises the external-production rule without
creating customer data.

```ts
import {
  TARGET_ADAPTER_SCHEMA_VERSION,
  decodeTargetAdapterContract,
  targetFromAdapter,
} from "@openagentsinc/qa-runner/target-adapter";

const adapter = decodeTargetAdapterContract({
  schemaVersion: TARGET_ADAPTER_SCHEMA_VERSION,
  id: "fixture-public-todo-prod",
  displayName: "Fixture Public Todo",
  target: {
    name: "public-todo-prod",
    baseUrl: "https://example.com",
    environment: "prod",
    owner: "external",
    capabilities: ["browser"]
  },
  auth: {
    kind: "none",
    freshIdentity: {
      required: true,
      strategy: "fresh anonymous browser context per run; no persisted cookies"
    }
  },
  restart: { kind: "none" },
  prodReadOnly: {
    policy: "read-only",
    allowedStepKinds: ["navigate", "wait-for", "screenshot", "assert"],
    blockedStepKinds: ["click", "type"],
    notes: "External production is observed only; writable flows use staging."
  },
  scenarioSeeds: [
    {
      id: "home-renders",
      title: "Home page renders",
      startPath: "/",
      commitment: "The target home page renders public text."
    }
  ],
  checklist: [
    "Auth does not reuse a human account.",
    "Every production run is read-only.",
    "Artifacts contain only public-safe refs."
  ]
});

const target = targetFromAdapter(adapter);
// target.restrictions contains "read-only"; click/type are refused by runner policy.
```

End-to-end flow:

1. Decode the adapter and convert it with `targetFromAdapter`.
2. Generate an initial scenario corpus from `scenarioSeeds`.
3. Run the read-only smoke against production: navigate to `/`, wait for public
   text, screenshot, and assert.
4. Run writable exploratory flows only against a fixture/staging adapter with a
   fresh seeded identity.
5. Distill confirmed findings into committed `*.e2e.test.ts` scenarios and add
   the run receipts to the coverage ledger.

For a real customer app, the only adapter-specific parts should be the base URL,
auth seed/reset details, optional restart hook, and the first scenario seeds.
The runner behavior stays the same.

