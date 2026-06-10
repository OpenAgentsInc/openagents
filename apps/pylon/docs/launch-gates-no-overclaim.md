# Pylon v0.3 Launch Gates And No-Overclaim Copy

Status: implemented for `0.3.0-rc1` local release gating.

Run the local release gate:

```sh
bun run release:gate
```

The gate runs:

- unit and runtime tests;
- bootstrap JSON smoke;
- status JSON smoke;
- inventory JSON smoke;
- operator snapshot JSON smoke;
- dashboard startup smoke;
- package dry-run;
- local package install smoke.

The public package claim currently allowed is:

- `@openagentsinc/pylon@0.3.0-rc1` is the v0.3 release candidate.
- `Pylon can use optional local Qwen3.5 inference when the Psionic backend,
  model, and tool-call gates pass`.

The optional local inference claim is bounded. It does not claim default
bundled models, startup downloads, paid inference, training, or universal
machine support.

Blocked copy until separate evidence rows exist:

- `Pylon v0.3.0 is stable`;
- `Pylon v0.3 is assignment-ready across the network`;
- `Paid Pylon work settles Bitcoin`;
- `Qwen is training on people's devices`;
- `Paid Qwen inference is live on Pylons`;
- `Pylons sell compute capacity live`;
- `full live GEPA network`;
- live marketplace, referral payout, or data revenue claims.

`src/launch-gates.ts` exposes `projectLaunchGateMatrix()` for evidence refs and
`assertLaunchCopyAllowed()` for copy checks. OpenAgents product surface/public launch copy should
consume the same states: allowed, blocked, or planned. Blocked claims require
the named evidence refs before copy can change.

macOS and Linux are the only first launch platforms. A CI workflow is still
blocked until the GitHub token used by the agent has workflow scope; until then,
`bun run release:gate` is the required local gate before public copy changes.

## Allowed scoped executor claim (v0.3)

The ONLY executor copy a v0.3 release may carry is the promise's scoped
safeCopy: one workload family (the Tassadar PoC numeric-model trace),
executed by a registered Pylon, replay-verified byte-identically by the
worker, with a dated settled receipt - receipts cited, nothing
generalized. "Paid Pylon work settles Bitcoin" and "Pylons sell compute
capacity live" remain blocked as general copy; release notes must use
the scoped line above, never the general ones.

## Local Claude Agent bridge copy (issues #4717-#4720)

Until `pylon.local_claude_agent_bridge.v1` carries real-device receipts,
the ONLY allowed copy for the lane is presence copy: the SDK dependency,
readiness probe, and `capability.pylon.local_claude_agent` declaration
exist. "Pylon commands your local Claude", "your Claude does coding work
through Pylon", and any executed-assignment claim remain blocked until
the #4720 bounded real-task smoke has live receipts. Branding rule in
all copy: "Claude Agent" / "your local Claude" / "Powered by Claude" -
never "Claude Code" (Anthropic branding terms).
