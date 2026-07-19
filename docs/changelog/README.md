# Release changelogs (DIST-14, #8927)

Every OpenAgents release carries two changelog artifacts:

1. **Human changelog** — clear, human-centric language: what changed for the
   user and why it matters. No commit-hash soup, no internal codenames. It is
   published at <https://openagents.com/changelog> and, once the signed
   ReleaseSet v2 (#8915) lands, embedded as the bounded release-notes field in
   the signed release payload so `/download` (#8924) and the in-app update
   prompt can show it.
2. **Agent changelog** — the detailed engineering ledger: per-change entries
   with issue refs, commits, contracts/specs touched, invariants affected,
   evidence links, and the acting lane/session. It lives in this directory and
   is linked from the human changelog, but it is never the primary public
   surface.

The human changelog uses the base STE profile.
The agent changelog uses the base profile when possible.
It can use the versioned [agent compact profile](../ste/agent-compact-profile.v1.md).
This profile permits controlled technical terms and labeled record fragments.
It does not permit an omitted condition, limit, proof state, or authority reference.

RC.25 is the reference pattern for this separation.
Its human section explains actions, benefits, and release limits.
Its agent section gives issues, commits, contracts, invariants, evidence, and lane data.
Do not rewrite a released changelog to apply this policy after publication.

Every dated release also records a public-safe provenance header: trigger
kind, trigger actor, release actor, authority reference, release URL, and
source-feedback reference. Historical releases say when they predate the
admitted authority profile rather than retroactively claiming a newer grant.
New autonomous RCs name the exact `AUTHORITY.md` profile revision, program,
and grant that permitted publication and communication.

## Directory layout

| Path                              | Role                                                            |
| --------------------------------- | --------------------------------------------------------------- |
| `UNRELEASED.md`                   | Accumulator: one entry per landed change since the last release |
| `YYYY-MM-DD-desktop-<version>.md` | One file per release: human changelog + agent changelog         |
| `README.md`                       | This convention document                                        |

File naming currently uses the `desktop` product segment because the Desktop
release set is the release unit (epic #8913). Naming and retention policy
defer to the DIST-01 release product specification (#8914) once it lands; a
rename there updates this convention in the same change.

The published page at `/changelog` is generated from the dated release files
into a committed data module
(`apps/openagents.com/apps/start/src/routes/-changelog-data.gen.ts`) that the
Start route imports at build time. The Start site is built and deployed from
committed source — there is no live changelog backend, exactly as `/download`
ships its release constants — so a build-time import is the honest,
cache-correct strategy. Do not edit the generated module by hand.

## CLAIM-RELEASE protocol: append your UNRELEASED entry

**Appending an entry to `UNRELEASED.md` is part of the CLAIM-RELEASE protocol
for landing lanes.** The CLAIM comment you already post on the issue carries
the needed facts (actor/session, base, scope); when your change lands on
`main`, append one entry to `UNRELEASED.md` in the same change or in the
integration commit. A landed change without an UNRELEASED entry is invisible
to the next release's changelog draft.

### Entry format

```markdown
## <Short title> (#NNNN)

- issues: #NNNN, #MMMM
- commits: <short shas on main, comma-separated>
- contracts-specs: <contracts, specs, or schema files touched, or `none`>
- invariants: <invariants added/relaxed/reinterpreted, or `none changed`>
- evidence: <links or repo paths to receipts/proofs, or `none`>
- lane: <CLAIM actor/session that landed the change>

<One or more paragraphs. The FIRST paragraph must be a clear, human-centric
summary — what changed for the user and why it matters — because the release
script drafts the human changelog from it. Later paragraphs may carry
engineering detail.>
```

All six metadata keys are required; write `none` (or `none changed`)
honestly rather than omitting a key. The first summary paragraph must obey
the user-facing copy rules: plain language, no commit hashes, no internal
codenames.

## Cutting a release

`scripts/changelog.ts` is the generation tool (also exposed as
`pnpm changelog`):

```sh
# Roll UNRELEASED into a dated release file, reset the accumulator,
# regenerate the /changelog data module, and print the bounded
# release-notes string:
pnpm changelog roll --version 0.1.0-rc.14 --channel rc --date 2026-07-20

# Regenerate the /changelog data module from the dated files:
pnpm changelog sync

# Verify the committed data module matches regeneration (CI/tests):
pnpm changelog check

# Print the bounded release-notes string for a released version:
pnpm changelog notes --version 0.1.0-rc.13
```

`roll` drafts the human changelog from the first summary paragraph of each
UNRELEASED entry plus the release header. The release operator is authorized
to review and edit that bounded draft under `AUTHORITY.md` revision 2; a
separate owner ceremony is not required for an RC. Stable publication retains
its explicit gate. The committed artifact is reviewed text, not raw
generation. `roll` refuses to overwrite an existing release file and refuses
to run when UNRELEASED has no entries, so re-running it is safe.

## Release-notes bound (ReleaseSet v2, #8915)

The release-notes string embedded in the signed ReleaseSet payload is
length-bounded. The bound is exported from `scripts/changelog.ts` as
`RELEASE_NOTES_MAX_LENGTH` (2000 characters); #8915 should import that
constant (or move it into the ReleaseSet v2 schema package and re-export it
here) so the schema bound and the generator bound can never drift. The full
human entry always lives on `/changelog`; the bounded string is a truncated
plain-text projection, never the authority.
