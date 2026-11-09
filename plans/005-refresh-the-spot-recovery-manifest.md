# Plan 005: The Pylon spot-recovery manifest matches its files, so its deploy command works again

> **Executor instructions**: Follow this plan step by step. Run every
> verification command and confirm the expected result before moving to the
> next step. If anything in the "STOP conditions" section occurs, stop and
> report. Do not improvise. When done, update the status row for this plan
> in `plans/README.md`, unless a reviewer dispatched you and told you they
> maintain the index.
>
> **Drift check (run first)**:
> `git diff --stat 76084e17a9..HEAD -- psionic/lib/ops/spot-recovery.coax psionic/platform/spot-recovery.sha256 psionic/platform/spot-recovery.generated.c psionic/platform/spot-recovery.h`
> If any of these changed, run step 1 first. If step 1 already reports all
> `OK`, someone has fixed this: mark the plan DONE in `plans/README.md` with
> a note and stop.

## Status

- **Priority**: P2
- **Effort**: S
- **Risk**: LOW
- **Depends on**: none
- **Category**: bug
- **Planned at**: commit `76084e17a9`, 2026-11-09

## Why this matters

`psionic/platform/pylon-spot-recovery.sh deploy` begins by checking a
manifest of SHA-256 hashes. One entry is stale: a one-line edit to
`psionic/lib/ops/spot-recovery.coax` on 2026-11-06 did not update the
manifest. The check fails, so the deploy command stops before it does
anything. The committed C file generated from that Coax source may also be
out of date. This plan regenerates the C, compares it, and refreshes the
manifest.

## Current state

Files:

- `psionic/lib/ops/spot-recovery.coax`: the policy, in Coax.
- `psionic/platform/spot-recovery.generated.c` and
  `psionic/platform/spot-recovery.h`: C generated from it and committed.
- `psionic/platform/spot-recovery.sha256`: the manifest, nine lines of
  `<sha256>  <path>` with paths relative to the repository root.
- `psionic/platform/spot-recovery-check.sh`: the local check.
- `psionic/platform/pylon-spot-recovery.sh`: the deploy script. Do not run
  its `deploy` command.

The failure, from the repository root:

```text
$ shasum -a 256 -c psionic/platform/spot-recovery.sha256
psionic/lib/ops/spot-recovery.coax: FAILED
... (the other eight lines report OK)
shasum: WARNING: 1 computed checksum did NOT match
```

The edit that caused it, commit `09101c58f9`:

```text
-spot_recovery_action :: (project:string,zone:string,vm:string,status:string,dry:string)->int #export_c {
+spot_recovery_action :: (project:string,zone:string,vm:string,status:string,dry:string)->int #export_c #effects(all) {
```

The deploy script's first step, `psionic/platform/pylon-spot-recovery.sh:41`:

```bash
  (cd "$recovery_root" && shasum -a 256 -c psionic/platform/spot-recovery.sha256)
```

How the committed C is produced and compared,
`psionic/platform/spot-recovery-check.sh:8-12`:

```bash
coax/bin/coax test psionic/lib/ops/spot-recovery.coax
coax/bin/coax mutate psionic/lib/ops/spot-recovery.coax --root-only --jobs 4
coax/bin/coax c psionic/lib/ops/spot-recovery.coax --library --header "$recovery_work/spot-recovery.h" > "$recovery_work/generated.c"
cmp "$recovery_work/generated.c" "$recovery_here/spot-recovery.generated.c"
cmp "$recovery_work/spot-recovery.h" "$recovery_here/spot-recovery.h"
```

Getting the compiler: `coax/build.sh` installs the published build for the
current tree in seconds and writes `coax/bin/coax`. If it reports that the
build is "missing", run `COAX_MUTATION=0 coax/build.sh` instead; that
bootstraps a compiler and takes much longer.

## Commands you will need

| Purpose | Command | Expected on success |
| --- | --- | --- |
| Install the compiler | `coax/build.sh` | exit 0; `coax/bin/coax` exists |
| The source's checks | `coax/bin/coax test psionic/lib/ops/spot-recovery.coax` | exit 0 |
| Manifest check | `shasum -a 256 -c psionic/platform/spot-recovery.sha256` | nine lines ending `OK` |

Run all commands from the repository root.

## Scope

**In scope**:

- `psionic/platform/spot-recovery.sha256`
- `psionic/platform/spot-recovery.generated.c` and
  `psionic/platform/spot-recovery.h`, only if step 3 shows they differ from
  a fresh compile
- `plans/README.md` (status row only)

**Out of scope**:

- `psionic/lib/ops/spot-recovery.coax`. Do not edit the source.
- `psionic/platform/pylon-spot-recovery.sh`. Do not run `deploy`, `resume`
  or any command in it that calls `gcloud`.
- Every other `*.sha256` file.

## Git workflow

- Work in a worktree: `git worktree add ../openagents-plan-005 origin/main`.
  This repository forbids branches other than `main` and edits in the main
  checkout.
- One commit, for example:
  `Psionic: the spot-recovery manifest matches its source again`.
- Do not push unless the operator tells you to.

## Steps

### Step 1: Confirm the failure

**Verify**: `shasum -a 256 -c psionic/platform/spot-recovery.sha256` reports
`FAILED` for exactly one line, `psionic/lib/ops/spot-recovery.coax`.

### Step 2: Run the source's checks

**Verify**: `coax/bin/coax test psionic/lib/ops/spot-recovery.coax` exits 0.

### Step 3: Regenerate the C and compare

```bash
work=$(mktemp -d)
coax/bin/coax c psionic/lib/ops/spot-recovery.coax --library --header "$work/spot-recovery.h" > "$work/generated.c"
cmp "$work/generated.c" psionic/platform/spot-recovery.generated.c && echo same-c
cmp "$work/spot-recovery.h" psionic/platform/spot-recovery.h && echo same-h
```

- If both `same-c` and `same-h` print, the committed C is current. Change
  neither file.
- If one differs, copy the fresh file over the committed one:
  `cp "$work/generated.c" psionic/platform/spot-recovery.generated.c` and/or
  `cp "$work/spot-recovery.h" psionic/platform/spot-recovery.h`. Then read
  `git diff psionic/platform/spot-recovery.generated.c psionic/platform/spot-recovery.h`.

**Verify**: after this step, both `cmp` commands print nothing and exit 0.

### Step 4: Refresh the manifest

For each file that the manifest check reports as `FAILED` (the Coax source,
plus the C or header if you replaced them), replace the 64 hex characters at
the start of that file's line in `psionic/platform/spot-recovery.sha256`
with the output of `shasum -a 256 <path>`. Keep the two spaces and the path.
Change no other line.

**Verify**: `shasum -a 256 -c psionic/platform/spot-recovery.sha256` prints
nine lines that all end in `OK`, with no warning.

## Test plan

No new test. The manifest check in step 4 and the comparison in step 3 are
the verification. `psionic/platform/spot-recovery-check.sh` also runs a
mutation pass and a `bun` test; run it if `bun` is installed, and report its
result, but it is not a done criterion.

## Done criteria

All must hold:

- [ ] `shasum -a 256 -c psionic/platform/spot-recovery.sha256` exits 0
- [ ] `coax/bin/coax test psionic/lib/ops/spot-recovery.coax` exits 0
- [ ] A fresh `coax c` of the source is byte-identical to the committed
      `spot-recovery.generated.c` and `spot-recovery.h` (step 3's `cmp`)
- [ ] `git diff --name-only origin/main` lists only files from the in-scope
      list
- [ ] `plans/README.md` status row updated

## STOP conditions

Stop and report back if:

- In step 3 the regenerated C differs from the committed C by more than the
  effect of one added `#effects(all)` annotation: for example, a changed
  function body or a changed return value. Report the diff. Someone must
  confirm that the deployed job's behavior is unchanged.
- `coax/bin/coax test` fails in step 2.
- More than one manifest line fails in step 1.
- `coax/build.sh` cannot install a compiler and the bootstrap also fails.

## Maintenance notes

- Nothing checks this manifest except the deploy command, so it goes stale
  whenever the Coax source is edited. The three manifests under
  `coax/web/api/` (`nip98.sha256`, `mesh-policy.sha256`,
  `caller-policy.sha256`) have `--check-inputs` commands that
  `coax/web/api/test.sh` runs. Giving spot-recovery the same treatment, or
  one check over every committed `*.sha256`, would catch the next drift at
  edit time. That check would be a new tool, which this repository requires
  to be written in Coax; it is left for a separate change.
- After this lands, the deploy is still a deliberate owner step. This plan
  only makes it possible.
