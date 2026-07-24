# Agent Computer image update cadence runbook

- Date: 2026-07-24
- Issue: [AC-02 #9206](https://github.com/OpenAgentsInc/openagents/issues/9206)
- Parent: [#9193](https://github.com/OpenAgentsInc/openagents/issues/9193)
- Plan: [Agent Computer completion plan](../omega/2026-07-24-agent-computer-omega-completion-plan.md) §5 AC-02
- Image contract: `apps/pylon/deploy/agent-computer/agent-computer-image.manifest.json`
- Bake host: `agent-computer-gce-1` (`openagentsgemini`, `us-central1-a`)

This runbook tells operators when to rebake the seven-harness Agent Computer
guest rootfs, how to publish it on the nested-virtualization host, how to
record qualification in the manifest, and when parent issue `#9193` may close.

It does not replace the control-plane deploy runbook in
[Agent Computer production](./agent-computer-production.md).
That runbook publishes the `oa-codex-control` image and the OpenAgents
monolith. This runbook owns the **guest rootfs** lifecycle only.

## Image truth

`apps/pylon/deploy/agent-computer/agent-computer-image.manifest.json` is the
only checked-in image truth for the Agent Computer guest.

The manifest records:

- `guestImage.rootfsDigest` and `guestImage.rootfsDigestPrevious`
- `guestImage.rootfsBakeReceipt` (source commit and staged binary digests)
- `guestImage.codex` (baked binary pins and `executionState`)
- `guestImage.harnesses.*` (harness pins and per-harness qualification)
- `guestImage.portableSessionControl` (PORT-03 controller digest)

Do not treat a host path, a bake receipt JSON on disk, or an issue comment as
image truth unless the manifest matches it after review.

No provider key, OAuth token, wallet material, or subscription home enters the
image. Provider credentials are broker-redeemed per turn at runtime only.

## When to rebake

Rebake and requalify when any trigger below is true.

| Trigger | Examples |
| --- | --- |
| Source change | `turn-runner.ts`, `portable-session-control.ts`, `guest-agent.py`, `crates/oa-workroomd`, or egress fix logic changes |
| Harness pin change | A new Codex, Claude Code, Cursor, Goose, Grok, OpenCode, or Pi version or digest in `build-agent-computer-rootfs.sh` |
| Runtime pin change | Node, TypeScript language server, or portable-session-control bundle changes |
| Security or dependency fix | A CVE or broken upstream artifact requires a new pin |
| Timed cadence | At least once per calendar month while Agent Computer production traffic is armed, even if no pin changed |

A timed rebake alone does not close `#9193`.
Each rebake still needs boot smoke and harness requalification before the
manifest advances.

## Harness bake pins

All guest harness versions and digests are pinned in
`apps/pylon/deploy/agent-computer/build-agent-computer-rootfs.sh` at the top of
the script.

Current pins (update the script first, then rebake):

| Harness | Pin location in script | Manifest field |
| --- | --- | --- |
| Codex | `CODEX_VERSION`, `CODEX_TARBALL_SHA256`, `CODEX_BINARY_SHA256` | `guestImage.codex` |
| Claude Code | `CLAUDE_CODE_VERSION` | `guestImage.harnesses.claudeCode` |
| OpenCode | `OPENCODE_VERSION` | `guestImage.harnesses.opencode` |
| Pi | `PI_VERSION` | `guestImage.harnesses.pi` |
| Goose | `GOOSE_VERSION`, `GOOSE_TARBALL_SHA256` | `guestImage.harnesses.goose` |
| Cursor | `CURSOR_VERSION`, `CURSOR_TARBALL_SHA256` | `guestImage.harnesses.cursor` |
| Grok | `GROK_VERSION`, `GROK_BINARY_SHA256` | `guestImage.harnesses.grok` |
| Node | `NODE_VERSION`, `NODE_TARBALL_SHA256` | (runtime base) |
| TypeScript LSP | `TYPESCRIPT_LANGUAGE_SERVER_*`, `TYPESCRIPT_*` | `guestImage.portableSessionControl.managedHelperArtifacts` |

After you change a pin in the bake script, update the matching manifest fields
in the same commit that lands the rebake receipt.

## Preconditions

- Work from a clean `main` checkout at the commit you will bake.
- Use the isolated automation gcloud config documented in workspace `AGENTS.md`.
- The bake host is Linux x86_64 with nested virtualization and `/dev/kvm`.
- You have root on the bake host over IAP SSH.
- You never overwrite the live rootfs file in place. The bake script writes a
  new `--output` path every time.

## 1. Stage guest binaries

On a build machine with the repository checkout:

```sh
# Workroom sidecar (Linux guest binary via Docker when the builder is not Linux)
apps/pylon/deploy/agent-computer/build-workroomd-for-image.sh --docker

# Or on the bake host directly when Rust targets linux x86_64:
# apps/pylon/deploy/agent-computer/build-workroomd-for-image.sh
```

Record the staged paths under `var/agent-computer/staging/` (gitignored).

Pack `turn-runner` and `portable-session-control` on the bake host with Vite
Plus, or pass `--repo-root` to the bake script so it compiles them during the
bake.

## 2. Rebake the rootfs

Run as root on `agent-computer-gce-1`:

```sh
cd /path/to/openagents/apps/pylon/deploy/agent-computer

sudo ./build-agent-computer-rootfs.sh \
  --repo-root /path/to/openagents \
  --workroomd /path/to/staged/oa-workroomd \
  --output /srv/openagents/cloud-vm/agent-computer-rootfs-<date>-v<N>.ext4
```

The script debootstraps jammy, installs the seven pinned harnesses, copies the
vsock guest agent, turn-runner, portable-session-control, and oa-workroomd,
applies the egress fix, runs `e2fsck`, seals `sha256`, and writes a
refs-and-digests-only bake receipt JSON beside the image.

Keep the previous live rootfs path until boot smoke and at least one harness
qualification turn pass on the candidate.

## 3. Boot smoke (mandatory before manifest promotion)

Before you change `guestImage.rootfsDigest`, prove the candidate boots on the
live provisioner:

1. Point the control daemon at the candidate rootfs path (not the manifest
   digest yet).
2. Run one bounded `POST /v1/cloud-vm/sessions` smoke with
   `provisionerKind=live`.
3. Confirm guest exec returns code `0`, artifacts extract, and
   `cleanupReceipt.tornDown=true`.
4. Confirm the host has zero Firecracker processes, zero TAP devices, and zero
   runtime directories after cleanup.
5. Run version probes for all seven harness binaries inside the guest.

Only after this smoke passes may you update `guestImage.rootfsDigest`,
`guestImage.rootfsDigestPrevious`, and `guestImage.rootfsBakeReceipt` in the
manifest.

## 4. Promote the rootfs on the host

After the manifest commit lands:

1. Copy or rename the candidate ext4 to the host path the control daemon reads
   (normally under `/srv/openagents/cloud-vm/`).
2. Restart or reload `oa-codex-control` if the daemon caches the rootfs path.
3. Re-run the readiness probe from
   [Agent Computer production](./agent-computer-production.md) §2.

Do not repoint production Cloud Run until readiness and one bounded smoke are
green on the new digest.

## 5. Requalify harnesses after a material rebake

Any rebake that changes `rootfsDigest`, a harness pin, turn-runner, workroomd,
or portable-session-control requires fresh runtime qualification for every
harness that executes real turns on Agent Computer.

For each harness (`pi`, `opencode`, `goose`, `claudeCode`, `cursor`, `grok`,
`codex`):

1. Run one real managed-cloud coding turn through the production or staging
   dispatch path (not a fixture-only runner).
2. Require staged change, verifier, writeback, exact or typed usage receipt,
   and microVM teardown.
3. Record public-safe refs only in the issue comment. Never paste prompts,
   tokens, or private traces.

Skip requalification for a harness only when the rebake diff cannot affect
that harness (for example a docs-only manifest note with an unchanged digest).
Document that skip in the closeout comment.

## 6. Record qualification in the manifest

Update `agent-computer-image.manifest.json` for each qualified harness:

| Field | Value |
| --- | --- |
| `guestImage.harnesses.<id>.executionState` | `runtime_secret_and_real_writeback_qualified` when the real turn passed |
| `guestImage.harnesses.<id>.qualification.provedAt` | UTC date of the turn |
| `guestImage.harnesses.<id>.qualification.turnRef` | Durable turn ref from the dispatch receipt |
| `guestImage.harnesses.<id>.qualification.exitCode` | `0` on success |
| `guestImage.harnesses.<id>.qualification.artifactRefs` | Public artifact digest refs from closeout |
| `guestImage.harnesses.<id>.qualification.commit` | Resulting commit SHA when writeback occurred |
| `guestImage.harnesses.<id>.qualification.cleanupReceipt` | Teardown receipt digest |
| `guestImage.harnesses.<id>.qualification.usage` | Exact or unavailable usage block from the turn |

Update summary fields:

- `guestImage.harnesses.status` — human-readable roll-up (for example
  `six_of_seven_runtime_qualified_owner_reauthentication_required_for_codex`)
- `guestImage.rootfsStatus` — one-line boot and qualification summary for the
  active digest

For Codex, also update `guestImage.codex.executionState` (see next section).

Commit manifest changes in the same push that cites the qualification issue
comment.

## 7. Codex owner-reauthentication gate (AC-01)

As of 2026-07-24, six harnesses are runtime-qualified on the live v24 image.
Codex is **not** qualified.

The manifest records:

```json
"guestImage.codex.executionState": "owner_reauthentication_required"
```

The baked Codex binary is present and boot-smoke proven.
A live Codex coding turn still needs a fresh owner device login into an
**isolated** Codex home for Agent Computer.

**Never** run `codex login` or `pylon auth codex` against the default
`~/.codex` home. That flow clears the owner live session.

AC-01 ([#9205](https://github.com/OpenAgentsInc/openagents/issues/9205)) owns
the owner action, the isolated login, one real Codex qualification turn, and
flipping `guestImage.codex.executionState` to qualified.

This runbook does not close `#9193` while Codex lacks a real-turn receipt.
Do not mark Codex qualified in the manifest without AC-01 proof.

## 8. Close `#9193`

Close [#9193](https://github.com/OpenAgentsInc/openagents/issues/9193) only
when all conditions below are true:

1. `guestImage.rootfsDigest` matches the live host rootfs.
2. All seven harnesses have `executionState`:
   `runtime_secret_and_real_writeback_qualified` (Codex included).
3. Each harness has a populated `qualification` block with turn, artifact,
   cleanup, and usage refs from real turns.
4. One roll-up issue comment lists every harness receipt and links this
   runbook.
5. This runbook and the manifest are on `main`.

Until AC-01 completes, keep `#9193` open and cite the Codex gate honestly in
the roll-up comment.

## 9. Standing cadence

While Agent Computer serves production managed-cloud turns:

| Activity | Cadence |
| --- | --- |
| Review manifest vs live host digest | Weekly |
| Rebake from current `main` if any trigger fired | Within 5 business days of the trigger |
| Timed safety rebake (no pin change) | Monthly |
| Full seven-harness requalification | After every material rebake |
| Roll-up comment on `#9193` or successor tracking issue | After each material promotion |

If a rebake fails boot smoke, roll back to `guestImage.rootfsDigestPrevious`
on the host and restore the prior manifest digest in Git before you investigate.

## Related documents

- [Agent Computer production deploy](./agent-computer-production.md)
- [Agent Computer host README](../../apps/pylon/deploy/agent-computer/README.md)
- [Agent Computer completion plan](../omega/2026-07-24-agent-computer-omega-completion-plan.md)
- [Agent Computer isolation posture](../khala-code/2026-07-06-agent-computer-isolation-posture.md)
