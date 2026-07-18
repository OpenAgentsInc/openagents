# OpenAgents Desktop production release operator runbook

This runbook operates the normative [OpenAgents Desktop cross-platform release
ProductSpec](./openagents-desktop-cross-platform-release.md). The ProductSpec
owns identities, targets, formats, trust, support, and rollback claims. This
file owns the operator sequence and MUST NOT be used to infer support for a
target whose implementation and native receipts are absent.

## Current transition status

ReleaseSet v2, the typed feed/resolver, impact planner, GitHub publication
adapter, candidate communications, requested-tester feedback intake, and
attributed `/changelog` contract are implemented. The real `scripts/release.ts`
CLI still uses fixture worker adapters until the owned runner registry and
native dispatch adapters in
[#8917](https://github.com/OpenAgentsInc/openagents/issues/8917) and
[#8926](https://github.com/OpenAgentsInc/openagents/issues/8926) land. This is
a typed implementation gap, not an owner-approval wait.

Until those adapters land:

- the delegated release operator MAY publish an explicitly limited GitHub RC
  candidate whose manifest and release body disclose the missing signed-feed
  boundary;
- operators MUST NOT synthesize missing native receipts, manually merge a
  partial matrix into ReleaseSet current, or call an unproved target supported;
- stable cross-platform promotion remains unavailable; and
- the existing compatibility procedure remains fail-closed on signing and
  preserves the mobile feed exactly as before.

## Canonical delegated entrypoint

The canonical signed-feed command is:

```sh
pnpm run release -- --channel <stable|rc> --version <semver> \
  --trigger-kind <owner_direction|agent_change|tester_feedback|release_incident> \
  --trigger-actor <public-actor> --trigger-ref <public-ref> \
  [--source-feedback <OpenAgents-issue-URL>]
```

The root package key `release` maps exactly to
`node --import tsx scripts/release.ts`. `--dry-run` walks the complete graph
with fixture workers and no cloud spend, `--yes` approves only declared-safe
gates, and `--resume <transaction-ref>` resumes one durable transaction. The
command prints bounded receipt lines and performs sections 1–9 without
intermediate manual commands once its concrete worker ports replace the
fixtures. `AUTHORITY.md` revision 2 delegates unattended RC release and its
transaction communications; stable is the only release-channel owner gate.

The bounded supporting commands are:

```sh
pnpm release:impact -- --base <last-delivered-ref> --head <candidate-ref>
pnpm release:github -- --manifest <publication-manifest.json> --publish
pnpm release:communicate -- --manifest <publication-manifest.json> \
  --phase candidate --release-url <github-release-url> --publish
pnpm release:feedback -- --manifest <publication-manifest.json> \
  --release-url <github-release-url> --publish
```

Each defaults to dry-run without `--publish`, validates a closed schema, and
emits no secret-bearing material. The GitHub publisher is idempotent only for
identical published bytes; it refuses asset replacement or version reuse.

## Release roles

One release has distinct roles even when one operator invokes several of
them:

| Role              | Authority                                                                                                                                           |
| ----------------- | --------------------------------------------------------------------------------------------------------------------------------------------------- |
| Coordinator       | Freezes version/channel/source revision/target set, runs common gates, dispatches owned workers, converges receipts, and requests signing/promotion |
| Target worker     | Builds one target-specific staged closure and emits artifacts, component ledger, and build receipt                                                  |
| Platform verifier | Verifies native package/signature/install/update behavior on the required native hosts                                                              |
| ReleaseSet signer | Accepts only a complete policy-valid receipt set and signs canonical ReleaseSet bytes with the pinned Ed25519 authority                             |
| Publisher         | Uploads immutable candidates and deploys `oa-updates`; cannot declare support                                                                       |
| Promoter          | Atomically advances a channel only after candidate and public-surface verification                                                                  |

For RCs, `AUTHORITY.md` revision 2 delegates Coordinator, Publisher,
Promoter, bounded release communications, requested-tester feedback intake,
linked issue creation, changelog publication, and rollback to the release
operator. This delegation does not merge evidence roles: the signer and native
verifiers still enforce their independent machine identities and proofs.

Production secrets never enter source, logs, issue comments, or public
receipts. Unattended agents MUST NOT probe the macOS Keychain. Owner-only
actions are requested only under the readiness rule in ProductSpec §17.

## 0. Select affected delivery lanes

Before changing a version or provisioning a worker, run `release:impact`
between the last delivered source ref and the candidate. Execute every selected
lane and no unselected binary lane. In particular, web, mobile OTA,
`oa-updates`, and release-infrastructure changes do not cause a Desktop build.
Any Desktop or Desktop-consumed shared-runtime/lockfile change selects the
entire five-target Desktop matrix. Unknown paths produce an operator-visible
no-binary result; they do not silently trigger spend.

The current impact command is also the explicit guard against the old habit of
rebuilding Windows and every other platform for unrelated updates. Desktop
renderer-only OTA remains prohibited until ProductSpec §11.2's signed
compatibility and rollback boundary is implemented.

## 1. Freeze immutable release inputs

Start in a new clean worktree at exact `origin/main`:

```sh
git fetch origin main
test "$(git rev-parse HEAD)" = "$(git rev-parse origin/main)"
test -z "$(git status --porcelain)"
```

Record in the coordinator request:

- exact semantic version and `stable` or `rc` channel;
- exact 40-character source revision and lockfile digest;
- ProductSpec version and ReleaseSet schema version;
- all five required target keys and eleven formats;
- expected package/product/publisher/signing-policy identities;
- owned worker and native acceptance-host inventory revisions;
- previous promoted version in the same channel;
- release-notes ref and retention class.

Reject a dirty checkout, non-main revision, reused/non-monotonic version,
prerelease on stable, partial target set, unavailable runner, or mismatched
identity before a build starts.

## 2. Run common gates once

The coordinator runs the exact repository checks required by its release
contract, including lock/generated-contract guards, unit/integration/type and
format checks, Electron security/fuse assertions, ReleaseSet
canonicalization/signature fixtures, update/rollback model checks, mobile-feed
preservation tests, and the no-GitHub-Actions authority guard.

The receipt binds the source revision and command manifest. A later code or
lockfile change invalidates every downstream receipt; do not reuse green
results across revisions.

## 3. Dispatch the owned five-target matrix

Dispatch exactly:

```text
darwin-arm64  -> DMG, ZIP
darwin-x64    -> DMG, ZIP
win32-x64     -> NSIS EXE
linux-arm64   -> AppImage, DEB, RPM
linux-x64     -> AppImage, DEB, RPM
```

Each worker uses the target-aware staging command and runner registry delivered
by DIST-03/DIST-04. A command is operator-authorized only after it is checked
into the repository, tests its target key, and emits the ProductSpec §9–10
ledger/receipt. The absence of such a command is a typed unavailable target,
not permission to substitute Forge flags manually.

Workers upload to immutable candidate object keys only. They never write a
current channel pointer or public download catalog.

## 4. Verify platform packages natively

For each target, verify the staged closure and every outer package against the
component ledger. Then run clean install, first launch, provider/agent runtime
startup, clean shutdown, N-1 update, interrupted update recovery, the
format-specific rollback boundary, reinstall, uninstall, and diagnostics on
the ProductSpec minimum/current native hosts.

Mandatory trust boundaries:

- macOS: Developer ID `OpenAgents, Inc. (HQWSG26L43)`, Team ID
  `HQWSG26L43`, notarization, staples, Gatekeeper, hardened runtime,
  entitlements, bundle/architecture/fuse/ASAR checks on downloaded bytes;
- Windows: Windows trust and Authenticode `Valid` with publisher
  `OpenAgents, Inc.` for installer, application, uninstaller, bundled CLIs,
  and native helpers before publication and before install;
- Linux: signed ReleaseSet digest/length for every package, architecture and
  payload/metadata oracles, plus native DEB/RPM package-manager lifecycle and
  AppImage retained-image update/rollback.

DEB/RPM receipts explicitly say `appOwnedRollback: false`. ZIP receipts say
`appOwnedUpdate: false` and `appOwnedRollback: false`.

## 5. Converge, generate changelogs, and sign ReleaseSet v2

The finalizer rejects unless all matrix cells agree on version, channel,
source revision, ProductSpec/schema/signing policy, target/format identity,
artifact basename, digest/length, component-ledger digest, and required native
receipts. Missing, duplicate, extra, stale, quarantined, or conflicting cells
fail closed.

After convergence and before signing, the command consumes
`docs/changelog/UNRELEASED.md` and the exact commit/issue range since the prior
release. It requires reviewed human-centric text, writes the immutable human
and detailed agent files at the ProductSpec §15.1 names, rolls the accumulator
forward idempotently, and inserts bounded human notes plus changelog refs and
digests into the ReleaseSet. A missing, unreviewed, over-bound, or inconsistent
changelog fails the transaction before signature or publication.

The isolated signer validates policy and signs the canonical ReleaseSet. It
self-verifies the exact bytes through the client verification seam before
publication. Electron Updater YAML, a storage listing, or a GitHub release is
never substituted for this signature.

## 6. Upload and serve a candidate

Upload artifacts and the immutable signed ReleaseSet to the Desktop candidate
prefix in the production Google Cloud project. Deploy `oa-updates` through its
sanctioned Google Cloud path with zero production traffic and a candidate tag.

The deploy MUST derive from or include the complete known-good mobile export.
A Desktop metadata-only directory cannot replace the service image. Before any
traffic move, GET and validate:

- every target resolution document and immutable artifact through the tagged
  candidate;
- ReleaseSet signature, target selection, SHA-256, byte length, and content
  type;
- the retained OpenAgents mobile manifest using its required Expo headers;
- unavailable/wrong-channel/wrong-architecture and signature/hash failure
  behavior.

Any mobile failure, partial target, or candidate/public-byte mismatch blocks
promotion.

### 6.1 Publish and communicate the tester candidate

After the candidate bytes pass their available gates, create a strict
`openagents.release_publication.v1` manifest. For a complete signed candidate,
it lists exactly all five targets and eleven artifacts. An experimental
candidate MUST declare `desktop_experimental_prerelease`, remain RC, enumerate
its limitations, and never be described as feed-promoted or supported.

Run the GitHub publisher. It creates a draft, uploads immutable bytes, compares
GitHub's reported size and `sha256:` digest with the local manifest, then makes
the prerelease public. Any existing tag with different bytes fails closed;
never delete/re-upload an asset or reuse the version. Publish the `candidate`
communication to every linked source issue and Forum `release-candidates`,
naming only the requested testers and asking for the structured result block.
Closed source issues are still valid feedback conversations.

The feedback intake reads only replies after the candidate marker from the
requested tester identities. A structured `PASS` creates an idempotent receipt
comment. `BLOCKED` or an unstructured result creates one linked Full Auto issue
with P0/P1 severity when supplied, comments the source conversation, and hands
the new issue back to the normal implementation loop. It never infers broad
intent from comment text and never messages unrelated users. If a requested
tester files a direct issue instead, intake may additively restore
`bug`/`area:release`/`area:desktop` only when the issue was created strictly
after the candidate marker and contains an exact linked source-issue shorthand
or canonical OpenAgents issue URL. This compensates for GitHub dropping labels
from non-collaborator API issue creation without granting tester repository
permissions or relabeling unrelated reports.

## 7. Verify `/download` and `/changelog` candidate truth

Before promotion, the typed download resolver must read the verified candidate
set without handwritten URLs. Exercise platform/architecture detection,
explicit alternatives, minimum-OS/version/channel/format copy, unavailable
targets, accessibility, and telemetry classification. A successful resolver
event is a download response, never an install or user count.

Verify `/changelog` renders the same candidate's human entry, newest-first,
links its detailed agent ledger, and exposes honest empty/degraded states. The
signed ReleaseSet notes, human source, and agent source must agree on version,
channel, date, and digests.

Public production `/download` continues resolving the old promoted set until
the atomic promotion in the next step.

## 8. Promote atomically

The promoter compares the candidate with the still-current channel, rechecks
monotonicity and all receipt refs, and advances one signed channel pointer (or
equivalent atomic selection) to the already-uploaded immutable ReleaseSet. It
does not rebuild or copy artifacts.

After promotion, repeat through production origins:

- all five target resolutions and downloaded hash/length checks;
- native update check for stable/RC identity isolation;
- `/download` default and alternatives;
- homepage download CTAs and `/changelog` version/channel/release-notes truth;
- mobile OTA manifest and representative retained asset;
- download telemetry admission/redaction;
- old/invalid client fail-closed fixtures.

Archive the promotion receipt, both changelogs, release notes, exact source revision, signed
ReleaseSet, per-target ledgers/receipts, `/download` proof, and mobile
preservation proof under ProductSpec §16. The command writes and echoes the
single final receipt at
`docs/deploy/receipts/YYYY-MM-DD-openagents-desktop-v<version>-<channel>.md`.

Finally publish the idempotent `published` communication to the same source
issues and Forum topic with the release and `/changelog` URLs. If promotion or
post-promotion health fails, publish `rolled_back` instead and include the
bounded failure/rollback ref. A promoted release is not complete until these
communications and `/changelog` attribution agree on trigger, release actor,
authority revision/grant, version, and source revision.

## 9. Failure, service rollback, and revocation

Before channel promotion, abandon/quarantine the candidate and retain the
bounded failure receipt. Never promote a partial set.

If the `oa-updates` service deployment is unhealthy, move Cloud Run traffic
back to the previous ready revision immediately. Service traffic rollback does
not change the signed Desktop channel pointer or overwrite artifacts.

If a promoted release is defective, follow the ProductSpec revocation path:
publish a signed typed revocation/unavailable state, remove it from
`/download`, and produce a strictly newer fixed release. Never repoint current
to an older version or enable downgrade. Installed clients may use only their
locally retained previous slot under the applicable format claim.

## 10. Current macOS arm64 v1 compatibility procedure

This section preserves the existing lane while the ReleaseSet v2 concrete
worker adapters and complete native receipt matrix are not yet converged. It
is an RC compatibility operation, not cross-platform support evidence, and it
MUST be retired by the v1 migration close rule in #8915.

Prerequisites:

- Apple identity `Developer ID Application: OpenAgents, Inc. (HQWSG26L43)`;
- scoped `ASC_API_*` and `OA_DEVELOPER_ID_APPLICATION` environment values;
- production Ed25519 manifest key through
  `OPENAGENTS_RELEASE_SECRETS_PATH` or its documented private JWK seam;
- sanctioned automation gcloud config outside the repository.

Run the existing focused contract gates and build:

```sh
pnpm exec vp test --run --max-concurrency 1 \
  apps/openagents-desktop/tests/release-preflight.test.ts \
  apps/openagents-desktop/tests/update-contract.test.ts \
  apps/openagents-desktop/tests/publish-release.test.ts \
  apps/openagents-desktop/tests/package-macos.test.ts \
  apps/openagents-desktop/tests/macos-gatekeeper.test.ts \
  apps/openagents-desktop/tests/launch-receipt.test.ts \
  apps/openagents-desktop/tests/update-rollback.test.ts
pnpm --dir apps/openagents-desktop run typecheck
pnpm --dir apps/openagents-desktop run build
node --import tsx apps/openagents-desktop/scripts/release-preflight.ts \
  --channel rc --latest-released <latest-version> --json
pnpm --dir apps/openagents-desktop run make:mac
```

`make:mac` rebuilds the DMG native addons and refuses without signing/notary
credentials. `OA_ALLOW_UNSIGNED_DEV=1` is local development only; its
`UNSIGNED-DEV` output cannot pass preflight or publication.

Re-run preflight against final post-staple bytes, then hash only those bytes:

```sh
DMG=apps/openagents-desktop/out/make/OpenAgents-<version>-arm64.dmg
APP=apps/openagents-desktop/out/OpenAgents-darwin-arm64/OpenAgents.app
node --import tsx apps/openagents-desktop/scripts/release-preflight.ts \
  --channel rc --latest-released <latest-version> \
  --dmg "$DMG" --app "$APP" --json
shasum -a 256 "$DMG"
stat -f '%z' "$DMG"
```

Stage v1 only through the scripted publisher:

```sh
DIST=/tmp/openagents-desktop-release-dist
rm -rf "$DIST" && mkdir -p "$DIST"
OPENAGENTS_RELEASE_SECRETS_PATH=<scoped-secret-path> \
  node --import tsx apps/openagents-desktop/scripts/publish-release.ts \
  --channel rc --version <version> --artifact "$DMG" \
  --dist-dir "$DIST" --notes-ref release.notes.<version>
```

Upload the identical immutable DMG and deploy `oa-updates` by deriving from the
current known-good image with the checked-in incremental Cloud Build path.
Never deploy a metadata-only source tree. First route only a tagged candidate.

Verify the three current Desktop documents, immutable artifact, and mobile
manifest through the candidate before traffic moves:

```sh
curl -fsS <candidate>/desktop/openagents/rc/release.json
curl -fsS <candidate>/desktop/openagents/rc/manifest.json
curl -fsS <candidate>/desktop/openagents/rc/manifest.sig.json
curl -fsS <candidate>/openagents-mobile/manifest \
  -H 'expo-protocol-version: 1' \
  -H 'expo-platform: ios' \
  -H 'expo-runtime-version: <current-mobile-runtime>' \
  -H 'expo-channel-name: openagents-production' \
  -o /tmp/mobile-manifest
```

After traffic promotion, download the public DMG to a fresh path, verify its
signed SHA-256/length, mount it, and run the packaged smoke from pristine
temporary user data. Require `[openagents-desktop smoke] OK`, lifecycle
teardown `{"ok":true,"active":0}`, the unpacked renderer and worker boundary,
and the external Codex integration oracle:

```sh
pnpm exec vp test --run apps/openagents-desktop/tests/package-macos.test.ts \
  apps/openagents-desktop/tests/release-staging.test.ts
CODEX_BIN="$(command -v codex)" \
  pnpm --dir apps/openagents-desktop run smoke:codex-turn-control
```

Require no `@openai/codex` package or Codex executable in the staged/signed
app, installed-Codex discovery `state:"ready"`, and a completed real turn using
the ordinary authenticated Codex home. Then record
clean install, account readiness, a coding turn, interruption/resume,
first-launch/rollback boundary, reinstall, diagnostics, and the repeated
mobile-feed probe. Experimental candidates remain under the OS temporary
directory and MUST NOT splice or re-sign `/Applications/OpenAgents.app`.
