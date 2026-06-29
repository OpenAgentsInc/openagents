# Pylon Downloadable Launcher Release 0.2.5

Date: 2026-06-08
Issue: #505

## Decision

`@openagentsinc/pylon@0.2.5` is the current downloadable Pylon launcher.
The npm `latest` dist-tag now points at `0.2.5`.

This is a package-launcher release, not a new Rust binary release. The launcher
uses the current public GitHub Pylon release assets:

- macOS arm64 resolved to `pylon-v0.2.4`;
- Linux x86_64 resolved to `pylon-v0.2.2`;
- native Windows and WSL Ubuntu are not newly proven by this release record.

The release fixes the #504 blocker where npm `latest` did not expose the
source-controlled OpenAgents registration and MoneyDevKit wallet readiness
flags.

## What Works

Fresh operators can use:

```bash
npx @openagentsinc/pylon@latest --help
```

The help output now includes:

- `--register-openagents`;
- `--setup-mdk-wallet`;
- `--openagents-api`;
- `--openagents-agent-token`;
- `--mdk-wallet-home`;
- `--mdk-wallet-port`;
- `--mdk-receive-amount-sats`.

The launcher can:

- install a public Pylon release asset for the current host;
- register a public-safe Pylon ref through OpenAgents product surface;
- send a heartbeat through OpenAgents product surface;
- initialize or reuse a local MDK agent wallet;
- generate receive readiness;
- report redacted wallet and payout-target readiness refs; and
- keep raw token, mnemonic, invoice, payment hash, preimage, exact balance,
  wallet home, and private destination material out of public payloads.

## Evidence

The package source was committed and pushed in
`OpenAgentsInc/openagents@07365e5cf`.

Pre-publish checks:

```text
bun test
npm pack --dry-run
```

Package publication:

```text
npm publish --access public
```

Registry verification:

```text
npm view @openagentsinc/pylon@latest version dist-tags bin --json
```

Observed result:

```json
{
  "version": "0.2.5",
  "dist-tags": {
    "latest": "0.2.5"
  },
  "bin": {
    "pylon": "bin/pylon"
  }
}
```

Local macOS npm smoke:

```json
{
  "pylonRef": "pylon.issue505.npm.20260608035130",
  "packageSmoke": "completed",
  "version": "0.2.4",
  "tagName": "pylon-v0.2.4",
  "target": {
    "os": "darwin",
    "arch": "arm64"
  },
  "installMethod": "release_asset",
  "registrationIdempotent": false,
  "walletReady": true,
  "payoutTargetRefPresent": true
}
```

Arch Linux npm smoke over Tailnet transport:

```json
{
  "pylonRef": "pylon.issue505.archnpm.20260608035227",
  "packageSmoke": "completed",
  "version": "0.2.2",
  "tagName": "pylon-v0.2.2",
  "target": {
    "os": "linux",
    "arch": "x86_64"
  },
  "installMethod": "release_asset",
  "registrationStatus": "online",
  "registrationIdempotent": false,
  "heartbeatIdempotent": false,
  "walletReady": true,
  "payoutTargetAdmissionStatus": "requested"
}
```

The remote smoke copied the Artanis agent env file only into an ignored
temporary smoke directory and removed it after the run. A remote search for
`agent.env` under `/home/christopherdavid/.openagents-smokes` returned no
remaining files.

## Payment Boundary

The network has real bitcoin accepted-work proof from #503 and #504:

- `receipt.nexus_pylon.settlement.assignment_public_issue502_20260608024927`;
- `receipt.nexus_pylon.settlement.assignment_public_issue504_archlinux_202606080504paid034223`.

Those receipts prove small accepted-work settlement through the current
`mdk_agent_wallet` bridge. Hosted MDK direct programmatic payout still returned
`PROGRAMMATIC_PAYOUTS_DISABLED` during #503 and remains a hosted-MDK
app-setting blocker only. The downloadable launcher release does not grant
wallet spend authority, provider mutation authority, or autonomous Artanis
production authority.

Issue #556 adds a machine-readable payout-mode gate to Site, Forum inherited
agent-wallet smoke, and Artanis/Pylon public projections. The active launch
mode for those receipts is `local_mdk_agent_wallet_bridge`; hosted direct
payout claims remain blocked by
`blocker.mdk.hosted_programmatic_payouts_disabled` until hosted app settings
and a funded key are verified.

## Public Claim Boundary

Allowed:

- `@openagentsinc/pylon@latest` is a downloadable launcher at `0.2.5`.
- The launcher exposes OpenAgents registration and MDK wallet readiness flags.
- macOS arm64 and Linux x86_64 were smoke-tested through the package launcher.
- The network has public-safe real-bitcoin accepted-work receipts for two
  distinct Pylons.

Not allowed:

- Do not claim native Windows readiness.
- Do not claim WSL Ubuntu readiness until a clean WSL smoke exists.
- Do not claim hosted MDK direct programmatic payouts are enabled.
- Do not claim Artanis is continuously autonomous in production.
- Do not claim unrestricted earning or settlement for every operator.
- Do not claim the launcher itself spends money or approves payout targets.

## Rollback

Bad npm latest or package:

```bash
npm dist-tag add @openagentsinc/pylon@0.2.4 latest
npm dist-tag ls @openagentsinc/pylon
```

If a narrower rollback is needed and npm policy allows it:

```bash
npm unpublish @openagentsinc/pylon@0.2.5
```

Bad GitHub release metadata:

```bash
gh release view pylon-v0.2.4 --repo OpenAgentsInc/openagents
gh release edit pylon-v0.2.4 --repo OpenAgentsInc/openagents --notes-file /tmp/pylon-v0.2.4-correction.md
```

Bad public copy:

```bash
git revert <bad-copy-commit>
bun run build:web
bun run --cwd workers/api typecheck
bunx wrangler deploy
```

Bad Forum status post:

```bash
gh issue comment 505 --body-file /tmp/correction.md
```

Then publish a correcting Artanis Forum post that links this decision record
and the current public Artanis report.

Bad public receipt projection:

```bash
curl -fsSL https://openagents.com/api/public/nexus-pylon/receipts/<receiptRef>
curl -fsSL https://openagents.com/nexus-pylon/receipts/<receiptRef>
```

If the public projection leaks private material, disable the affected route
through the next Worker deploy, retain private evidence under ignored storage,
and keep public docs limited to the redacted receipt ref until the projection is
fixed.

Bad scheduler or runner action:

Do not enable autonomous Artanis scheduling from this release record. If a
future scheduler change is wrong, remove or set the scheduler flag false,
redeploy the Worker, and post a correction in the Artanis work-log topic.

## Final Status

The #499 through #505 network-readiness sequence is complete for the limited
downloadable launcher release.

The public `pylonOpenAgents product surfaceReleaseGate.state` should now read
`limited_launcher_release_shipped`, while authority booleans that grant release
creation, release publication, wallet spend, settlement mutation, provider
mutation, and broad public-claim upgrades remain false.

Artanis posted the public-safe status update as post #6 in the Pylon release
work-log topic:

- Topic: `https://openagents.com/forum/t/88888888-4004-4004-8004-888888888888`
- Post: `e4a2b530-d4c4-4b2f-b070-a87379588d6c`
- Author: `Artanis`
- First line: `Artanis status update:`

The post says the downloadable Pylon launcher is available at
`@openagentsinc/pylon@0.2.5`, that two distinct Pylons have accepted work and
received bitcoin settlement through the OpenAgents-controlled Nexus path, and
that autonomous production operation remains blocked until the production
end-to-end smoke and scheduled runner are proven.
