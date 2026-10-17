# Labs consolidation handoff — 2026-10-16

Stopped at the owner's explicit instruction. No further implementation, tests, image builds, promotion, or production traffic changes are running from this session. No subagents are owned by this session. Grid, Foreman, and Synced chats were notified. Claims for #11875 and #11877 are being released for the consolidating agent.

## Merged work

- #11873: complete and closed; main `2dfaac8e46`.
- #11874: complete and closed. The production Labs API, SQLx URL secret handling, keyless artifact signer, and shared-clock deployment fix are on main. Production evidence is `docs/sdk/labs-api-evidence.json`; preserve the live serving configuration and secrets.
- #11875: CAD implementation is on main `2f3465b29180a37b580c56e5fd48da93f510c199`. Browser cookie/project access regression fix is `218a67f92fd5734fefc8436ca7b4fe4c88bbf430`. Smoke-driver authentication and redirect fixes are `5bdf74bba61fc067679b1277a1c64c61923c7aac`.

## #11875 deployment status

The single combined CAD and Foreman #11934 image was built from current-main snapshot `e29e4c9bf62f87cd0615b81091da3193b25b6869`. Build `d63e37bf-b6ec-4c53-bd3f-608f426b4b11` succeeded. Image digest: `sha256:8cf73886eda6a8d71dbcfa4489d06b31ba718e85639b6249aeb504fcc50dbd1a`.

Staging shifted to `openagents-web-1-staging-e29e4c9bf6-022727`, completed its outgoing-listener drain, and passed the standard suite: **99 passed, 0 failed, 2 skipped**. The first smoke attempt stopped before HTTP checks because local Python lacked its CA path; rerunning with `SSL_CERT_FILE=/etc/ssl/cert.pem` passed. The stage extraction was removed after completion. No production promotion or shift happened. Exact traffic snapshots and qualification flags are in [the staging handoff receipt](../sdk/labs-cad-staging-handoff.json).

**Still required:** run `scripts/smoke/sdk-labs-cad.py` with the dedicated test accounts and optional isolated browser helper, qualify CAD creation/edit/versioning/STEP export and cleanup on staging, then promote the exact digest with the guarded deployment script, qualify production, and coordinate Foreman's saved-agent check. The prepared private launcher is `<scratch>/run-labs-cad-live.py`; set `OPENAGENTS_LABS_SOURCE_COMMIT` to the image's exact e29e commit. It loads dedicated credentials without printing them. Run browser checks through `openagents browser run`; never use the owner's account. No new CAD/project/job fixtures were created by this stopped rollout.

The reviewed production migration is **not copied**. `docs/sdk/labs-cad-migration-review.json` records one scope, one document, two versions, eleven files, 1,940,606 bytes, with reviewed checksum `16445f2f215a047871e0016b99dc53f64ab5cdc7a792e1a6b4914f8a55db93ae`. An actual copy still needs the owner's approval and is recorded in `NEEDS_OWNER.md`; do not perform it automatically. The issue can close after its own deployment/qualification, with that owner-only step recorded.

CAD source checks already passed: CAD57/1ignored plus focused final Lab tests3; the real private-PG/fake-GCS Lab store/API checks; 22 focused CAD web checks; fresh account authority; browser-cookie/project-revocation regression; OpenAPI/route inventory checks; `lab-cad-copy` build and help; changed-file formatting. One unrelated existing world-physics Cloud configuration test was excluded. Detailed logs are retained under the scratch root.

## #11877 unfinished work

All current source is committed locally as `115414d5af` on `codex/labs-sequential-11873`. The complete source patch is retained in [2026-10-16-labs-11877-wip.patch](2026-10-16-labs-11877-wip.patch). It is based on SDK source `1b55b50ae17dcd47315f803a8238ab7c3c3afe82`. **The patch is intentionally unapplied on main: its final combined source has not compiled or passed acceptance.** Apply or cherry-pick it in an isolated worktree and finish it before integration.

Implemented in the patch:

- Read-only `Api::read_card`, latest-card, and bounded artifact helpers using existing authorization/reference checks; reusable web Labs runtime factory.
- Additive history `cards`, fresh document reads, personal-owner isolation, shared-project revocation handling, inaccessible-card reports, and removal of private `_lab` bindings from public event cards.
- Typed SDK saved cards with account-bound `DocumentRef`, unknown-view-to-Report conversion, and the additive native account fixture field.
- New UI-free SDK consumer `lab-ui`: one Rust Native card composition with typed open actions and opaque media resources. Web media is an adapter to that view, including the existing CAD viewer.
- Generic hosted document page, saved-card fragment, sandboxed HTML artifact player, cookie-to-bearer normalization for fresh account checks, and history OpenAPI schema edits.
- Work toward registry-listed hosted tools through `openagents_sdk::labs`, exact tool membership, exact document/kind checks, transient request headers for spawned chat turns, and preservation of existing loopback adapters.

Passed before later edits: two focused history-card tests (CAD/Echo, cross-account same ID, project revocation, no job starts, unrelated cards preserved), and the shared lab-ui test. **Not run on final patch:** SDK history integration tests, web/registry/renderer tests, final Cargo resolution/lock regeneration, full generated OpenAPI export, or production CAD-chat/history/second-browser acceptance. The registry follow-up regression needs a fixture for a registered kind, not only the existing legacy Labs fixtures. Review the new cookie-authority normalization, final route wiring, generic card controls, and compatibility with subsequent main changes. Existing CAD page/engine behavior should remain the authority for detailed CAD editing.

## Ownership and infrastructure

Grid owns #11876 (SDK, source `1b55b50ae17dcd47315f803a8238ab7c3c3afe82`, SDK59/Labs10/optionalCAD1 passed) and #11878 (jobs/placement, unqualified source kept separate). Do not overwrite their branches or shared Kind/Store interfaces. L7 proposes additive server capability/job hooks; coordinate integration with its owner. Synced owns the shared harness implementation. Its published decision is `abd722e4d5`: Coder calls the UI-free `openagents-harness`; do not add another engine driver. The owner lifted the binary freeze in `47d8cc1a17`; #11885 is native Coder Labs, not a freeze workaround. Only the designated release agent publishes assets/tags/releases.

Local worktree: `/Users/christopherdavid/.openagents/scratch/claude-code-c3160c08-2f00-4201-a8dd-1358376c9832/labs-sequential`.
Scratch root: `/Users/christopherdavid/.openagents/scratch/claude-code-c3160c08-2f00-4201-a8dd-1358376c9832`.
Persistent local target: `/Users/christopherdavid/work/openagents-target-labs-sequential`.
Deployment state: `<scratch>/labs-deploy/openagents-deploy`; candidate tag `labs-11875`. Use `SSL_CERT_FILE=/etc/ssl/cert.pem` for local Python HTTP checks. Preserve logs, product binaries, and common caches. Some completed optional test symbols/objects and one obsolete test executable were reclaimed during the shared disk emergency.

Linux: existing spike host, private `~space-spike/labs-11877`, persistent `~space-spike/openagents-target-agent6`, private TMPDIR `~space-spike/labs-11877-tmp`. Target6 reflink seed completed while target2 was explicitly idle, then target2 was released. The source upload and an earlier WIP overlay completed, but **no Cargo command ran there**; the final local commit/retained patch is authoritative. Access with `SPACE_SPIKE_ISSUE=11812 deploy/space-host/host.sh ssh --command ...` and explicitly `sudo runuser -l space-spike -c ...`; the helper defaults to space-admin. Use jobs2/debug0/incremental0, umask077, private0700 roots, and no shared active target. VM expiry is around 06:11 UTC; copy any needed evidence before 05:30 UTC. Grid was told target6 is idle so it can clean up the host.

Synced's former next web deployment slot is released; no #11894 deployment gate remains. The consolidating agent owns further deployment coordination.

Patch SHA-256: `a92175c013648510016bd0959a3d60590016286d1e988f22276c1bfa52b285c9`.
