# Environment computer migration

Implementation for [#11808](https://github.com/OpenAgentsInc/openagents/issues/11808).
The existing Environment setup, clean build, fresh verification, and Save owners
use the [Rust client](../../crates/openagents-sdk/) for new, explicitly enrolled
accounts. Boat is the supported artifact adapter for this migration. The
[SPACE artifact implementation](../cloud/space.md) remains a separate capability;
a Boat image is never converted into a SPACE template by changing its label.

The web and lifecycle services are deployed. The synthetic staging flow passed
clean build, image capture, two fresh verifiers, Save, and ordinary SDK reuse.
The default enrollment list is empty. The
[verification record](environment-computers-evidence.json) contains the checks,
serving revisions, qualification boundaries, and cleanup receipts.

## Allocation inventory

| Existing operation | Enrolled Environment | Original Environment |
| --- | --- | --- |
| Inspect source and develop an install recipe | Setup computer created through the public client; existing setup tools and evidence | Original Boat provider |
| Install from a frozen source and recipe | Separate builder, public create and command records, saved request identities | Original builder and capture adapter |
| Capture a clean reusable image | Private artifact call authorized by the native build owner; original stored deployment | Original named snapshot adapter |
| Check the saved image on a fresh baseline | Separate account-owned computer from the exact saved image | Original verification provider |
| Run checks and installation again on a fresh fork | Another account-owned computer; actual image hydration checked by the adapter | Original fresh verification provider |
| Save | Existing candidate review, verification evidence, and exact source/recipe/image admission | Same existing admission |
| Delete temporary computers | Public client delete, followed by observed deletion, using the saved reference | Original provider cleanup |
| Delete unused output images | Original builder binding and deployment, including after builder deletion | Original image-name cleanup |
| Private sign-in copies and stopped private homes | Existing private-home flow, separate from reusable builders | Existing private-home retention sweeper |
| Standalone headless Environment owners | Existing explicit provider configuration | Unchanged |
| Claude Code runs on saved versions | Existing run owner; chat computers have their own SDK migration | Existing run owner |

The retained private-home and standalone paths are explicit compatibility paths.
Their retirement requires a separate authorization design for interactive sign-in
and a migration of their resource records. This change does not interpret a
private checkpoint or signed-in home as a reusable environment image.

## Admission and account authority

Set both variables on the web service before enrolling an account:

| Variable | Purpose |
| --- | --- |
| `OPENAGENTS_ENVIRONMENT_COMPUTERS_ORIGIN` | Website origin of the deployed public Computers API. Production uses the SDK's canonical production deployment; a staging origin stays bound to staging. |
| `OPENAGENTS_ENVIRONMENT_COMPUTERS_ACCOUNTS` | Comma-separated account IDs admitted for new Environments. The existing cloud-access gate also applies. |
| `OPENAGENTS_WEB_ASK_SALT` | Existing private signing key, also used to bind an Environment job credential. Keep its value in the deployment's secret configuration. |

The existing deployment sealing keyring protects the user's current session and,
for a private repository, one repository broker ticket. These values are stored
separately from requests and machine records. The session and current workspace
membership epoch must still match before allocation, wake, command dispatch,
file access, output reads, or image capture. Private repository access redeems
the same bounded broker ticket again before work. Public repositories use an
anonymous Git read check and the existing exact-commit source verification.
Private attempts select a `GH_TOKEN` broker reference without requiring an
operator Git credential. There is no shared repository token fallback.
An explicit retry or edit after an attempt ends refreshes authority from the
person’s current session. Older attempts keep their original membership epoch
and expiration; refreshing the grant cannot relabel them.

An `ecj_` credential can import only an Environment job session. It cannot import
a normal account or Solver session, list computers, inspect account settings,
operate another attempt, or change saved request bytes. The web derives account
ownership from the sealed grant; the caller cannot supply an account header.
Only the trusted boundary supplies the backend's attempt header.

Ordinary account access can list the computer, inspect its lifecycle status,
stop it, cancel work, or delete it. Commands, files, and command output require
the Environment attempt's authorization. This prevents a customer from adding
private state to a clean builder through the general Computers API and prevents
a revoked grant from using an ordinary account request to bypass the attempt.
Cleanup remains available after repository or workspace access is revoked.
Builders and verifiers accept only their registered native commands; direct
file writes are limited to interactive setup computers. Saved images remain
private to the owning account and cannot become a catalog default through this
API.

## Immutable attempts and retries

The private `environment-computers/` directory contains separate attempt, grant,
image-owner, and lock records. Directories use mode `0700`; files use `0600`.
Updates use an atomic rename and sync. One filesystem lock covers each native
computer's saved requests and references.

Each attempt records its account, workspace and membership epoch, role, native
computer ID, environment, source, recipe, input digest, deadline, idle bound,
image identity, and exact create request. Builder and verifier inputs come from
the existing frozen job records. The builder's derived image name is registered
before allocation. Mutable setup-recipe edits cannot relabel an earlier attempt.
Unsupported artifact capabilities refuse before a computer is created.
The native computer's job key is separate from the Environment's build or
verification ID. Allocation and capture check the saved job, Environment IDs,
computer identity, and verifier role together; a logical build ID cannot select
another job's file.

A command keeps its native ID, specification digest, public request, public
command reference, and byte cursors. A lost create or execute reply retries the
same saved request. Recovery can look up an accepted command by its request key
without dispatching it again. A lost delete reply reads the same computer and
waits for observed deletion. Output gaps and unknown execution remain unknown;
they cannot become a successful install or verification. Command timeouts are
bounded by the public API's 3,600-second limit; computer allocation also carries
the native attempt's remaining deadline.

The native owners keep their existing evidence segments and recovery behavior.
The public command digest describes the transported command; the adapter returns
the saved native specification digest to the native evidence owner. Non-secret
Git settings and relative working directories are carried in that transported
command. Credential values arrive separately at dispatch.

Managed commands keep a physical `~/workspace` for the public file API and a
`~/repo` alias for existing saved-image consumers. An existing recipe checkout
moves as one directory; two different nonempty trees or a workspace symlink
refuse instead of merging their contents. Fresh verification and qualification
also check that a saved image remains usable through both paths.

## Clean images and credential handling

Image publication requires a native clean build in its capture phase, successful
sanitization evidence, the frozen inputs, and an observed stopped builder. The
backend independently checks the builder role, deadline, stopped state, image
name, source resource, and artifact capability. Setup computers, private
checkpoints, and ordinary stopped computers cannot publish through this route.

Before storing command output, the Rust worker removes exact credential values
using command-salted fingerprints. It stores lengths and hashes, not values or
an unfinished plaintext tail. Raw source cursors and sanitized bytes advance in
one database update, so a stale worker cannot replace newer output. A restarted
worker rereads the unfinished tail from the guest. Managed raw command logs live
under `/run/oa-environment-commands`, outside the disk image. Normal command
storage remains compatible. This is exact-value output filtering, not a claim
that arbitrary repository code cannot send a credential elsewhere.

Fresh verification still checks the artifact's source, manifest, recipe,
installation behavior, checks, and real hydration. Save uses the existing
candidate digest and immutable version admission. An install exit code or tool
version string alone cannot admit a version.

## Rollout, rollback, and cleanup

New shared records live under `<environment-state>/public-computers/`. A child
Studio runs the same native owners there; the parent combines lists and forwards
requests for `envc-` IDs. Original servers scan only the original directory, so
an older process cannot resume a new shared attempt through direct Boat calls.
Existing Environment records keep their original provider even when enrollment
changes.

To stop new enrollment, remove accounts from
`OPENAGENTS_ENVIRONMENT_COMPUTERS_ACCOUNTS`. Keep the migration-aware server, origin,
sealing keys, and original backend connections available while shared attempts
or saved images remain. Removing the origin does not authorize a fallback to
Boat. If a shared binding or connection is missing, the operation stops with an
honest unavailable outcome.

Cancellation, setup or verification failure, deadline expiry, and restart use
the original owners' cleanup. Temporary computers are deleted through their
saved public references. The hourly leftover sweep also uses those references
for shared attempts past their native deadline and grace period, recovering a
reference by its create key if the original reply was lost;
it never sends a public computer ID to the original Boat cleanup path. Image reads and deletion resolve the original builder's
stored deployment and retained provider resource even after the builder has
been deleted. A name collision with another builder refuses deletion. Saved
versions keep their images; failed or abandoned builds use the existing unused
image sweep. The hourly shared-owner sweep retries failed image cleanup without
touching a candidate still under review. An absent image remains unconfirmed
until the attempt deadline and grace period pass, because an earlier capture
request might still finish. Do not remove an old backend connection until its computer and
image inventory is empty.

Machine usage remains attached to each existing owning attempt and public
computer record. The adapter reports an unconfirmed machine bill as unknown,
with a stable evidence identity. It does not insert zero or add a new charge on
recovery. Durable backend usage and reservations use the shared
[Computers accounting](computers.md) owner.

## Verification

The staging run passed in 1,774.35 seconds and deleted its five computers and
output image. An independent provider check found no VMs for their create
identities or the earlier failed setup attempt; the output image returned `404`.
All temporary keys and sessions were revoked and rejected on recheck.

The affected checks passed: 13 Boat driver tests, 40 SDK tests, 52 Environment
operator tests, 43 lifecycle service tests with PostgreSQL, and 819 web tests.
The record lists four unrelated exclusions. Released Coder 1.0.0 passed version,
status, and computer-list checks against staging in an isolated home.


Focused tests cover job credential separation, account and membership checks,
repository revocation, sealed grant binding, frozen request retries, lost
command references, deletion recovery, byte cursors, output gaps, secret-value
filtering across restart, original-provider image cleanup, unsupported artifact
admission, and separation from older Studio recovery.

The opt-in Rust staging check is
`environments::computers::qualification::a_staging_repository_build_fresh_verifier_and_save_use_shared_computers`.
It requires `OA_ENVIRONMENT_QUALIFY_CONFIG` to name a mode-`0600` file for a
fresh disposable staging account. It drives the existing setup tools with
scripted replies and real staging lifecycle operations. Its local gateway
adapter checks the real staging account on every session read and forwards
workspace reads unchanged. This adapter exists because the native session read
is private to the deployed gateway sidecar. Qualification must record this
transport boundary and must not describe the run as an AI-model evaluation or
a deployed-browser test.

Keep qualification state and cleanup receipts in the run's scratch directory.
Delete every test computer and owned output image, revoke temporary sessions and
keys, and retain only scrubbed evidence in the repository. Never run the check
on the owner's production account or computers.
