# HE-02: Released harness-environment artifact for Omega

- Date: 2026-07-24
- Packet: `HE-02`
- OpenAgents issue: [#9210](https://github.com/OpenAgentsInc/openagents/issues/9210)
- Depends on: closed [#9209](https://github.com/OpenAgentsInc/openagents/issues/9209) (HE-01)
- Plan: [2026-07-24-agent-computer-omega-completion-plan.md](./2026-07-24-agent-computer-omega-completion-plan.md)
- STE issue: 9
- Glossary revision: `openagents-ste-glossary-v1`
- Status: complete

## Result

`@openagentsinc/agent-harness-environment@0.1.0-rc.1` is published to the public
npm registry under the `rc` dist-tag.
Omega and `omega-effectd` can depend on released bytes.
They do not need a relative openagents path for this package.

## Artifact

| Field | Value |
| --- | --- |
| Package | `@openagentsinc/agent-harness-environment` |
| Version | `0.1.0-rc.1` |
| Dist-tag | `rc` (also current `latest` for first publish) |
| Tarball SHA-256 | `9ed2d1c2439dfd33f736b2d3f63795144f7ffb9ad0ce8965f49cc78cd44334fd` |
| Registry tarball | `https://registry.npmjs.org/@openagentsinc/agent-harness-environment/-/agent-harness-environment-0.1.0-rc.1.tgz` |
| npm integrity | `sha512-Ee+KtqkUXPApKV5c2bSqqHOdKk7hyUVDuizu7xKVL/XDpFsj0W+1uLRH3sa/4WIuUFytlj5Sz7t45qLx/Qk6KQ==` |
| Peer floor | `@openagentsinc/agent-harness-contract` and `@openagentsinc/agent-runtime-schema` `>=0.2.1-rc.4`, `effect` `>=4.0.0-beta.94` |

## Consumer pin

`packages/omega-effectd/package.json` now depends on
`@openagentsinc/agent-harness-environment@0.1.0-rc.1` (not `workspace:*`).
In the monorepo, pnpm still links the matching workspace package version.
Outside the monorepo, install resolves from npm only.

## Verification

1. `pnpm pack` in `packages/harness-environment` produced the tarball.
2. `npm publish <tarball> --access public --tag rc` returned success.
3. `npm pack @openagentsinc/agent-harness-environment@0.1.0-rc.1` from an empty
   directory reproduced SHA-256
   `9ed2d1c2439dfd33f736b2d3f63795144f7ffb9ad0ce8965f49cc78cd44334fd`.
4. Package tests remain green under `packages/harness-environment`.

## Non-claims

- This receipt does not publish `@openagentsinc/omega-effectd`.
- This receipt does not close Omega live Agent Computer proof (`OpenAgentsInc/omega#30`).
- This receipt does not qualify Codex on Agent Computer (`#9205`).
